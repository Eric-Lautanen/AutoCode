// probe.rs -- Automated model capability probe.
//
// Given a provider (with API key) and a model id, runs a bounded sequence of
// live API calls to learn the model's real capabilities: context window,
// max output tokens, thinking/reasoning args and which efforts are accepted,
// and vision support. The result lands in the probed-model registry
// (autocode_core::helpers::probed) and the providers.json file, so the model
// behaves exactly like a baked-in one afterwards.
//
// Progress is reported through a channel of ProbeUpdate values; the UI polls
// them every frame and renders a progress bar. Every stage is cancel-aware
// and every failure is recovered from (fall back to the next strategy /
// conservative defaults) — a probe can report partial results, never wedge.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use rustls::pki_types::ServerName;

use autocode_core::state::{ApiProvider, ModelManifest, ProbeSpec};

use super::http::{build_http_request, connect_tcp, decode_chunked, tls_config};
use super::permits::with_permit;

// -- Public surface ------------------------------------------------------------

/// Coarse probe phase, shown as a label next to the progress bar.
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeStage {
    /// Contacting /models, reading model metadata.
    Metadata,
    /// Baseline completion: reachability, output-token floor, passively
    /// observed reasoning fields.
    Basic,
    /// Trying thinking-arg conventions and verifying reasoning efforts.
    Thinking,
    /// Vision probe (tiny image request).
    Vision,
    /// Finished (success or partial — check `ok` on the final update).
    Done,
}

impl ProbeStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Metadata => "Fetching model metadata",
            Self::Basic => "Probing baseline completion",
            Self::Thinking => "Probing thinking / reasoning",
            Self::Vision => "Probing vision support",
            Self::Done => "Done",
        }
    }
}

/// One progress tick. `fraction` is 0.0-1.0 overall; `message` is a short
/// human-readable status line. The final update carries `ok`.
#[derive(Debug, Clone)]
pub struct ProbeUpdate {
    pub stage: ProbeStage,
    pub fraction: f32,
    pub message: String,
    pub ok: Option<bool>,
}

/// A model entry parsed from the provider's /models catalog.
#[derive(Debug, Clone)]
pub struct FetchedModel {
    pub id: String,
    pub metadata: Option<serde_json::Value>,
}

/// Handle to a running probe. Poll `poll()` from the UI thread; dropping the
/// handle cancels the probe at the next request boundary.
pub struct ProbeHandle {
    rx: Receiver<ProbeUpdate>,
    done: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl ProbeHandle {
    /// Non-blocking poll: the newest update since the last call, if any.
    pub fn poll(&self) -> Option<ProbeUpdate> {
        let mut latest = None;
        loop {
            match self.rx.try_recv() {
                Ok(u) => latest = Some(u),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        latest
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }
}

impl Drop for ProbeHandle {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Launch a probe on a background thread. Safe to drop the handle at any
/// time; the worker stops at the next request boundary.
pub fn probe_model(provider: ApiProvider, model_id: String, spec: ProbeSpec) -> ProbeHandle {
    let (tx, rx) = std::sync::mpsc::channel::<ProbeUpdate>();
    let done = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));

    let worker_done = Arc::clone(&done);
    let worker_cancel = Arc::clone(&cancel);
    let _ = std::thread::Builder::new()
        .name("model-probe".into())
        .spawn(move || {
            // Share the global request-concurrency gate with real completions.
            with_permit(worker_cancel.as_ref(), || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_probe(provider, model_id, spec, &tx, &worker_cancel)
                }));
                if let Err(p) = result {
                    let msg = autocode_core::helpers::panic_msg(&p);
                    let _ = tx.send(ProbeUpdate {
                        stage: ProbeStage::Done,
                        fraction: 1.0,
                        message: format!("Probe failed: {msg}"),
                        ok: Some(false),
                    });
                }
                worker_done.store(true, Ordering::Relaxed);
            });
        });

    ProbeHandle { rx, done, cancel }
}

// -- Stage thresholds (overall fraction) ---------------------------------------
//
// Weights mirror where probe time actually goes: metadata is a single GET,
// while the baseline completion is a full model round-trip — and on
// serverless gateways (NVIDIA NIM and friends) it can absorb minutes of
// cold-start model loading. Long in-flight requests never stall the bar: a
// `Crawler` creeps the fraction forward within the current stage band while
// the response is pending.

const FRAC_METADATA_START: f32 = 0.02;
const FRAC_METADATA_END: f32 = 0.12;
const FRAC_BASIC_START: f32 = 0.14;
/// Baseline crawl ceiling — output floor-probing continues from here.
const FRAC_BASIC_CRAWL_END: f32 = 0.50;
const FRAC_BASIC_END: f32 = 0.52;
/// Thinking splits into convention candidates, then per-effort verification;
/// the bar crawls through each slice without doubling back.
const FRAC_THINKING_CANDIDATES_END: f32 = 0.70;
const FRAC_THINKING_END: f32 = 0.88;
const FRAC_VISION_END: f32 = 1.0;

/// Per-request deadline unless the provider's probe spec overrides it
/// (`ProbeSpec::timeout_secs`). Serverless gateways can take minutes to load
/// a model on first request, so this is generous — and the baseline retries
/// once on top of it (see `baseline_probe`).
const DEFAULT_PROBE_TIMEOUT_SECS: u64 = 60;
/// TCP connect deadline: endpoints answer connects in seconds or not at all.
const CONNECT_TIMEOUT_SECS: u64 = 20;
/// Wait between baseline retry attempts (lets a loading model finish).
const RETRY_BACKOFF_SECS: f32 = 2.0;
/// How often an in-flight request nudges the progress bar.
const CRAWL_TICK: Duration = Duration::from_secs(2);
const METADATA_MAX_BYTES: usize = 4 * 1024 * 1024;

// Context-window probe tiers. When a gateway's /models metadata doesn't
// state the context window (NVIDIA NIM's catalog doesn't), these sizes are
// tested live from largest to smallest — the largest that answers wins.
// Most current models accept ≥100K; 1M covers the biggest text models
// (GLM 5.3 and friends) without risking pathological test costs.
const CONTEXT_TIERS: [u32; 5] = [
    1_048_576, // 1M
    524_288,   // 512K
    262_144,   // 256K
    131_072,   // 128K
    100_000,   // 100K — most models clear this
];

fn emit(
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    stage: ProbeStage,
    fraction: f32,
    message: String,
) {
    let _ = tx.send(ProbeUpdate {
        stage,
        fraction,
        message,
        ok: None,
    });
}

// -- Progress crawler ------------------------------------------------------------

/// Keeps the progress bar honest during long in-flight requests. While the
/// probe waits on a response (cold-start model loading is the worst case),
/// the bar would otherwise sit frozen at the stage's start fraction; the
/// crawler emits a heartbeat every `CRAWL_TICK` with a fraction that creeps
/// from `from` toward `to` — asymptotically, so it never crosses into the
/// next stage's slice. Stage completion emits own the crossing.
struct Crawler<'a> {
    tx: &'a std::sync::mpsc::Sender<ProbeUpdate>,
    stage: ProbeStage,
    from: f32,
    to: f32,
    start: std::time::Instant,
    last_tick: std::time::Instant,
}

impl<'a> Crawler<'a> {
    fn new(
        tx: &'a std::sync::mpsc::Sender<ProbeUpdate>,
        stage: ProbeStage,
        from: f32,
        to: f32,
    ) -> Self {
        let now = std::time::Instant::now();
        Self {
            tx,
            stage,
            from,
            to,
            start: now,
            last_tick: now,
        }
    }

    /// Current crawl fraction: 1 - 1/(1 + elapsed/90) crosses ~25% of the
    /// band after 30s, ~40% after 60s, and approaches but never reaches 1.
    fn frac(&self) -> f32 {
        let elapsed = self.start.elapsed().as_secs_f32();
        let progress = 1.0 - 1.0 / (1.0 + elapsed / 90.0);
        (self.from + (self.to - self.from) * progress).min(self.to)
    }

    /// Called from the read loop every `CRAWL_TICK` while nothing arrives.
    fn tick(&mut self) {
        if self.last_tick.elapsed() < CRAWL_TICK {
            return;
        }
        self.last_tick = std::time::Instant::now();
        emit(
            self.tx,
            self.stage.clone(),
            self.frac(),
            format!(
                "Still waiting for response… {}s",
                self.start.elapsed().as_secs()
            ),
        );
    }
}

/// Shared knobs for one probe HTTP exchange.
struct NetCtx<'a> {
    cancel: &'a AtomicBool,
    timeout_secs: u64,
    /// Invoked ~every `CRAWL_TICK` while the response is pending.
    tick: &'a mut dyn FnMut(),
}

impl<'a> NetCtx<'a> {
    fn new(cancel: &'a AtomicBool, timeout_secs: u64, tick: &'a mut dyn FnMut()) -> Self {
        Self {
            cancel,
            timeout_secs,
            tick,
        }
    }
}

/// Cancel-aware sleep (retry backoff).
fn sleep_cancelable(cancel: &AtomicBool, secs: f32) -> Result<(), String> {
    let end = std::time::Instant::now() + Duration::from_secs_f32(secs);
    while std::time::Instant::now() < end {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// The baseline completion with up to three attempts on transient failures
/// (timeouts, 5xx, 429, 408, dropped connections). Serverless gateways —
/// NVIDIA NIM in particular — load a model on first request and can take
/// minutes, and their load balancers may drop the connection while it is
/// still loading; the model often finishes loading in the background, so a
/// later attempt hits a warm endpoint. All attempts share one progress
/// crawler so the bar keeps creeping across the backoffs.
fn baseline_probe(
    provider: &ApiProvider,
    model_id: &str,
    body: &serde_json::Value,
    cancel: &AtomicBool,
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    timeout_secs: u64,
) -> Result<(u16, serde_json::Value), String> {
    const ATTEMPTS: u32 = 3;
    let mut crawler = Crawler::new(
        tx,
        ProbeStage::Basic,
        FRAC_BASIC_START,
        FRAC_BASIC_CRAWL_END,
    );
    for attempt in 1..=ATTEMPTS {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        if attempt > 1 {
            emit(
                tx,
                ProbeStage::Basic,
                crawler.frac(),
                format!(
                    "Attempt {attempt}/{ATTEMPTS} — model may still be loading (serverless cold start)…"
                ),
            );
            sleep_cancelable(cancel, RETRY_BACKOFF_SECS * (attempt - 1) as f32)?;
        }
        // Scope ctx so the crawler is free again for the result emits below.
        let result = {
            let mut tick = || crawler.tick();
            let mut ctx = NetCtx::new(cancel, timeout_secs, &mut tick);
            chat_once(provider, model_id, body, &mut ctx)
        };
        match result {
            Ok((status, resp)) => {
                // 4xx (except 429 rate-limit and 408 idle-kill) is a real
                // rejection — no point retrying.
                if status.eq(&200)
                    || ((400..500).contains(&status) && status != 429 && status != 408)
                    || attempt == ATTEMPTS
                {
                    return Ok((status, resp));
                }
                emit(
                    tx,
                    ProbeStage::Basic,
                    crawler.frac(),
                    format!("HTTP {status} — retrying…"),
                );
            }
            Err(e) => {
                if attempt == ATTEMPTS {
                    return Err(e);
                }
                let hint = if e.contains("timed out") {
                    format!(
                        "Request timed out after {timeout_secs}s — serverless models can take minutes to load; retrying…"
                    )
                } else {
                    format!("{e} — retrying…")
                };
                emit(tx, ProbeStage::Basic, crawler.frac(), hint);
            }
        }
    }
    unreachable!("loop returns on the final attempt")
}

// -- Main probe sequence ---------------------------------------------------------

fn run_probe(
    provider: ApiProvider,
    model_id: String,
    spec: ProbeSpec,
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    cancel: &AtomicBool,
) {
    let mut notes: Vec<String> = Vec::new();
    let mut context_window: Option<u32> = None;
    let mut max_output: Option<u32> = None;
    let mut output_floor: Option<u32> = None;
    let mut metadata: Option<serde_json::Value> = None;
    let timeout_secs = spec.timeout_secs.unwrap_or(DEFAULT_PROBE_TIMEOUT_SECS);

    // ── Stage 1: metadata ────────────────────────────────────────────────
    emit(
        tx,
        ProbeStage::Metadata,
        FRAC_METADATA_START,
        format!("Fetching model catalog from {}", provider.base_url),
    );
    let base = provider.base_url.trim_end_matches('/');
    let models_url = if !provider.models_list_url.is_empty() {
        provider.models_list_url.replace("{base_url}", base)
    } else {
        format!("{base}/models")
    };

    let mut catalog_len = 0usize;
    let mut meta_crawler = Crawler::new(
        tx,
        ProbeStage::Metadata,
        FRAC_METADATA_START,
        FRAC_METADATA_END * 0.9,
    );
    let mut meta_tick = || meta_crawler.tick();
    let mut meta_ctx = NetCtx::new(cancel, timeout_secs, &mut meta_tick);
    match http_get_json(&provider, &models_url, &mut meta_ctx) {
        Ok((status, body)) if status.eq(&200) => {
            let models = extract_models(&body);
            catalog_len = models.len();
            if let Some(found) = models.iter().find(|m| m.id == model_id) {
                metadata = found.metadata.clone();
            } else if catalog_len == 0 {
                notes.push("model catalog was empty or unparseable".into());
            } else {
                notes.push(format!(
                    "model not listed in catalog ({} entries); using default metadata",
                    catalog_len
                ));
            }
        }
        Ok((status, body)) => {
            notes.push(format!(
                "models endpoint returned HTTP {status} — continuing without metadata ({})",
                short_body(body.as_str().unwrap_or(""))
            ));
        }
        Err(e) => {
            notes.push(format!("models endpoint unreachable: {e}"));
        }
    }

    if let Some(mv) = &metadata {
        context_window = context_from_metadata(mv);
        max_output = max_output_from_metadata(mv);
        emit(
            tx,
            ProbeStage::Metadata,
            FRAC_METADATA_END * 0.9,
            format!(
                "Catalog OK ({} models). Model metadata found{}",
                catalog_len,
                context_window
                    .map(|c| format!(" — context ≈ {}", fmt_tokens(c)))
                    .unwrap_or_default()
            ),
        );
    } else {
        emit(
            tx,
            ProbeStage::Metadata,
            FRAC_METADATA_END * 0.9,
            format!(
                "Catalog OK ({} models). No metadata for this model — values will be measured live",
                catalog_len
            ),
        );
    }
    emit(
        tx,
        ProbeStage::Metadata,
        FRAC_METADATA_END,
        "Metadata stage complete".into(),
    );

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    // ── Stage 2: baseline completion ─────────────────────────────────────
    emit(
        tx,
        ProbeStage::Basic,
        FRAC_BASIC_START,
        "Sending baseline completion probe".into(),
    );

    let basic_probe_max = max_output.unwrap_or(4096).clamp(64, 2048);
    let passive_reasoning;

    match baseline_probe(
        &provider,
        &model_id,
        &basic_body(&model_id, basic_probe_max, &[]),
        cancel,
        tx,
        timeout_secs,
    ) {
        Ok((status, body)) if status.eq(&200) => {
            // Some gateways return HTTP 200 with an {"error": ...} body.
            // Treat that as a rejection, not a successful probe.
            if body.get("error").is_some_and(|e| !e.is_null()) {
                let msg = format!(
                    "baseline probe rejected (HTTP 200 with error body): {}",
                    error_message(&body)
                );
                notes.push(msg.clone());
                emit(tx, ProbeStage::Basic, FRAC_BASIC_END, msg);
                finish_probe(
                    tx,
                    &provider,
                    &model_id,
                    false,
                    context_window,
                    max_output.or(output_floor),
                    "off".into(),
                    Vec::new(),
                    false,
                    false,
                    notes,
                    spec,
                    std::collections::HashMap::new(),
                );
                return;
            }
            let content = resp_text(&body);
            let reasoning = resp_reasoning(&body);
            passive_reasoning = !reasoning.is_empty();
            if content.is_empty() && reasoning.is_empty() {
                let finish = resp_finish(&body).unwrap_or_default();
                if finish == "length" {
                    // Output cap hit with zero visible text (thinking-only or
                    // strict cap). Floor-probe to find where text appears.
                    emit(
                        tx,
                        ProbeStage::Basic,
                        FRAC_BASIC_CRAWL_END,
                        "Empty response at max_tokens — measuring output floor".into(),
                    );
                    match floor_probe(&provider, &model_id, cancel, tx, timeout_secs) {
                        Some((floor, saw_content)) => {
                            output_floor = Some(floor);
                            if saw_content {
                                max_output = max_output.or(Some((floor as f32 * 2.5) as u32)
                                    .map(|v: u32| v.next_power_of_two()));
                            } else {
                                notes.push(format!(
                                    "no visible output up to {} max_tokens — model may be thinking-only at small budgets",
                                    floor
                                ));
                            }
                        }
                        None => {
                            notes.push("output floor probe failed (all attempts errored)".into())
                        }
                    }
                } else {
                    notes.push(format!(
                        "baseline response was empty (finish_reason: {})",
                        if finish.is_empty() {
                            "none".into()
                        } else {
                            finish
                        }
                    ));
                }
            } else if !content.is_empty() {
                emit(
                    tx,
                    ProbeStage::Basic,
                    FRAC_BASIC_END,
                    "Baseline completion OK — model responds normally".into(),
                );
            } else {
                emit(
                    tx,
                    ProbeStage::Basic,
                    FRAC_BASIC_END,
                    "Baseline OK — reasoning-only output observed (thinking model)".into(),
                );
            }
        }
        Ok((status, body)) => {
            let msg = format!(
                "baseline probe rejected: HTTP {status} — {}",
                error_message(&body)
            );
            notes.push(msg.clone());
            emit(tx, ProbeStage::Basic, FRAC_BASIC_END, msg);
            finish_probe(
                tx,
                &provider,
                &model_id,
                false,
                context_window,
                max_output.or(output_floor),
                "off".into(),
                Vec::new(),
                false,
                false,
                notes,
                spec,
                std::collections::HashMap::new(),
            );
            return;
        }
        Err(e) => {
            let mut msg = format!("baseline probe failed: {e}");
            if msg.contains("timed out") {
                msg.push_str(&format!(
                    " — model never answered within {timeout_secs}s per attempt. Serverless endpoints can take several minutes to load a model; raise timeout_secs for this provider and re-probe."
                ));
            } else if msg.contains("connection closed") {
                msg.push_str(
                    " — the server dropped the request mid-flight, which serverless gateways commonly do while a model is still loading. The probe already retried 3×; try re-probing in a minute or two."
                );
            }
            notes.push(msg.clone());
            emit(tx, ProbeStage::Basic, FRAC_BASIC_END, msg);
            finish_probe(
                tx,
                &provider,
                &model_id,
                false,
                context_window,
                max_output.or(output_floor),
                "off".into(),
                Vec::new(),
                false,
                false,
                notes,
                spec,
                std::collections::HashMap::new(),
            );
            return;
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    // ── Stage 2b: context-window tiers (when metadata didn't say) ────────
    // Runs only when the catalog gave no context size (NVIDIA NIM). The
    // largest tier that answers wins, so 1M-context models register as 1M
    // instead of falling back to a 128K default.
    let (ctx, floor_out) = context_and_output(
        &provider,
        &model_id,
        context_window,
        output_floor,
        cancel,
        tx,
        timeout_secs,
    );
    context_window = Some(ctx);
    if floor_out.is_some() {
        output_floor = floor_out;
    }
    emit(
        tx,
        ProbeStage::Basic,
        FRAC_BASIC_END,
        format!("Context window: {}", fmt_tokens(ctx)),
    );

    // ── Stage 3: thinking / reasoning ─────────────────────────────────────
    let mut thinking_api = String::from("off");
    let mut verified_efforts: Vec<String> = Vec::new();
    let mut thinking_overrides: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

    if spec.thinking_style != ProbeSpec::THINKING_OFF {
        // Candidate order: provider convention first, then metadata-inferred,
        // then the remaining conventions as fallbacks.
        let mut candidates: Vec<String> = vec![spec.thinking_style.clone()];
        for h in reasoning_hint_from_metadata(metadata.as_ref()) {
            if !candidates.contains(&h) {
                candidates.push(h);
            }
        }
        for c in [
            ProbeSpec::THINKING_OPENROUTER,
            ProbeSpec::THINKING_OPENAI,
            ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS,
        ] {
            if !candidates.iter().any(|x| x == c) {
                candidates.push(c.to_string());
            }
        }

        let efforts: Vec<String> = if spec.reasoning_efforts.is_empty() {
            vec!["high".into()]
        } else {
            spec.reasoning_efforts.clone()
        };
        let floor = output_floor.unwrap_or(basic_probe_max);

        let total_candidates = candidates.len();
        let mut confirmed: Option<(String, Vec<String>)> = None;

        // Bar budget for "which convention?": the first half of the thinking
        // span. Per-effort verification crawls the second half below.
        let cand_span = FRAC_THINKING_CANDIDATES_END - FRAC_BASIC_END;

        for (ci, cand) in candidates.iter().enumerate() {
            if confirmed.is_some() {
                break;
            }
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let frac = FRAC_BASIC_END + cand_span * (ci as f32 / total_candidates as f32);
            let band_end = FRAC_BASIC_END + cand_span * ((ci + 1) as f32 / total_candidates as f32);
            emit(
                tx,
                ProbeStage::Thinking,
                frac,
                format!("Testing thinking convention: {cand}"),
            );
            let mut crawler = Crawler::new(tx, ProbeStage::Thinking, frac, band_end);

            let args = thinking_args(cand, efforts.first().unwrap_or(&"high".into()));
            let mut body = basic_body(&model_id, floor.max(512), &args);
            let mut tick = || crawler.tick();
            let mut ctx = NetCtx::new(cancel, timeout_secs, &mut tick);
            match chat_once(&provider, &model_id, &body, &mut ctx) {
                Ok((status, resp)) if status.eq(&200) => {
                    // Args accepted (or silently ignored). Decide from output.
                    let text = resp_text(&resp);
                    let reasoning = resp_reasoning(&resp);
                    if !text.is_empty() || !reasoning.is_empty() {
                        confirmed = Some((cand.clone(), Vec::new()));
                        if cand == ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS {
                            // Keep the exact wire JSON the gateway accepted so
                            // future requests reproduce it verbatim.
                            for (k, v) in args {
                                thinking_overrides.insert(
                                    efforts.first().cloned().unwrap_or_else(|| "high".into()),
                                    serde_json::Value::Object({
                                        let mut m = serde_json::Map::new();
                                        m.insert(k, v);
                                        m
                                    }),
                                );
                            }
                        }
                    } else {
                        // Args accepted but nothing generated at the floor
                        // budget: likely a thinking model whose whole budget
                        // went to reasoning. Try one higher budget before
                        // giving up on this convention.
                        body["max_tokens"] = serde_json::json!(floor.max(512) * 4);
                        match chat_once(&provider, &model_id, &body, &mut ctx) {
                            Ok((s2, r2)) if s2.eq(&200) => {
                                let t2 = resp_text(&r2);
                                let rr2 = resp_reasoning(&r2);
                                if !t2.is_empty() || !rr2.is_empty() {
                                    confirmed = Some((cand.clone(), Vec::new()));
                                    if cand == ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS {
                                        for (k, v) in args {
                                            thinking_overrides.insert(
                                                efforts
                                                    .first()
                                                    .cloned()
                                                    .unwrap_or_else(|| "high".into()),
                                                serde_json::Value::Object({
                                                    let mut m = serde_json::Map::new();
                                                    m.insert(k, v);
                                                    m
                                                }),
                                            );
                                        }
                                    }
                                    if rr2.is_empty() && !t2.is_empty() {
                                        notes.push(
                                            "thinking args accepted; reasoning not visible in output".into(),
                                        );
                                    }
                                } else {
                                    notes.push(format!(
                                        "thinking convention '{cand}' accepted args but produced no output"
                                    ));
                                }
                            }
                            _ => {
                                notes.push(format!(
                                    "thinking convention '{cand}' accepted args but produced no output"
                                ));
                            }
                        }
                    }
                }
                Ok((status, resp)) if (400..500).contains(&status) => {
                    // 4xx with thinking args → retry without them to prove the
                    // request shape itself is fine; if the base also 4xx's the
                    // model is likely unavailable entirely.
                    match chat_once(
                        &provider,
                        &model_id,
                        &basic_body(&model_id, floor.max(512), &[]),
                        &mut ctx,
                    ) {
                        Ok((s2, _)) if s2.eq(&200) => {
                            notes.push(format!(
                                "thinking convention '{cand}' rejected: HTTP {status} ({})",
                                error_message(&resp)
                            ));
                        }
                        _ => {
                            notes.push("model rejected both thinking and plain requests at floor budget — skipping remaining conventions".to_string());
                            break;
                        }
                    }
                }
                Ok((status, resp)) => {
                    notes.push(format!(
                        "thinking probe got HTTP {status} ({}) — stopping thinking stage",
                        error_message(&resp)
                    ));
                    break;
                }
                Err(e) => {
                    notes.push(format!("thinking probe transport error: {e}"));
                    break;
                }
            }
        }

        if let Some((cand, _)) = confirmed {
            // Verify which efforts are actually accepted (cap 4 to bound time).
            let check: Vec<String> = efforts.iter().take(4).cloned().collect();
            let per = (FRAC_THINKING_END - FRAC_THINKING_CANDIDATES_END)
                / (check.len().max(1) as f32 + 1.0);
            for (ei, eff) in check.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let eff_from = FRAC_THINKING_CANDIDATES_END + per * ei as f32;
                let eff_to = FRAC_THINKING_CANDIDATES_END + per * (ei as f32 + 1.0);
                emit(
                    tx,
                    ProbeStage::Thinking,
                    eff_from,
                    format!("Verifying reasoning effort '{eff}'"),
                );
                let args = thinking_args(&cand, eff);
                let mut crawler = Crawler::new(tx, ProbeStage::Thinking, eff_from, eff_to);
                let mut tick = || crawler.tick();
                let mut ctx = NetCtx::new(cancel, timeout_secs, &mut tick);
                match chat_once(
                    &provider,
                    &model_id,
                    &basic_body(&model_id, floor.max(512), &args),
                    &mut ctx,
                ) {
                    Ok((s, r)) if s.eq(&200) => {
                        if !resp_text(&r).is_empty() || !resp_reasoning(&r).is_empty() {
                            verified_efforts.push(eff.clone());
                            if cand == ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS {
                                thinking_overrides.insert(
                                    eff.clone(),
                                    serde_json::Value::Object({
                                        let mut m = serde_json::Map::new();
                                        for (k, v) in args {
                                            m.insert(k, v);
                                        }
                                        m
                                    }),
                                );
                            }
                        } else {
                            notes.push(format!("effort '{eff}' accepted but produced no output"));
                        }
                    }
                    Ok((s, r)) => {
                        notes.push(format!(
                            "effort '{eff}' rejected: HTTP {s} ({})",
                            error_message(&r)
                        ));
                    }
                    Err(e) => {
                        notes.push(format!("effort '{eff}' check failed: {e}"));
                    }
                }
            }

            // "off" args check: can the model disable thinking on request?
            let off_args = thinking_off_args(&cand);
            if let Some(off) = off_args {
                let mut noop = || {};
                let mut ctx = NetCtx::new(cancel, timeout_secs, &mut noop);
                match chat_once(
                    &provider,
                    &model_id,
                    &basic_body(&model_id, floor.max(512), &off),
                    &mut ctx,
                ) {
                    Ok((s, _)) if s.eq(&200) => {
                        if cand == ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS {
                            thinking_overrides.insert(
                                "off".into(),
                                serde_json::Value::Object({
                                    let mut m = serde_json::Map::new();
                                    for (k, v) in off {
                                        m.insert(k, v);
                                    }
                                    m
                                }),
                            );
                        }
                    }
                    _ => {
                        notes.push("model cannot disable thinking via API args".into());
                    }
                }
            }

            thinking_api = match cand.as_str() {
                ProbeSpec::THINKING_OPENROUTER => "openrouter".into(),
                ProbeSpec::THINKING_OPENAI => "openai".into(),
                _ => "off".into(), // wire format carried by thinking_overrides
            };
        } else if passive_reasoning {
            thinking_api = "openai".into();
            verified_efforts = efforts.into_iter().take(1).collect();
            notes.push(
                "no thinking convention confirmed, but reasoning fields observed — defaulting to openai-style".into(),
            );
        } else {
            notes.push("no thinking convention accepted — marking model as non-thinking".into());
        }

        emit(
            tx,
            ProbeStage::Thinking,
            FRAC_THINKING_END,
            if thinking_api == "off" && thinking_overrides.is_empty() {
                "Thinking stage complete — no thinking support detected".into()
            } else {
                format!(
                    "Thinking stage complete — {} (efforts: {})",
                    if thinking_api == "off" {
                        "gateway overrides"
                    } else {
                        &thinking_api
                    },
                    if verified_efforts.is_empty() {
                        "none".to_string()
                    } else {
                        verified_efforts.join(", ")
                    }
                )
            },
        );
    } else {
        emit(
            tx,
            ProbeStage::Thinking,
            FRAC_THINKING_END,
            "Thinking probe skipped (provider convention: off)".into(),
        );
    }

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    // ── Stage 4: vision ───────────────────────────────────────────────────
    let mut supports_vision = false;
    if spec.vision_probe {
        emit(
            tx,
            ProbeStage::Vision,
            FRAC_THINKING_END + 0.03,
            "Testing image input support".into(),
        );
        let hint = vision_hint_from_metadata(metadata.as_ref());
        match hint {
            Some(false) => {
                supports_vision = false;
                emit(
                    tx,
                    ProbeStage::Vision,
                    FRAC_VISION_END,
                    "Vision stage complete — metadata says text-only".into(),
                );
            }
            hint => {
                let mut crawler = Crawler::new(
                    tx,
                    ProbeStage::Vision,
                    FRAC_THINKING_END + 0.03,
                    FRAC_VISION_END - 0.01,
                );
                let mut tick = || crawler.tick();
                let mut ctx = NetCtx::new(cancel, timeout_secs, &mut tick);
                match chat_once(
                    &provider,
                    &model_id,
                    &vision_body(&model_id, basic_probe_max.min(256)),
                    &mut ctx,
                ) {
                    Ok((s, r))
                        if s == 200
                            && (!resp_text(&r).is_empty() || !resp_reasoning(&r).is_empty()) =>
                    {
                        supports_vision = true;
                        emit(
                            tx,
                            ProbeStage::Vision,
                            FRAC_VISION_END,
                            "Vision stage complete — image input accepted".into(),
                        );
                    }
                    Ok((s, _r)) if (400..500).contains(&s) => {
                        if hint == Some(true) {
                            notes.push(
                                "metadata suggests vision but the image request was rejected (may need larger model tier)".into(),
                            );
                        }
                        emit(
                            tx,
                            ProbeStage::Vision,
                            FRAC_VISION_END,
                            "Vision stage complete — image input rejected".into(),
                        );
                    }
                    _ => {
                        emit(
                            tx,
                            ProbeStage::Vision,
                            FRAC_VISION_END,
                            "Vision stage complete — inconclusive".into(),
                        );
                    }
                }
            }
        }
    } else {
        emit(
            tx,
            ProbeStage::Vision,
            FRAC_VISION_END,
            "Vision probe skipped".into(),
        );
    }

    finish_probe(
        tx,
        &provider,
        &model_id,
        true,
        context_window,
        max_output.or(output_floor),
        thinking_api,
        verified_efforts,
        supports_vision,
        passive_reasoning,
        notes,
        spec,
        thinking_overrides,
    );
}

/// Emit the final result, register the probed model, and report success.
/// `baseline_ok` is false when the model itself was unreachable (auth error,
/// model not found, network down): in that case the probe reports failure and
/// the UI does NOT add the model — the user fixes the cause and re-probes.
#[allow(clippy::too_many_arguments)]
fn finish_probe(
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    provider: &ApiProvider,
    model_id: &str,
    baseline_ok: bool,
    context_window: Option<u32>,
    max_output: Option<u32>,
    thinking_api: String,
    efforts: Vec<String>,
    supports_vision: bool,
    passive_reasoning: bool,
    notes: Vec<String>,
    spec: ProbeSpec,
    thinking_overrides: std::collections::HashMap<String, serde_json::Value>,
) {
    let ctx = context_window.unwrap_or(131_072);
    // Thinking models burn budget on reasoning before visible text; keep a
    // sane output default relative to the context window.
    let out = max_output.unwrap_or_else(|| 8192.min(ctx / 2).max(2048));
    let max_out_thinking = if thinking_api != "off" || passive_reasoning {
        Some((out * 4).min(ctx / 2).max(out))
    } else {
        None
    };

    let manifest = ModelManifest {
        context_window: ctx,
        max_output_tokens: out,
        max_output_tokens_thinking: max_out_thinking,
        thinking_api: thinking_api.clone(),
        reasoning_efforts: efforts,
        supports_cache_control: spec.supports_cache_control,
        requests_per_hour: None,
        thinking_overrides,
        supports_vision,
    };

    // Only register models that passed the baseline gate. Failed probes
    // surface as FAILED in the UI without inserting anything — the user fixes
    // the cause (key, model id, network) and re-probes.
    if baseline_ok {
        autocode_core::helpers::probed::insert(&provider.kind, model_id, manifest.clone());
    }
    let summary = if !baseline_ok {
        format!(
            "Model unreachable — {}",
            notes
                .last()
                .cloned()
                .unwrap_or_else(|| "probe failed".into())
        )
    } else if notes.is_empty() {
        format!(
            "Probed {model_id}: context {}, max output {}{}",
            fmt_tokens(ctx),
            fmt_tokens(out),
            if supports_vision { ", vision" } else { "" }
        )
    } else {
        format!("Probed {model_id} with notes: {}", notes.join("; "))
    };
    let _ = tx.send(ProbeUpdate {
        stage: ProbeStage::Done,
        fraction: 1.0,
        message: summary,
        ok: Some(baseline_ok),
    });
}

// -- Probe request bodies --------------------------------------------------------

/// Baseline body: one tiny user turn, optional extra thinking args.
fn basic_body(
    model: &str,
    max_tokens: u32,
    extra_thinking: &[(String, serde_json::Value)],
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Reply with exactly: OK"}],
        "max_tokens": max_tokens,
        "temperature": 0.0,
        "stream": false,
    });
    for (k, v) in extra_thinking {
        body[k.as_str()] = v.clone();
    }
    body
}

/// 4x4 red PNG as a data URL — smallest valid image any vision endpoint
/// should accept. Targets the OpenAI image_url content-part shape.
const PROBE_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAFklEQVR4nGP8z8Dwn4GBgYGJgQoAAF9vAgN6vRcUAAAAAElFTkSuQmCC";

fn vision_body(model: &str, max_tokens: u32) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Describe this image in one short sentence."},
                {"type": "image_url", "image_url": {"url": PROBE_PNG_DATA_URL}}
            ]
        }],
        "max_tokens": max_tokens,
        "stream": false,
    })
}

/// Thinking args for one convention + effort. Returns (key, value) pairs to
/// merge at the top level of the request body.
fn thinking_args(style: &str, effort: &str) -> Vec<(String, serde_json::Value)> {
    match style {
        ProbeSpec::THINKING_OPENROUTER => {
            vec![("reasoning".into(), serde_json::json!({"effort": effort}))]
        }
        ProbeSpec::THINKING_OPENAI => {
            vec![("reasoning_effort".into(), serde_json::json!(effort))]
        }
        ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS => {
            vec![(
                "chat_template_kwargs".into(),
                serde_json::json!({"enable_thinking": true, "reasoning_effort": effort}),
            )]
        }
        _ => Vec::new(),
    }
}

/// Args that request thinking disabled, when a convention supports expressing it.
fn thinking_off_args(style: &str) -> Option<Vec<(String, serde_json::Value)>> {
    match style {
        ProbeSpec::THINKING_OPENROUTER => Some(vec![(
            "reasoning".into(),
            serde_json::json!({"effort": "none"}),
        )]),
        ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS => Some(vec![(
            "chat_template_kwargs".into(),
            serde_json::json!({"enable_thinking": false}),
        )]),
        // openai-style: omitting reasoning_effort IS the off state.
        _ => None,
    }
}

/// Bisect the smallest max_tokens where the model still produces visible
/// text. Probes 64 → 256 → 1024 → 4096, keeping the first level with output.
fn floor_probe(
    provider: &ApiProvider,
    model: &str,
    cancel: &AtomicBool,
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    timeout_secs: u64,
) -> Option<(u32, bool)> {
    let levels = [64u32, 256, 1024, 4096];
    let mut crawler = Crawler::new(tx, ProbeStage::Basic, FRAC_BASIC_CRAWL_END, FRAC_BASIC_END);
    for mt in levels {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        emit(
            tx,
            ProbeStage::Basic,
            crawler.frac(),
            format!("Probing max_tokens={mt}"),
        );
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": mt,
            "stream": false,
        });
        let mut tick = || crawler.tick();
        let mut ctx = NetCtx::new(cancel, timeout_secs, &mut tick);
        match chat_once(provider, model, &body, &mut ctx) {
            Ok((200, resp)) => {
                let completion = resp
                    .pointer("/usage/completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let saw = !resp_text(&resp).is_empty() || !resp_reasoning(&resp).is_empty();
                let capped = resp_finish(&resp).as_deref() == Some("length");
                if saw {
                    return Some((mt, true));
                }
                if capped && completion > 0 {
                    return Some((mt, false));
                }
            }
            Ok((s, r)) => {
                // 4xx at this level: the cap is above what the API allows for
                // this tier — bail with the previous level.
                let _ = r;
                if (400..500).contains(&s) {
                    return None;
                }
            }
            Err(_) => {}
        }
    }
    None
}

// -- Metadata extractors (pure, unit-tested) -------------------------------------

/// Pull the model list out of a /models response. Handles OpenAI
/// `{"data": [...]}`, bare arrays, and `{"models": [...]}` shapes. Ids come
/// from `id` / `name` / `model` fields.
pub fn extract_models(body: &serde_json::Value) -> Vec<FetchedModel> {
    let arr: Vec<serde_json::Value> = if let Some(a) = body["data"].as_array() {
        a.clone()
    } else if let Some(a) = body["models"].as_array() {
        a.clone()
    } else if let Some(a) = body.as_array() {
        a.clone()
    } else {
        return Vec::new();
    };
    arr.into_iter()
        .filter_map(|m| {
            let id = m["id"]
                .as_str()
                .or_else(|| m["name"].as_str())
                .or_else(|| m["model"].as_str())
                .map(|s| s.to_string())?;
            Some(FetchedModel {
                id,
                metadata: Some(m),
            })
        })
        .collect()
}

/// Best-effort context-window extraction from model metadata.
pub fn context_from_metadata(v: &serde_json::Value) -> Option<u32> {
    let candidates = [
        v["context_length"].as_u64(),
        v["top_provider"]["context_length"].as_u64(),
        v["architecture"]["max_context_length"].as_u64(),
        v["context_window"].as_u64(),
        v["max_context_length"].as_u64(),
        v["max_input_tokens"].as_u64(),
        v["input_token_limit"].as_u64(),
        v["max_context_window_tokens"].as_u64(),
        v["spec"]["context_length"].as_u64(),
        v["limits"]["max_context_window_tokens"].as_u64(),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|n| *n >= 1024)
        .map(|n| n.min(u32::MAX as u64) as u32)
}

/// Best-effort max-output extraction from model metadata.
pub fn max_output_from_metadata(v: &serde_json::Value) -> Option<u32> {
    let candidates = [
        v["top_provider"]["max_completion_tokens"].as_u64(),
        v["max_output_tokens"].as_u64(),
        v["max_completion_tokens"].as_u64(),
        v["max_tokens"].as_u64(),
        v["output_token_limit"].as_u64(),
        v["limits"]["max_output_tokens"].as_u64(),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|n| *n >= 256)
        .map(|n| n.min(u32::MAX as u64) as u32)
}

/// Does metadata suggest vision support? `None` = no signal.
pub fn vision_hint_from_metadata(v: Option<&serde_json::Value>) -> Option<bool> {
    let v = v?;
    if let Some(b) = v["supports_vision"].as_bool() {
        return Some(b);
    }
    if let Some(b) = v["supports_image_input"].as_bool() {
        return Some(b);
    }
    if let Some(arr) = v["architecture"]["input_modalities"].as_array() {
        return Some(arr.iter().any(|m| m.as_str() == Some("image")));
    }
    if let Some(arr) = v["input_modalities"].as_array() {
        return Some(arr.iter().any(|m| m.as_str() == Some("image")));
    }
    if let Some(arr) = v["supported_input"].as_array() {
        return Some(
            arr.iter()
                .filter_map(|m| m.as_str())
                .any(|s| s.contains("image") || s.contains("png") || s.contains("jpeg")),
        );
    }
    if let Some(arr) = v["capabilities"].as_array() {
        return Some(arr.iter().any(|m| {
            m.as_str()
                .map(|s| s.contains("vision") || s.contains("image"))
                .unwrap_or(false)
        }));
    }
    if v["capabilities"].is_object() {
        return v["capabilities"]["vision"]
            .as_bool()
            .or_else(|| v["capabilities"]["image_input"].as_bool())
            .or(Some(false));
    }
    None
}

/// Which thinking conventions does metadata suggest? Empty = no signal.
pub fn reasoning_hint_from_metadata(v: Option<&serde_json::Value>) -> Vec<String> {
    let Some(v) = v else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(arr) = v["supported_parameters"].as_array() {
        let has = |needle: &str| {
            arr.iter()
                .any(|p| p.as_str().map(|s| s == needle).unwrap_or(false))
        };
        if has("reasoning") || has("include_reasoning") {
            out.push(ProbeSpec::THINKING_OPENROUTER.into());
        }
        if has("reasoning_effort") {
            out.push(ProbeSpec::THINKING_OPENAI.into());
        }
    }
    out
}

// -- Response inspection helpers ---------------------------------------------------

fn resp_text(body: &serde_json::Value) -> String {
    body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn resp_reasoning(body: &serde_json::Value) -> String {
    body["choices"][0]["message"]["reasoning_content"]
        .as_str()
        .or_else(|| body["choices"][0]["message"]["reasoning"].as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn resp_finish(body: &serde_json::Value) -> Option<String> {
    body["choices"][0]["finish_reason"]
        .as_str()
        .map(|s| s.to_string())
}

fn error_message(body: &serde_json::Value) -> String {
    let s = body["error"]["message"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .or_else(|| body["message"].as_str())
        .or_else(|| body["detail"].as_str())
        .unwrap_or("")
        .to_string();
    short_body(&s)
}

/// First number ≥ `min` that appears in `msg` after `phrase`. Accepts
/// thousands separators ("262,144"). Used to read the REAL context cap out
/// of vLLM/NIM-style rejection bodies.
fn number_after(phrase: &str, msg: &str, min: u64) -> Option<u64> {
    let start = msg.find(phrase)? + phrase.len();
    let mut buf = String::new();
    for c in msg[start..].chars() {
        if c.is_ascii_digit() || (c == ',' && !buf.is_empty()) {
            buf.push(c);
        } else if !buf.is_empty() {
            break;
        }
    }
    buf.retain(|c| c != ',');
    buf.parse::<u64>().ok().filter(|n| *n >= min)
}

/// Extract the model's true context-window cap from an over-limit error.
/// vLLM: "This model's maximum context length is 32768 tokens. However, you
/// requested ..." — the cap is cited after "maximum context length is".
/// NIM: "...larger than the maximum allowed: 262144".
fn context_cap_from_error(resp: &serde_json::Value, status: u16) -> Option<u32> {
    if status != 400 && status != 413 {
        return None;
    }
    let msg = error_message(resp).to_ascii_lowercase();
    if msg.is_empty() {
        return None;
    }
    const MIN_PLAUSIBLE: u64 = 1024;
    const MAX_PLAUSIBLE: u64 = 100_000_000;
    let phrases = [
        "maximum context length is ",
        "maximum context length of ",
        "maximum allowed: ",
        "maximum allowed is ",
        "less than or equal to ",
        "at most ",
    ];
    phrases
        .iter()
        .filter_map(|p| number_after(p, &msg, MIN_PLAUSIBLE))
        .find(|n| *n <= MAX_PLAUSIBLE)
        .map(|n| n.min(u32::MAX as u64) as u32)
}

fn short_body(s: &str) -> String {
    let t: String = s.chars().take(120).collect();
    if s.chars().count() > 120 {
        format!("{t}...")
    } else {
        t
    }
}

fn fmt_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn chat_endpoint_of(provider: &ApiProvider) -> String {
    let base = provider.base_url.trim_end_matches('/');
    provider.chat_endpoint_url().replace("{base_url}", base)
}

// -- Context-window tier probing ----------------------------------------------

/// Deterministic marker that an over-limit response carried no parsable cap.
const CONTEXT_CAP_UNPARSED: u32 = u32::MAX;

/// Report the context tier that should be probed next, or None when done.
/// The largest tier that answers OK wins; on rejection, a cap parsed from
/// the error (1 < cap < tier) lets the walk skip tiers it can never pass.
fn next_context_tier(accepted: u32, rejected: u32, cap: u32) -> Option<u32> {
    if accepted != 0 {
        return None; // largest accepted tier already found — done
    }
    CONTEXT_TIERS
        .iter()
        .copied()
        .filter(|t| *t < rejected)
        .find(|t| match cap {
            // No cap known yet, or the gateway refuses to cite one (NIM's
            // LB): test every tier rather than guessing.
            0 | CONTEXT_CAP_UNPARSED => true,
            _ => *t <= cap,
        })
}

/// Choose the context window to register: metadata when present, else the
/// largest accepted tier, else metadata-informed caps, else the old 131K
/// default.
fn pick_context_window(metadata_ctx: Option<u32>, accepted_tier: u32, error_cap: u32) -> u32 {
    if let Some(m) = metadata_ctx {
        return m;
    }
    if accepted_tier != 0 {
        return accepted_tier;
    }
    if error_cap != 0 && error_cap != CONTEXT_CAP_UNPARSED {
        return error_cap;
    }
    131_072
}

/// Measure the context window when metadata didn't say. Walks
/// [CONTEXT_TIERS] largest→smallest; each tier is one tiny completion with
/// `max_tokens = tier - filler`. Over-limit errors distinguish three cases:
/// (a) request echoed as TOO LARGE → tier + margin fits, stop;
/// (b) rejection body cites the real cap → jump past the failed tiers;
/// (c) other 4xx/5xx → treat as over-limit without a cap (keep walking).
/// NIM needs (c): its load balancer rejects oversized prompts without
/// revealing any cap, so each tier must actually be tested.
fn context_tier_probe(
    provider: &ApiProvider,
    model: &str,
    cancel: &AtomicBool,
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    timeout_secs: u64,
) -> (u32, u32) {
    let mut accepted: u32 = 0;
    let mut error_cap: u32 = 0;
    let mut crawler = Crawler::new(tx, ProbeStage::Basic, FRAC_BASIC_CRAWL_END, FRAC_BASIC_END);
    let mut rejected: u32 = u32::MAX; // nothing rejected yet → every tier eligible
    loop {
        if cancel.load(Ordering::Relaxed) {
            return (accepted, error_cap);
        }
        let Some(tier) = next_context_tier(accepted, rejected, error_cap) else {
            break;
        };
        emit(
            tx,
            ProbeStage::Basic,
            crawler.frac(),
            format!("Testing context tier {}", fmt_tokens(tier)),
        );
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": tier - 1,
            "stream": false,
        });
        let mut tick = || crawler.tick();
        let mut ctx = NetCtx::new(cancel, timeout_secs, &mut tick);
        match chat_once(provider, model, &body, &mut ctx) {
            Ok((s, resp)) if s.eq(&200) && resp.get("error").is_none_or(|e| e.is_null()) => {
                accepted = tier;
                break;
            }
            // Over-limit rejections — including HTTP 200 with an error body
            // (seen on some gateways). The error carries no usable prompt
            // count, so keep walking down the tiers.
            Ok((s, resp)) => {
                if let Some(c) = context_cap_from_error(&resp, s) {
                    error_cap = c;
                } else {
                    error_cap = CONTEXT_CAP_UNPARSED;
                }
                if !(400..500).contains(&s) && !s.eq(&200) {
                    // 5xx: no point retesting a failing tier.
                    break;
                }
                rejected = tier;
            }
            Err(_) => {
                // Transport-level failure: no size information, stop probing.
                break;
            }
        }
    }
    (accepted, error_cap)
}

/// Context window and max-output budget for a model that passed the
/// baseline. Context comes from metadata when available, otherwise the
/// tier walk measures it live. Max output is metadata, a measured output
/// floor, or 8K (clamped to the context).
fn context_and_output(
    provider: &ApiProvider,
    model_id: &str,
    metadata_ctx: Option<u32>,
    output_floor: Option<u32>,
    cancel: &AtomicBool,
    tx: &std::sync::mpsc::Sender<ProbeUpdate>,
    timeout_secs: u64,
) -> (u32, Option<u32>) {
    if let Some(m) = metadata_ctx {
        return (m, None);
    }
    emit(
        tx,
        ProbeStage::Basic,
        FRAC_BASIC_CRAWL_END,
        "Metadata has no context size — testing context tiers live (1M → 100K)".into(),
    );
    let (accepted, error_cap) = context_tier_probe(provider, model_id, cancel, tx, timeout_secs);
    let ctx = pick_context_window(metadata_ctx, accepted, error_cap);
    (ctx, output_floor)
}

// -- Raw HTTP helpers (blocking, cancel-aware) --------------------------------------

/// GET a JSON document. Returns (status, parsed body).
fn http_get_json(
    provider: &ApiProvider,
    url: &str,
    ctx: &mut NetCtx,
) -> Result<(u16, serde_json::Value), String> {
    let (host, path, port, use_tls) = super::http::parse_url(url).map_err(|e| e.to_string())?;
    let stream =
        connect_tcp(&host, port, CONNECT_TIMEOUT_SECS).map_err(|e| format!("connect: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(ctx.timeout_secs)));

    let auth_type = autocode_core::helpers::provider_manifest(&provider.kind)
        .and_then(|m| m.auth_type.as_deref())
        .unwrap_or("Bearer");
    let auth = match auth_type {
        "x-api-key" => format!("x-api-key: {}", provider.api_key.as_str()),
        _ => format!("Authorization: Bearer {}", provider.api_key.as_str()),
    };
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: autocode-probe\r\nAccept: application/json\r\n{auth}\r\nConnection: close\r\n\r\n"
    );

    let mut buffer = Vec::with_capacity(8192);
    read_response(
        stream,
        use_tls,
        &host,
        request.as_bytes(),
        &mut buffer,
        METADATA_MAX_BYTES,
        ctx,
    )?;

    let body = split_body(&buffer);
    let status = parse_status(&buffer).ok_or("missing HTTP status line")?;
    if status != 200 {
        return Ok((
            status,
            serde_json::Value::String(String::from_utf8_lossy(&body).to_string()),
        ));
    }
    let text = String::from_utf8_lossy(&body);
    let v = serde_json::from_str::<serde_json::Value>(text.trim())
        .map_err(|e| format!("invalid JSON from /models: {e}"))?;
    Ok((200, v))
}

/// POST a chat-completions request. Returns (status, parsed body or raw text).
fn chat_once(
    provider: &ApiProvider,
    _model: &str,
    body: &serde_json::Value,
    ctx: &mut NetCtx,
) -> Result<(u16, serde_json::Value), String> {
    let url = chat_endpoint_of(provider);
    let (host, path, port, use_tls) = super::http::parse_url(&url).map_err(|e| e.to_string())?;
    let stream =
        connect_tcp(&host, port, CONNECT_TIMEOUT_SECS).map_err(|e| format!("connect: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(ctx.timeout_secs)));

    let payload = serde_json::to_string(body).map_err(|e| e.to_string())?;
    let mut extra: Vec<(&str, &str)> = Vec::new();
    if let Some(prov) = autocode_core::helpers::provider_manifest(&provider.kind) {
        if prov.auth_type.as_deref() == Some("x-api-key") {
            extra.push(("x-api-key", provider.api_key.as_str()));
        }
        if let Some(ver) = &prov.anthropic_version {
            extra.push(("anthropic-version", ver.as_str()));
        }
    }
    let request = build_http_request(&host, &path, provider.api_key.as_str(), &payload, &extra);

    let mut buffer = Vec::with_capacity(8192);
    read_response(
        stream,
        use_tls,
        &host,
        request.as_bytes(),
        &mut buffer,
        1024 * 1024,
        ctx,
    )?;

    let body_bytes = split_body(&buffer);
    let status = parse_status(&buffer).ok_or("missing HTTP status line")?;
    let text = String::from_utf8_lossy(&body_bytes);
    match serde_json::from_str::<serde_json::Value>(text.trim()) {
        Ok(v) => Ok((status, v)),
        Err(_) => Ok((
            status,
            serde_json::Value::String(text.chars().take(2000).collect()),
        )),
    }
}

/// Does `buffer` already hold a complete HTTP response (headers + full
/// body)? Used to salvage responses from gateways that close the TLS
/// connection without close_notify (or RST it) right after answering.
fn response_complete(buffer: &[u8]) -> bool {
    let Some(head_end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false; // headers not even complete
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).to_ascii_lowercase();
    let body = &buffer[head_end + 4..];
    if head.contains("transfer-encoding:") && head.contains("chunked") {
        // Terminated by the zero-length chunk.
        body.windows(5).any(|w| w == b"0\r\n\r\n")
    } else if let Some(cl) = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        body.len() >= cl
    } else {
        // Body delimited by connection close — EOF means complete.
        !body.is_empty()
    }
}

/// Read a full HTTP response (headers + body) with 1s read ticks so the
/// cancel flag is honored mid-read. Decodes chunked bodies. While the
/// response is pending, nudges `ctx.tick` every `CRAWL_TICK` so the UI bar
/// keeps moving.
fn read_response(
    stream: std::net::TcpStream,
    use_tls: bool,
    host: &str,
    request: &[u8],
    buffer: &mut Vec<u8>,
    max_bytes: usize,
    ctx: &mut NetCtx,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(ctx.timeout_secs);

    if use_tls {
        let config = tls_config();
        let dns_name = rustls::pki_types::DnsName::try_from(host.to_string())
            .map_err(|_| "invalid DNS name")?;
        let client = rustls::ClientConnection::new(config, ServerName::DnsName(dns_name))
            .map_err(|e| format!("tls: {e}"))?;
        let mut tls = rustls::StreamOwned::new(client, stream);
        tls.write_all(request).map_err(|e| format!("write: {e}"))?;
        tls.flush().ok();
        read_loop(tls, buffer, max_bytes, deadline, ctx)?;
    } else {
        let mut plain = stream;
        plain
            .write_all(request)
            .map_err(|e| format!("write: {e}"))?;
        plain.flush().ok();
        read_loop(plain, buffer, max_bytes, deadline, ctx)?;
    }

    // Decode chunked transfer encoding in place if needed.
    let head = String::from_utf8_lossy(&buffer[..buffer.len().min(16384)]).to_ascii_lowercase();
    if head.contains("transfer-encoding:") && head.contains("chunked") {
        let split = buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|i| i + 4)
            .unwrap_or(0);
        if split > 0 {
            let body = decode_chunked(&buffer[split..]);
            buffer.truncate(split);
            buffer.extend_from_slice(&body);
        }
    }
    Ok(())
}

/// Fill `buffer` from `io` until a complete response, EOF, the byte cap, or
/// an error. A read error does NOT fail the request when the buffer already
/// holds a complete HTTP response — several gateways (NVIDIA NIM among them)
/// close the TLS connection without close_notify, which rustls reports as
/// `UnexpectedEof` even after a perfectly good response.
fn read_loop<S: Read>(
    mut io: S,
    buffer: &mut Vec<u8>,
    max_bytes: usize,
    deadline: std::time::Instant,
    ctx: &mut NetCtx,
) -> Result<(), String> {
    let mut chunk = [0u8; 8192];
    let mut last_tick = std::time::Instant::now();
    loop {
        if ctx.cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        if std::time::Instant::now() > deadline {
            return Err("timed out".into());
        }
        if last_tick.elapsed() >= CRAWL_TICK {
            last_tick = std::time::Instant::now();
            (ctx.tick)();
        }
        match io.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let take = n.min(max_bytes.saturating_sub(buffer.len()));
                buffer.extend_from_slice(&chunk[..take]);
                if buffer.len() >= max_bytes {
                    break;
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                // Unclean server close after a whole response: use the
                // response instead of failing.
                if response_complete(buffer) {
                    break;
                }
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Err("connection closed by server before the response finished".into());
                }
                return Err(format!("read: {e}"));
            }
        }
    }
    Ok(())
}

fn parse_status(buffer: &[u8]) -> Option<u16> {
    let head = String::from_utf8_lossy(&buffer[..buffer.len().min(1024)]);
    let first = head.lines().next()?;
    let mut parts = first.split_whitespace();
    parts.next()?;
    parts.next()?.parse().ok()
}

fn split_body(buffer: &[u8]) -> Vec<u8> {
    buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| buffer[i + 4..].to_vec())
        .unwrap_or_default()
}

// -- Tests ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_models_openai_shape() {
        let body = serde_json::json!({
            "data": [
                {"id": "m/a", "context_length": 128000},
                {"id": "m/b"}
            ]
        });
        let models = extract_models(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "m/a");
        assert_eq!(
            models[0].metadata.as_ref().unwrap()["context_length"],
            128000
        );
        assert_eq!(models[1].id, "m/b");
    }

    #[test]
    fn extract_models_alt_shapes() {
        let bare = serde_json::json!([{"name": "n1"}, {"id": "n2"}]);
        assert_eq!(extract_models(&bare).len(), 2);
        let wrapped = serde_json::json!({"models": [{"model": "w1"}]});
        assert_eq!(extract_models(&wrapped)[0].id, "w1");
        let junk = serde_json::json!({"error": "nope"});
        assert!(extract_models(&junk).is_empty());
    }

    #[test]
    fn context_metadata_paths() {
        assert_eq!(
            context_from_metadata(&serde_json::json!({"context_length": 40960})),
            Some(40960)
        );
        assert_eq!(
            context_from_metadata(&serde_json::json!({"top_provider": {"context_length": 200000}})),
            Some(200000)
        );
        assert_eq!(
            context_from_metadata(&serde_json::json!({"context_window": 1_048_576})),
            Some(1_048_576)
        );
        assert_eq!(context_from_metadata(&serde_json::json!({"id": "x"})), None);
        // Absurd/tiny values are rejected.
        assert_eq!(
            context_from_metadata(&serde_json::json!({"context_length": 8})),
            None
        );
    }

    #[test]
    fn max_output_metadata_paths() {
        assert_eq!(
            max_output_from_metadata(
                &serde_json::json!({"top_provider": {"max_completion_tokens": 16384}})
            ),
            Some(16384)
        );
        assert_eq!(
            max_output_from_metadata(&serde_json::json!({"max_tokens": 8192})),
            Some(8192)
        );
        assert_eq!(max_output_from_metadata(&serde_json::json!({})), None);
    }

    #[test]
    fn vision_hint_signals() {
        assert_eq!(
            vision_hint_from_metadata(Some(&serde_json::json!(
                {"architecture": {"input_modalities": ["text", "image"]}}
            ))),
            Some(true)
        );
        assert_eq!(
            vision_hint_from_metadata(Some(&serde_json::json!(
                {"architecture": {"input_modalities": ["text"]}}
            ))),
            Some(false)
        );
        assert_eq!(
            vision_hint_from_metadata(Some(&serde_json::json!({"supports_vision": true}))),
            Some(true)
        );
        assert_eq!(
            vision_hint_from_metadata(Some(&serde_json::json!({}))),
            None
        );
        assert_eq!(vision_hint_from_metadata(None), None);
    }

    #[test]
    fn reasoning_hint_signals() {
        assert_eq!(
            reasoning_hint_from_metadata(Some(&serde_json::json!(
                {"supported_parameters": ["reasoning", "include_reasoning"]}
            ))),
            vec!["openrouter".to_string()]
        );
        assert_eq!(
            reasoning_hint_from_metadata(Some(&serde_json::json!(
                {"supported_parameters": ["reasoning_effort"]}
            ))),
            vec!["openai".to_string()]
        );
        assert!(reasoning_hint_from_metadata(Some(&serde_json::json!({}))).is_empty());
    }

    #[test]
    fn thinking_args_per_style() {
        let or = thinking_args(ProbeSpec::THINKING_OPENROUTER, "high");
        assert_eq!(or[0].0, "reasoning");
        assert_eq!(or[0].1["effort"], "high");

        let oa = thinking_args(ProbeSpec::THINKING_OPENAI, "low");
        assert_eq!(oa[0].0, "reasoning_effort");

        let nim = thinking_args(ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS, "high");
        assert_eq!(nim[0].0, "chat_template_kwargs");
        assert_eq!(nim[0].1["enable_thinking"], true);

        assert!(thinking_args(ProbeSpec::THINKING_OFF, "high").is_empty());
    }

    #[test]
    fn thinking_off_args_per_style() {
        let or = thinking_off_args(ProbeSpec::THINKING_OPENROUTER).unwrap();
        assert_eq!(or[0].1["effort"], "none");
        let nim = thinking_off_args(ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS).unwrap();
        assert_eq!(nim[0].1["enable_thinking"], false);
        assert!(thinking_off_args(ProbeSpec::THINKING_OPENAI).is_none());
    }

    // -- Context tiers and error-cap extraction ------------------------------

    #[test]
    fn number_after_parses() {
        let msg = "this model's maximum context length is 262144 tokens. however, you requested 1048600 tokens";
        assert_eq!(
            number_after("maximum context length is ", msg, 1024),
            Some(262144)
        );
        let comma = "larger than the maximum allowed: 262,144 tokens";
        assert_eq!(number_after("maximum allowed: ", comma, 1024), Some(262144));
        // The number must actually FOLLOW the phrase.
        assert_eq!(
            number_after(
                "maximum context length is ",
                "1048600 tokens exceeds the maximum context length is irrelevant",
                1024
            ),
            None
        );
        assert_eq!(number_after("missing: ", "nothing here", 1024), None);
        assert_eq!(
            number_after("maximum allowed: ", "maximum allowed: 512 tokens", 1024),
            None
        );
    }

    #[test]
    fn context_cap_extraction() {
        let vllm = serde_json::json!({
            "error": {"message": "This model's maximum context length is 32768 tokens. However, you requested 200000 tokens (103 in the messages, 199897 in the completion). Please reduce the length of the messages or completion."}
        });
        assert_eq!(context_cap_from_error(&vllm, 400), Some(32768));

        let nim_lb = serde_json::json!({
            "message": "Request is larger than the maximum allowed: 262144"
        });
        assert_eq!(context_cap_from_error(&nim_lb, 400), Some(262144));

        let comma = serde_json::json!({
            "error": "prompt is larger than the maximum allowed: 262,144 tokens"
        });
        assert_eq!(context_cap_from_error(&comma, 400), Some(262144));

        // No cap phrase → None; wrong status → None.
        assert_eq!(
            context_cap_from_error(
                &serde_json::json!({"error": {"message": "auth failed"}}),
                400
            ),
            None
        );
        assert_eq!(context_cap_from_error(&vllm, 200), None);
    }

    #[test]
    fn tier_walk_logic() {
        // No rejections yet → start at the top tier.
        assert_eq!(next_context_tier(0, u32::MAX, 0), Some(1_048_576));
        // Largest tier rejected without a cap → step down one tier.
        assert_eq!(next_context_tier(0, 1_048_576, 0), Some(524_288));
        // Parsed cap below the next tier → the walk skips to the passing one.
        assert_eq!(next_context_tier(0, 1_048_576, 262_144), Some(262_144));
        // Cap 100K: nothing above it can pass.
        assert_eq!(next_context_tier(0, 1_048_576, 100_000), Some(100_000));
        // Unparsed sentinel: no skipping — every tier gets tested.
        assert_eq!(
            next_context_tier(0, 1_048_576, CONTEXT_CAP_UNPARSED),
            Some(524_288)
        );
        // A tier answered OK → walk finished.
        assert_eq!(next_context_tier(262_144, 1_048_576, 0), None);
    }

    #[test]
    fn pick_context_prefers_metadata_then_tier_then_cap() {
        assert_eq!(pick_context_window(Some(40960), 0, 0), 40960);
        assert_eq!(
            pick_context_window(None, 262_144, CONTEXT_CAP_UNPARSED),
            262_144
        );
        // Cap parsed from an error, nothing accepted → cap is the best guess.
        assert_eq!(pick_context_window(None, 0, 32768), 32768);
        // Nothing known anywhere → old conservative default.
        assert_eq!(pick_context_window(None, 0, CONTEXT_CAP_UNPARSED), 131_072);
    }

    #[test]
    fn basic_body_shape() {
        let body = basic_body("test/model", 512, &[]);
        assert_eq!(body["model"], "test/model");
        assert_eq!(body["max_tokens"], 512);
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "user");

        let with_args = basic_body("m", 512, &thinking_args(ProbeSpec::THINKING_OPENAI, "high"));
        assert_eq!(with_args["reasoning_effort"], "high");
    }

    #[test]
    fn vision_body_has_image_part() {
        let body = vision_body("m", 256);
        let parts = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "image_url");
        assert!(
            parts[1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }

    #[test]
    fn response_inspection() {
        let ok = serde_json::json!({
            "choices": [{
                "message": {"content": "OK", "reasoning_content": "hmm"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        assert_eq!(resp_text(&ok), "OK");
        assert_eq!(resp_reasoning(&ok), "hmm");
        assert_eq!(resp_finish(&ok).as_deref(), Some("stop"));

        let len_cut = serde_json::json!({"choices": [{"message": {}, "finish_reason": "length"}]});
        assert_eq!(resp_text(&len_cut), "");
        assert_eq!(resp_finish(&len_cut).as_deref(), Some("length"));
    }

    #[test]
    fn parse_status_and_body_split() {
        let raw = b"HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\n\r\n{\"error\":{\"message\":\"rate limited\"}}";
        assert_eq!(parse_status(raw), Some(429));
        let body = split_body(raw);
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"]["message"], "rate limited");
        assert_eq!(error_message(&v), "rate limited");
    }

    #[test]
    fn fmt_and_short() {
        assert_eq!(fmt_tokens(4096), "4K");
        assert_eq!(fmt_tokens(1_048_576), "1.0M");
        assert_eq!(fmt_tokens(512), "512");
        assert!(short_body(&"x".repeat(300)).ends_with("..."));
    }

    // -- Unclean TLS closes (rustls UnexpectedEof interop) -----------------

    /// Reader that yields `data`, then reports the rustls error real servers
    /// produce when they close a TLS connection without close_notify.
    struct UncleanCloseReader {
        data: Vec<u8>,
        pos: usize,
    }

    impl std::io::Read for UncleanCloseReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos < self.data.len() {
                let n = buf.len().min(self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "peer closed connection without sending TLS close_notify: docs",
                ))
            }
        }
    }

    #[test]
    fn response_complete_shapes() {
        let full = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec();
        assert!(response_complete(&full));
        let truncated = b"HTTP/1.1 200 OK\r\nContent-Length: 200\r\n\r\nok".to_vec();
        assert!(!response_complete(&truncated));
        let headers_only = b"HTTP/1.1 200 OK\r\n".to_vec();
        assert!(!response_complete(&headers_only));
        let chunked =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n0\r\n\r\n".to_vec();
        assert!(response_complete(&chunked));
        let chunked_trunc =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n".to_vec();
        assert!(!response_complete(&chunked_trunc));
        // Close-delimited: any body counts as complete.
        let close_delim = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\npayload".to_vec();
        assert!(response_complete(&close_delim));
        assert!(!response_complete(b""));
    }

    #[test]
    fn read_loop_salvages_complete_response_after_unclean_close() {
        // NVIDIA NIM-style gateway: full 200 answer, then TCP close without
        // TLS close_notify. The response in the buffer must win.
        let body = b"{\"choices\":[]}";
        let raw = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            String::from_utf8_lossy(body)
        );
        let cancel = AtomicBool::new(false);
        let mut noop = || {};
        let mut ctx = NetCtx::new(&cancel, 5, &mut noop);
        let mut buffer = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = read_loop(
            UncleanCloseReader {
                data: raw.into_bytes(),
                pos: 0,
            },
            &mut buffer,
            1024 * 1024,
            deadline,
            &mut ctx,
        );
        assert!(
            result.is_ok(),
            "complete response must be salvaged, got: {result:?}"
        );
        assert_eq!(split_body(&buffer), body.to_vec());
    }

    #[test]
    fn read_loop_reports_truncated_response() {
        // Same unclean close, but the body never arrived — must be an error
        // with a readable message, not a rustls docs URL.
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 500\r\n\r\nshort".to_vec();
        let cancel = AtomicBool::new(false);
        let mut noop = || {};
        let mut ctx = NetCtx::new(&cancel, 5, &mut noop);
        let mut buffer = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = read_loop(
            UncleanCloseReader { data: raw, pos: 0 },
            &mut buffer,
            1024 * 1024,
            deadline,
            &mut ctx,
        );
        assert!(
            matches!(&result, Err(m) if m.contains("connection closed by server")),
            "truncated response must fail with a readable message, got: {result:?}"
        );
    }

    // -- End-to-end probe against a local mock provider ---------------------

    /// A minimal OpenAI-compatible HTTP server on 127.0.0.1:0 serving canned
    /// /models and /chat/completions responses. Lets the probe engine run a
    /// full sequence (metadata → baseline → thinking → vision) over real
    /// sockets without any external service. Returns (base_url, join handle);
    /// the server stops when the handle drops.
    fn spawn_mock_provider(
        models_body: String,
        chat_responder: impl Fn(serde_json::Value) -> (u16, serde_json::Value) + Send + 'static,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            // Keep the listener alive for the life of the thread.
            let _listener = listener;
            let _ = ready_tx.send(());
            while let Ok((stream, _)) = _listener.accept() {
                let responder = &chat_responder;
                let _ = handle_mock_conn(stream, &models_body, responder);
            }
        });
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("mock server failed to start");
        (format!("http://{}", addr), handle)
    }

    fn handle_mock_conn(
        mut stream: std::net::TcpStream,
        models_body: &str,
        chat_responder: &impl Fn(serde_json::Value) -> (u16, serde_json::Value),
    ) -> std::io::Result<()> {
        use std::io::Read;
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        // Read until end of headers, then Content-Length bytes of body.
        loop {
            let n = stream.read(&mut tmp)?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                let content_len: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                if buf.len() >= pos + 4 + content_len {
                    break;
                }
            }
        }
        let text = String::from_utf8_lossy(&buf).to_string();
        let (status, response_body) = if text.starts_with("GET ") {
            (200, models_body.to_string())
        } else {
            // POST: pull the JSON body and run the canned responder.
            let body = text
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
                .unwrap_or(serde_json::Value::Null);
            let (status, v) = chat_responder(body);
            (status, v.to_string())
        };
        let resp = format!(
            "HTTP/1.1 {} mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            response_body.len(),
            response_body
        );
        stream.write_all(resp.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    fn mock_provider() -> ApiProvider {
        let mut p = ApiProvider::new(autocode_core::state::ProviderKind::new("openai-compatible"));
        p.api_key = autocode_core::state::SecretString::new("test-key".into());
        p
    }

    fn wait_done(handle: &ProbeHandle) -> Option<ProbeUpdate> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(u) = handle.poll()
                && u.ok.is_some()
            {
                return Some(u);
            }
            if handle.is_done() || std::time::Instant::now() > deadline {
                return handle.poll();
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Like `wait_done`, but also returns every intermediate update the fast
    /// poll loop happened to catch (poll coalesces, so this is a sample —
    /// enough to assert heartbeats were emitted).
    fn wait_collect(handle: &ProbeHandle) -> Vec<ProbeUpdate> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
        let mut seen = Vec::new();
        loop {
            if let Some(u) = handle.poll() {
                let done = u.ok.is_some();
                seen.push(u);
                if done {
                    return seen;
                }
            }
            if handle.is_done() || std::time::Instant::now() > deadline {
                if let Some(u) = handle.poll() {
                    seen.push(u);
                }
                return seen;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn end_to_end_probe_detects_thinking_and_vision() {
        let models = serde_json::json!({
            "data": [{
                "id": "mock/thinker",
                "context_length": 65536,
                "top_provider": {"max_completion_tokens": 8192},
                "supported_parameters": ["reasoning_effort"],
                "architecture": {"input_modalities": ["text", "image"]}
            }]
        });
        let (base, _server) = spawn_mock_provider(models.to_string(), |body| {
            // Vision probe: content is an array with an image_url part.
            let is_vision = body["messages"][0]["content"].is_array();
            let has_effort = body.get("reasoning_effort").is_some();
            if is_vision {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "a red square"}, "finish_reason": "stop"}]}),
                )
            } else if has_effort {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "ok", "reasoning_content": "pondering"}, "finish_reason": "stop"}]}),
                )
            } else {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "OK"}, "finish_reason": "stop"}]}),
                )
            }
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_OPENAI.into(),
            reasoning_efforts: vec!["low".into(), "high".into()],
            vision_probe: true,
            ..Default::default()
        };
        let handle = probe_model(provider, "mock/thinker".into(), spec);
        let final_update = wait_done(&handle).expect("probe should finish");

        assert_eq!(
            final_update.ok,
            Some(true),
            "summary: {}",
            final_update.message
        );

        let pm = autocode_core::helpers::probed::lookup(
            &autocode_core::state::ProviderKind::new("openai-compatible"),
            "mock/thinker",
        )
        .expect("probed model must be registered");
        assert_eq!(pm.context_window, 65536);
        assert_eq!(pm.max_output_tokens, 8192);
        assert_eq!(pm.thinking_api, "openai");
        assert!(pm.reasoning_efforts.contains(&"low".to_string()));
        assert!(pm.reasoning_efforts.contains(&"high".to_string()));
        assert!(
            pm.supports_vision,
            "metadata said image; probe should confirm"
        );
    }

    #[test]
    fn end_to_end_probe_reports_unreachable_model_as_failure() {
        let models = serde_json::json!({"data": [{"id": "mock/ghost"}]});
        let (base, _server) = spawn_mock_provider(models.to_string(), |_body| {
            (
                404,
                serde_json::json!({"error": {"message": "model not found"}}),
            )
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_OPENAI.into(),
            reasoning_efforts: vec!["high".into()],
            vision_probe: false,
            ..Default::default()
        };
        let handle = probe_model(provider, "mock/ghost".into(), spec);
        let final_update = wait_done(&handle).expect("probe should finish");

        assert_eq!(final_update.ok, Some(false));
        assert!(
            final_update.message.contains("unreachable"),
            "summary should say unreachable: {}",
            final_update.message
        );
        assert!(
            autocode_core::helpers::probed::lookup(
                &autocode_core::state::ProviderKind::new("openai-compatible"),
                "mock/ghost",
            )
            .is_none(),
            "failed probe must NOT register the model"
        );
    }

    #[test]
    fn end_to_end_probe_falls_back_when_no_metadata() {
        // Catalog does not list the model at all — everything must be
        // learned from live requests, with conservative defaults.
        let models = serde_json::json!({"data": [{"id": "other/model"}]});
        let (base, _server) = spawn_mock_provider(models.to_string(), |body| {
            let has_kwargs = body.get("chat_template_kwargs").is_some();
            if has_kwargs {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "ok", "reasoning_content": "thinking"}, "finish_reason": "stop"}]}),
                )
            } else {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "OK"}, "finish_reason": "stop"}]}),
                )
            }
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_CHAT_TEMPLATE_KWARGS.into(),
            reasoning_efforts: vec!["high".into()],
            vision_probe: false,
            ..Default::default()
        };
        let handle = probe_model(provider, "mystery/model".into(), spec);
        let final_update = wait_done(&handle).expect("probe should finish");

        assert_eq!(
            final_update.ok,
            Some(true),
            "summary: {}",
            final_update.message
        );
        let pm = autocode_core::helpers::probed::lookup(
            &autocode_core::state::ProviderKind::new("openai-compatible"),
            "mystery/model",
        )
        .expect("model should register despite missing metadata");
        // Largest tier the mock accepted — the tier walk measures it live
        // when metadata carries no context size.
        assert_eq!(pm.context_window, 1_048_576);
        assert!(pm.max_output_tokens >= 2048);
        // NIM-style convention confirmed but stored as overrides, not "openai".
        assert_eq!(pm.thinking_api, "off");
        assert!(
            pm.thinking_overrides.contains_key("high"),
            "chat_template_kwargs convention should be captured as an override"
        );
    }

    #[test]
    fn end_to_end_probe_retries_through_serverless_cold_start() {
        // First chat request hangs longer than the probe's timeout (model
        // loading); the retry must recover instead of reporting FAILED.
        use std::sync::atomic::AtomicUsize;
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = Arc::clone(&hits);
        let models = serde_json::json!({"data": [{"id": "nim/cold"}]});
        let (base, _server) = spawn_mock_provider(models.to_string(), move |_body| {
            if hits2.fetch_add(1, Ordering::SeqCst) == 0 {
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
            (
                200,
                serde_json::json!({"choices": [{"message": {"content": "OK"}, "finish_reason": "stop"}]}),
            )
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_OFF.into(),
            reasoning_efforts: vec![],
            vision_probe: false,
            timeout_secs: Some(1),
            ..Default::default()
        };
        let handle = probe_model(provider, "nim/cold".into(), spec);
        let updates = wait_collect(&handle);
        let final_update = updates.last().expect("probe should finish");

        assert_eq!(
            final_update.ok,
            Some(true),
            "a cold-start timeout must be retried, not fatal — summary: {}",
            final_update.message
        );
        assert!(
            hits.load(Ordering::SeqCst) >= 2,
            "mock should have seen at least two baseline attempts"
        );
        assert!(
            updates
                .iter()
                .any(|u| u.message.contains("loading") || u.message.contains("retrying")),
            "retry should surface a cold-start message; saw: {:?}",
            updates.iter().map(|u| &u.message).collect::<Vec<_>>()
        );
        assert!(
            autocode_core::helpers::probed::lookup(
                &autocode_core::state::ProviderKind::new("openai-compatible"),
                "nim/cold",
            )
            .is_some(),
            "model must register after successful retry"
        );
    }

    /// Mock gateway whose /models carries no metadata (NIM-style) and which
    /// answers each request with a body built by `responder`. Uses a unique
    /// model id per caller — probed-model registration keys on
    /// (provider, model) and first insert wins for the process lifetime.
    fn spawn_no_metadata_provider(
        model_id: &str,
        responder: impl Fn(serde_json::Value) -> (u16, serde_json::Value) + Send + 'static,
    ) -> (String, std::thread::JoinHandle<()>) {
        spawn_mock_provider(
            serde_json::json!({"data": [{"id": model_id}]}).to_string(),
            responder,
        )
    }

    #[test]
    fn end_to_end_context_tier_walk_down_to_passing_tier() {
        // NIM-style: no metadata, and every prompt over 131_072 tokens is
        // rejected WITHOUT a parsable cap (status 413, opaque wording). The
        // walk must descend until a tier answers; 128K wins.
        let (base, _server) = spawn_no_metadata_provider("ghost/128k", |body| {
            let mt = body["max_tokens"].as_u64().unwrap_or(0) as u32;
            if mt >= 131_072 {
                (
                    413,
                    serde_json::json!({"error": {"message": "request entity too large"}}),
                )
            } else {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "OK"}, "finish_reason": "stop"}]}),
                )
            }
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_OFF.into(),
            reasoning_efforts: vec![],
            vision_probe: false,
            timeout_secs: Some(10),
            ..Default::default()
        };
        let handle = probe_model(provider, "ghost/128k".into(), spec);
        let final_update = wait_done(&handle).expect("probe should finish");
        assert_eq!(final_update.ok, Some(true), "{}", final_update.message);
        let pm = autocode_core::helpers::probed::lookup(
            &autocode_core::state::ProviderKind::new("openai-compatible"),
            "ghost/128k",
        )
        .expect("model should register");
        assert_eq!(
            pm.context_window, 131_072,
            "walk must stop at the largest tier the mock accepts"
        );
        assert!(
            pm.max_output_tokens >= 2048 && pm.max_output_tokens <= 131_072,
            "max output must stay inside the discovered context, got {}",
            pm.max_output_tokens
        );
    }

    #[test]
    fn end_to_end_context_tier_uses_parser_cap_to_skip_tiers() {
        // vLLM-style gateway: the 1M rejection cites the REAL cap (32768),
        // so the walk must jump straight down to 32768 and stop there.
        let (base, _server) = spawn_no_metadata_provider("ghost/vllm-cap", |body| {
            let mt = body["max_tokens"].as_u64().unwrap_or(0) as u32;
            if mt >= 32_768 {
                (
                    400,
                    serde_json::json!({"error": {"message": "This model's maximum context length is 32768 tokens. However, you requested 1048576 tokens (1 in the messages, 1048575 in the completion). Please reduce the length of the messages or completion."}}),
                )
            } else {
                (
                    200,
                    serde_json::json!({"choices": [{"message": {"content": "OK"}, "finish_reason": "stop"}]}),
                )
            }
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_OFF.into(),
            reasoning_efforts: vec![],
            vision_probe: false,
            timeout_secs: Some(10),
            ..Default::default()
        };
        let handle = probe_model(provider, "ghost/vllm-cap".into(), spec);
        let final_update = wait_done(&handle).expect("probe should finish");
        assert_eq!(final_update.ok, Some(true), "{}", final_update.message);
        let pm = autocode_core::helpers::probed::lookup(
            &autocode_core::state::ProviderKind::new("openai-compatible"),
            "ghost/vllm-cap",
        )
        .expect("model should register");
        assert_eq!(
            pm.context_window, 32_768,
            "parsed cap must skip the middle tiers"
        );
    }

    #[test]
    fn end_to_end_probe_crawls_progress_during_slow_request() {
        // A slow (but responsive) model: the bar must creep forward via
        // heartbeats instead of freezing at the stage's start fraction, and
        // fractions must never move backwards.
        let models = serde_json::json!({"data": [{"id": "slow/model"}]});
        let (base, _server) = spawn_mock_provider(models.to_string(), |_body| {
            std::thread::sleep(std::time::Duration::from_millis(2500));
            (
                200,
                serde_json::json!({"choices": [{"message": {"content": "OK"}, "finish_reason": "stop"}]}),
            )
        });
        let mut provider = mock_provider();
        provider.base_url = base;
        let spec = ProbeSpec {
            thinking_style: ProbeSpec::THINKING_OFF.into(),
            reasoning_efforts: vec![],
            vision_probe: false,
            timeout_secs: Some(15),
            ..Default::default()
        };
        let handle = probe_model(provider, "slow/model".into(), spec);
        let updates = wait_collect(&handle);

        assert_eq!(
            updates.last().map(|u| u.ok),
            Some(Some(true)),
            "summary: {:?}",
            updates.last().map(|u| &u.message)
        );
        let heartbeats: Vec<&ProbeUpdate> = updates
            .iter()
            .filter(|u| u.message.starts_with("Still waiting"))
            .collect();
        assert!(
            !heartbeats.is_empty(),
            "slow request must emit crawl heartbeats; saw: {:?}",
            updates
                .iter()
                .map(|u| (&u.fraction, &u.message))
                .collect::<Vec<_>>()
        );
        assert!(
            heartbeats.iter().all(|u| u.fraction > FRAC_BASIC_START),
            "heartbeat fraction must move past the baseline start"
        );
        // The sampled sequence must be non-decreasing overall (allow a
        // sub-percent epsilon for float rounding).
        let mut prev = 0.0f32;
        for u in &updates {
            assert!(
                u.fraction >= prev - 0.011,
                "progress went backwards: {} → {} ({})",
                prev,
                u.fraction,
                u.message
            );
            prev = u.fraction.max(prev);
        }
    }
}
