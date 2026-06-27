# Task: Modular thinking-mode overrides + NVIDIA NIM manifest fix

## Context (read this first, don't skip)

Two separate problems are being fixed in this pass:

1. **Tag leakage.** Some models emit `<think>...</think>` as plain text inside
   the regular content stream instead of using a structured `reasoning_content`
   field. When that happens, the raw tags leak into the visible chat message
   and also get persisted into conversation history, which degrades later
   turns. **Fix:** a small stateful filter in the SSE parser that strips these
   tags regardless of which provider/model is in use. This is a safety net —
   apply it unconditionally, it's a no-op for models that never emit tags.

2. **Wrong wire format per model.** The code currently assumes the `thinking_api`
   enum on a model (`OpenAI`, `Anthropic`, `DeepSeek`, `Gemini`, `Grok`) tells you
   how to ask that model to think. That's true for a model's *native* API, but
   it breaks down for gateways (like NVIDIA NIM) that re-host many model
   families behind one OpenAI-compatible endpoint using their *own* dialect
   (`chat_template_kwargs.enable_thinking` instead of `reasoning_effort`, etc).
   **Fix:** add a per-model `thinking_overrides` map, keyed by effort label
   (or `"off"`), containing the literal JSON to send. When a model has an
   entry for the active key, it wins over the built-in convention. No new
   Rust enum variants needed — this is pure data, added in four places that
   each already do the same thing for other fields (`requests_per_hour`,
   `max_output_tokens_thinking`, etc), so you're extending an existing
   pattern, not inventing a new one.

Do the steps in order. **Run the suggested `cargo check` after each numbered
step** — if a step is wrong, you want to know immediately, not after 8 more
edits stack on top of it. If a struct-literal step is missing a field, the
compiler error will say exactly which file/line is incomplete — trust that
error over guessing.

**Not every step has a compiler safety net.** Steps that add a *struct
field* (Parts B1–B4) will fail loudly if a construction site is missed —
the error tells you exactly where. Steps that add an *enum variant* (Part D)
generally will not, because Rust enums with a wildcard `_` match arm accept
new variants silently. Where a step has no compiler-enforced checkpoint,
these instructions give you a `grep` command instead — treat that as
mandatory, not optional, since nothing else will catch a skipped step until
the manual runtime tests at the very end.

**If `cargo check --workspace` ever fails in a file not mentioned in these
steps** (most likely somewhere in `ui/src/settings/providers.rs`, which
almost certainly references `ThinkingApi` to render a dropdown), that means
something there matches on the enum exhaustively without a wildcard arm.
Fix it the same way Step D1 fixed `label()`/`variants()`: add one arm for
the new variant, following whatever pattern the surrounding arms already
use. This is a normal, expected possibility — not a sign the plan is wrong.

**Out of scope, on purpose:** none of this requires touching `ProviderKind`
(that's a separate type identifying *which* provider, e.g. `"openrouter"`
as a manifest key — already correct and unrelated to this bug) or any
migration of previously-saved app state. Every change here is additive —
new fields with `#[serde(default)]`, a new enum variant — so existing
saved providers/sessions keep loading exactly as before.

---

## PART A — Tag-leak safety net

### Step A1 — Add the filter struct to `ai/src/provider/http.rs`

**Find this exact block:**

```rust
// -- SSE stream parsing --------------------------------------------------------

pub(crate) fn parse_sse_stream_from_reader<R: BufRead>(
```

**Replace with:**

```rust
// -- SSE stream parsing --------------------------------------------------------

/// Splits inline `<think>...</think>` content out of a text stream, even when
/// tags are split across multiple chunks. One instance per stream; state must
/// persist across calls for the lifetime of that stream.
struct ThinkTagFilter {
    in_think: bool,
    carry: String,
}

impl ThinkTagFilter {
    fn new() -> Self {
        Self { in_think: false, carry: String::new() }
    }

    /// Feed raw delta text in; get back (visible_text, reasoning_text).
    /// Either may be empty. Call once per content delta, in order.
    fn process(&mut self, chunk: &str) -> (String, String) {
        self.carry.push_str(chunk);
        let mut visible = String::new();
        let mut reasoning = String::new();

        loop {
            let tag = if self.in_think { "</think>" } else { "<think>" };
            match self.carry.find(tag) {
                Some(idx) => {
                    let (before, after) = self.carry.split_at(idx);
                    if self.in_think {
                        reasoning.push_str(before);
                    } else {
                        visible.push_str(before);
                    }
                    let rest = after[tag.len()..].to_string();
                    self.carry = rest;
                    self.in_think = !self.in_think;
                }
                None => {
                    // No full tag in the buffer yet. Hold back a suffix that
                    // could be the start of a split tag, flush the rest now.
                    let hold = tag.len().saturating_sub(1);
                    let flush_len = self.carry.len().saturating_sub(hold);
                    let flush_len = self.carry.floor_char_boundary(flush_len);
                    let flushed: String = self.carry.drain(..flush_len).collect();
                    if self.in_think {
                        reasoning.push_str(&flushed);
                    } else {
                        visible.push_str(&flushed);
                    }
                    break;
                }
            }
        }
        (visible, reasoning)
    }
}

pub(crate) fn parse_sse_stream_from_reader<R: BufRead>(
```

### Step A2 — Instantiate it per-stream

**Find:**

```rust
    let mut raw_buf = String::new();
    let mut last_log = std::time::Instant::now();
```

**Replace with:**

```rust
    let mut raw_buf = String::new();
    let mut last_log = std::time::Instant::now();
    let mut tag_filter = ThinkTagFilter::new();
```

### Step A3 — Route content deltas through it

**Find:**

```rust
        if let Some(text) = delta["content"].as_str().filter(|s| !s.is_empty())
            && tx.send(ProviderEvent::Delta(text.to_string())).is_err()
        {
            return Err("channel closed".into());
        }
```

**Replace with:**

```rust
        if let Some(text) = delta["content"].as_str().filter(|s| !s.is_empty()) {
            let (visible, reasoning) = tag_filter.process(text);
            if !visible.is_empty() && tx.send(ProviderEvent::Delta(visible)).is_err() {
                return Err("channel closed".into());
            }
            if !reasoning.is_empty() && tx.send(ProviderEvent::Reasoning(reasoning)).is_err() {
                return Err("channel closed".into());
            }
        }
```

### Step A4 — Recognize OpenRouter's reasoning field name

OpenRouter (and possibly other gateways) return reasoning text in a field
named `reasoning`, not `reasoning_content`. Right now only the latter is
read, so reasoning from OpenRouter is silently dropped rather than shown.

**Find:**

```rust
        if let Some(reasoning) = delta["reasoning_content"]
            .as_str()
            .filter(|s| !s.is_empty())
            && tx
                .send(ProviderEvent::Reasoning(reasoning.to_string()))
                .is_err()
        {
            return Err("channel closed".into());
        }
```

**Replace with:**

```rust
        if let Some(reasoning) = delta["reasoning_content"]
            .as_str()
            .filter(|s| !s.is_empty())
            && tx
                .send(ProviderEvent::Reasoning(reasoning.to_string()))
                .is_err()
        {
            return Err("channel closed".into());
        }
        // OpenRouter (and possibly others) use "reasoning" instead of
        // "reasoning_content" as the delta field name.
        if let Some(reasoning) = delta["reasoning"]
            .as_str()
            .filter(|s| !s.is_empty())
            && tx
                .send(ProviderEvent::Reasoning(reasoning.to_string()))
                .is_err()
        {
            return Err("channel closed".into());
        }
```

**Checkpoint:** `cargo check -p autocode-ai` (or `cargo check --workspace` if
you're not sure of the package name). Part A is now done and is fully
independent of Part B below.

---

## PART B — Modular thinking_overrides

The field `thinking_overrides: HashMap<String, serde_json::Value>` gets added
in five places, in this exact order, because each later place is populated
*from* the one before it:

```
manifest.rs (baked-in defaults)
   -> provider_file.rs ModelEntry (user-saved per-model config)
   -> provider.rs ApiProvider (the live, in-memory provider+model state)
   -> types.rs CompletionRequest (what gets handed to the HTTP layer)
   -> client.rs (where it's actually used to build the request body)
```

Add the field to the struct, then add the one-line copy at every place that
already copies its sibling fields (`requests_per_hour`, `thinking_api`, etc).
Match the existing style at each site exactly — don't reformat surrounding
code.

### Step B1 — `core/src/state/manifest.rs`

**Find:**

```rust
#[derive(Deserialize, Clone)]
pub struct ModelManifest {
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub max_output_tokens_thinking: Option<u32>,
    pub thinking_api: String,
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    #[serde(default)]
    pub supports_cache_control: bool,
    /// Max API requests allowed per hour (0 or None = unlimited).
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
}
```

**Replace with:**

```rust
#[derive(Deserialize, Clone)]
pub struct ModelManifest {
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub max_output_tokens_thinking: Option<u32>,
    pub thinking_api: String,
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    #[serde(default)]
    pub supports_cache_control: bool,
    /// Max API requests allowed per hour (0 or None = unlimited).
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
    /// Per-effort raw JSON overrides for gateways with non-standard thinking
    /// knobs (e.g. NVIDIA NIM's chat_template_kwargs). Keyed by effort label,
    /// "off" for the disabled state. When the active key has an entry here,
    /// this JSON is merged into the request body verbatim instead of running
    /// ThinkingApi's built-in convention for that request. Add a new
    /// gateway's quirk here — never in Rust.
    #[serde(default)]
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,
}
```

**Checkpoint:** `cargo check -p autocode-core`. Expect no errors yet — this
struct isn't fully consumed until later steps, but adding a `#[serde(default)]`
field never breaks existing deserialization.

### Step B2 — `core/src/storage/provider_file.rs`

**Find:**

```rust
    /// Rate per hour (0 or None = unlimited).
    #[serde(default)]
    pub requests_per_hour: Option<u32>,

    /// Handoff threshold percentage (10-95). Defaults to 80.
```

**Replace with:**

```rust
    /// Rate per hour (0 or None = unlimited).
    #[serde(default)]
    pub requests_per_hour: Option<u32>,

    /// Per-effort raw JSON overrides for non-standard gateway thinking knobs.
    /// See ModelManifest::thinking_overrides for the full explanation.
    #[serde(default)]
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,

    /// Handoff threshold percentage (10-95). Defaults to 80.
```

Now there are **two** places in this same file that build a `ModelEntry` from
manifest defaults (`defs`). Both need the same one-line addition.

**Find (first occurrence — inside the "no config saved" branch):**

```rust
                } else {
                    // No config saved for this model; use manifest defaults.
                    let defs = model_or_safe(&ap.kind, m_id);
                    result.push(ModelEntry {
                        id: m_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        handoff_percent: ap.handoff_percent,
                        temperature: ap.temperature,
                        top_p: ap.top_p,
                        frequency_penalty: ap.frequency_penalty,
                        presence_penalty: ap.presence_penalty,
                    });
                }
```

**Replace with:**

```rust
                } else {
                    // No config saved for this model; use manifest defaults.
                    let defs = model_or_safe(&ap.kind, m_id);
                    result.push(ModelEntry {
                        id: m_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        thinking_overrides: defs.thinking_overrides.clone(),
                        handoff_percent: ap.handoff_percent,
                        temperature: ap.temperature,
                        top_p: ap.top_p,
                        frequency_penalty: ap.frequency_penalty,
                        presence_penalty: ap.presence_penalty,
                    });
                }
```

**Find (second occurrence — inside the "Legacy" branch, ends with `.collect()` not `;`):**

```rust
            ap.saved_models
                .iter()
                .map(|m_id| {
                    let defs = model_or_safe(&ap.kind, m_id);
                    ModelEntry {
                        id: m_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        handoff_percent: ap.handoff_percent,
                        temperature: ap.temperature,
                        top_p: ap.top_p,
                        frequency_penalty: ap.frequency_penalty,
                        presence_penalty: ap.presence_penalty,
                    }
                })
                .collect()
```

**Replace with:**

```rust
            ap.saved_models
                .iter()
                .map(|m_id| {
                    let defs = model_or_safe(&ap.kind, m_id);
                    ModelEntry {
                        id: m_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        thinking_overrides: defs.thinking_overrides.clone(),
                        handoff_percent: ap.handoff_percent,
                        temperature: ap.temperature,
                        top_p: ap.top_p,
                        frequency_penalty: ap.frequency_penalty,
                        presence_penalty: ap.presence_penalty,
                    }
                })
                .collect()
```

`convert_to_providers` in this same file does **not** need any change — it
already does `models_config.insert(m.id.clone(), m.clone())`, a full clone of
the whole `ModelEntry`, so the new field rides along automatically.

**Checkpoint:** `cargo check -p autocode-core`.

### Step B3 — `core/src/state/provider.rs`

**Find:**

```rust
    /// Max API requests allowed per hour (0 or None = unlimited).
    /// Set from providers.json on first load; user can override in settings.
    #[serde(default)]
    pub requests_per_hour: Option<u32>,

    /// Separate URL for fetching model lists (e.g. "https://api.example.com/v1/models").
```

**Replace with:**

```rust
    /// Max API requests allowed per hour (0 or None = unlimited).
    /// Set from providers.json on first load; user can override in settings.
    #[serde(default)]
    pub requests_per_hour: Option<u32>,

    /// Per-effort raw JSON overrides for non-standard gateway thinking knobs.
    /// See ModelManifest::thinking_overrides for the full explanation.
    #[serde(default)]
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,

    /// Separate URL for fetching model lists (e.g. "https://api.example.com/v1/models").
```

Now three methods need a one-line addition each, mirroring exactly how they
already handle `requests_per_hour`.

**In `new()` — find:**

```rust
            requests_per_hour: defs.requests_per_hour,
            temperature: 0.2,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            models_list_url: models_url,
            saved_models,
            supports_strict_tools_override: None,
            models_config,
        }
    }
```

**Replace with:**

```rust
            requests_per_hour: defs.requests_per_hour,
            thinking_overrides: defs.thinking_overrides.clone(),
            temperature: 0.2,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            models_list_url: models_url,
            saved_models,
            supports_strict_tools_override: None,
            models_config,
        }
    }
```

**In `fill_from_manifest()` — find:**

```rust
        self.thinking_api = parse_thinking_api(&defs.thinking_api);
        self.requests_per_hour = defs.requests_per_hour;
        if let Some(effort) = defs.reasoning_efforts.first() {
            self.reasoning_effort.clone_from(effort);
        }
    }
```

**Replace with:**

```rust
        self.thinking_api = parse_thinking_api(&defs.thinking_api);
        self.requests_per_hour = defs.requests_per_hour;
        self.thinking_overrides = defs.thinking_overrides.clone();
        if let Some(effort) = defs.reasoning_efforts.first() {
            self.reasoning_effort.clone_from(effort);
        }
    }
```

**In `apply_model_entry()` — find:**

```rust
        self.requests_per_hour = mc.requests_per_hour;
        self.handoff_percent = mc.handoff_percent;
```

**Replace with:**

```rust
        self.requests_per_hour = mc.requests_per_hour;
        self.thinking_overrides = mc.thinking_overrides.clone();
        self.handoff_percent = mc.handoff_percent;
```

`reset_defaults()` needs no change — it already calls `fill_from_manifest()`
at the end, which now carries the field for free.

**Checkpoint:** `cargo check -p autocode-core`.

### Step B4 — `ai/src/provider/types.rs`

**Find:**

```rust
    pub thinking_mode: bool,
    pub reasoning_effort: String,
    pub thinking_api: autocode_core::state::ThinkingApi,
    pub top_p: f32,
```

**Replace with:**

```rust
    pub thinking_mode: bool,
    pub reasoning_effort: String,
    pub thinking_api: autocode_core::state::ThinkingApi,
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,
    pub top_p: f32,
```

**Checkpoint:** `cargo check -p autocode-ai`. **This will fail** — that's
expected, because nothing constructs `CompletionRequest` with this field yet.
The error tells you the exact file and line of the struct literal you need to
fix in Step B5. Don't skip ahead — read the error, it does the searching for
you.

### Step B5 — `ai/src/chat/completion.rs` (locate-and-patch)

This file wasn't available when these instructions were written, so use the
compiler error from Step B4 plus this search to find the exact spot(s):

```
grep -n "thinking_api" ai/src/chat/completion.rs
grep -n "CompletionRequest {" ai/src/chat/completion.rs
```

**This file may build `CompletionRequest` in more than one place** — for
example a normal send, and a separate path for auto-continue/handoff. If
either grep returns more than one match, every single one needs the same
fix below — not just the first.

For each match, find the line inside that `CompletionRequest { ... }`
struct literal that looks like:

```rust
thinking_api: <something>.thinking_api.clone(),
```

(`<something>` is whatever variable holds the `ApiProvider` in that
function — likely named `provider`, `prov`, or similar, and may differ
between occurrences; use whatever name is already there in each one.) Add a
new line directly below it using that **same** variable:

```rust
thinking_api: <something>.thinking_api.clone(),
thinking_overrides: <something>.thinking_overrides.clone(),
```

**Checkpoint:** `cargo check -p autocode-ai`. If you see more than one
"missing field `thinking_overrides`" error, that confirms there were
multiple construction sites — fix all of them, then re-run the check. This
must pass clean before moving on.

### Step B6 — `ai/src/provider/client.rs`

Three changes in this file: add a passthrough field to `RequestBody`, accept
the resolved override in `build_request_body`, and resolve it in `run_request`.

**Find (in `ai/src/provider/http.rs`, this is the end of the `RequestBody` struct):**

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'a str>,
}
```

**Replace with:**

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'a str>,
    /// Raw JSON merged into the request body root when a per-model
    /// thinking_overrides entry matches the active effort/off key.
    /// Bypasses the ThinkingApi convention entirely when set.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}
```

**Now in `ai/src/provider/client.rs` — find:**

```rust
    let body = build_request_body(
        &req,
        provider.kind.supports_cache_control(),
        provider.supports_strict_tools(),
    )?;
```

**Replace with:**

```rust
    let thinking_key: &str = if req.thinking_mode {
        req.reasoning_effort.as_str()
    } else {
        "off"
    };
    let thinking_override = req.thinking_overrides.get(thinking_key).cloned();

    let body = build_request_body(
        &req,
        provider.kind.supports_cache_control(),
        provider.supports_strict_tools(),
        thinking_override,
    )?;
```

**Find:**

```rust
fn build_request_body(
    req: &CompletionRequest,
    supports_cache: bool,
    supports_strict: bool,
) -> Result<String, serde_json::Error> {
```

**Replace with:**

```rust
fn build_request_body(
    req: &CompletionRequest,
    supports_cache: bool,
    supports_strict: bool,
    thinking_override: Option<serde_json::Value>,
) -> Result<String, serde_json::Error> {
```

**Find:**

```rust
        thinking: None,
        reasoning_effort: None,
    };

    match &req.thinking_api {
        autocode_core::state::ThinkingApi::DeepSeek if req.thinking_mode => {
            body.thinking = Some(serde_json::json!({"type": "enabled"}));
            body.reasoning_effort = Some(&req.reasoning_effort);
        }
        autocode_core::state::ThinkingApi::OpenAI if req.thinking_mode => {
            body.reasoning_effort = Some(&req.reasoning_effort);
        }
        autocode_core::state::ThinkingApi::Anthropic if req.thinking_mode => {
            body.thinking = Some(serde_json::json!({"type": "enabled", "budget_tokens": 16000}));
        }
        autocode_core::state::ThinkingApi::Gemini if req.thinking_mode => {
            body.thinking = Some(serde_json::json!({"type": "enabled"}));
        }
        autocode_core::state::ThinkingApi::Grok if req.thinking_mode => {
            body.thinking = Some(serde_json::json!({"type": "enabled"}));
        }
        _ => {}
    }
```

**Replace with:**

```rust
        thinking: None,
        reasoning_effort: None,
        extra: None,
    };

    match thinking_override {
        // A manifest-supplied override for the active effort/off key always
        // wins over the built-in convention below.
        Some(v) => {
            body.extra = Some(v);
        }
        None => match &req.thinking_api {
            autocode_core::state::ThinkingApi::DeepSeek if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
                body.reasoning_effort = Some(&req.reasoning_effort);
            }
            autocode_core::state::ThinkingApi::OpenAI if req.thinking_mode => {
                body.reasoning_effort = Some(&req.reasoning_effort);
            }
            autocode_core::state::ThinkingApi::Anthropic if req.thinking_mode => {
                body.thinking =
                    Some(serde_json::json!({"type": "enabled", "budget_tokens": 16000}));
            }
            autocode_core::state::ThinkingApi::Gemini if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
            }
            autocode_core::state::ThinkingApi::Grok if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
            }
            _ => {}
        },
    }
```

**Checkpoint:** `cargo check -p autocode-ai`, then `cargo check --workspace`
to confirm everything still links together end to end.

---

## PART C — Fix `providers.json` for NVIDIA NIM

Replace the entire `"nvidia-nim"` block with the version below. Notes on
what changed and why are inline as a guide, but the actual `providers.json`
file must be valid JSON — **do not include comments in the real file**, only
in your own notes.

```json
"nvidia-nim": {
  "label": "NVIDIA NIM",
  "base_url": "https://integrate.api.nvidia.com/v1",
  "chat_endpoint": "{base_url}/chat/completions",
  "models_endpoint": "{base_url}/models",
  "supports_cache_control": false,
  "supports_parallel_tool_calls": false,
  "supports_strict_tools": false,
  "default_model": "z-ai/glm-5.1",
  "counting_endpoint": "{base_url}/tokenize",
  "models": {
    "z-ai/glm-5.1": {
      "context_window": 200000, "max_output_tokens": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true } },
        "off":  { "chat_template_kwargs": { "enable_thinking": false, "clear_thinking": true } }
      }
    },
    "moonshotai/kimi-k2.6": {
      "context_window": 262144, "max_output_tokens": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true } },
        "off":  { "chat_template_kwargs": { "enable_thinking": false } }
      }
    },
    "deepseek-ai/deepseek-v4-flash": {
      "context_window": 1048576, "max_output_tokens": 65536, "max_output_tokens_thinking": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high", "max"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true, "thinking": true, "reasoning_effort": "high" } },
        "max":  { "chat_template_kwargs": { "enable_thinking": true, "thinking": true, "reasoning_effort": "max" } },
        "off":  { "chat_template_kwargs": { "enable_thinking": false, "thinking": false } }
      }
    },
    "deepseek-ai/deepseek-v4-pro": {
      "context_window": 1048576, "max_output_tokens": 192000, "max_output_tokens_thinking": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high", "max"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true, "thinking": true, "reasoning_effort": "high" } },
        "max":  { "chat_template_kwargs": { "enable_thinking": true, "thinking": true, "reasoning_effort": "max" } },
        "off":  { "chat_template_kwargs": { "enable_thinking": false, "thinking": false } }
      }
    },
    "minimaxai/minimax-m2.7": { "context_window": 204800, "max_output_tokens": 32768, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false, "requests_per_hour": 30 },
    "minimaxai/minimax-m3": { "context_window": 1048576, "max_output_tokens": 256000, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false, "requests_per_hour": 30 },
    "nvidia/nemotron-3-super-120b-a12b": {
      "context_window": 1000000, "max_output_tokens": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true }, "reasoning_budget": 16384 },
        "off":  { "chat_template_kwargs": { "enable_thinking": false } }
      }
    },
    "nvidia/nemotron-3-ultra-550b-a55b": {
      "context_window": 1000000, "max_output_tokens": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true }, "reasoning_budget": 16384 },
        "off":  { "chat_template_kwargs": { "enable_thinking": false } }
      }
    },
    "qwen/qwen3-next-80b-a3b-instruct": {
      "context_window": 262144, "max_output_tokens": 16384,
      "thinking_api": "off", "reasoning_efforts": ["high"],
      "supports_cache_control": false, "requests_per_hour": 30,
      "thinking_overrides": {
        "high": { "chat_template_kwargs": { "enable_thinking": true } },
        "off":  { "chat_template_kwargs": { "enable_thinking": false } }
      }
    },
    "meta/llama-4-maverick-17b-128e-instruct": { "context_window": 1048576, "max_output_tokens": 8192, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false, "requests_per_hour": 30 },
    "mistralai/mistral-medium-3.5-128b": { "context_window": 262144, "max_output_tokens": 16384, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false, "requests_per_hour": 30 }
  }
}
```

**What changed and why:**
- `z-ai/glm-5.1`, `moonshotai/kimi-k2.6`, `qwen/qwen3-next-80b-a3b-instruct`:
  these reason via `chat_template_kwargs.enable_thinking`, not
  `reasoning_effort`. GLM reasons by default, so it needs an explicit `"off"`
  override to ever actually turn off — the others get one too, for symmetry
  and because relying on undocumented defaults is fragile.
- `deepseek-ai/deepseek-v4-flash` / `-pro`: NVIDIA's hosted DeepSeek-v4 wants
  `chat_template_kwargs` with `enable_thinking`/`thinking`/`reasoning_effort`
  all nested together — different from DeepSeek's own native API shape. **This
  exact shape is a best-effort reconstruction from third-party sources, not a
  primary NVIDIA doc** — before relying on it, open `build.nvidia.com`, find
  this model, click "View Code," and confirm the sample request body matches.
  If it doesn't, only this one JSON object needs to change — nothing in Rust.
- `nvidia/nemotron-3-super-120b-a12b` / `nemotron-3-ultra-550b-a55b`: these
  were previously marked `reasoning_efforts: []` with no way to ever enable
  thinking. They're documented hybrid reasoning models — added a `"high"`
  override using `chat_template_kwargs.enable_thinking` plus the top-level
  `reasoning_budget` field NVIDIA's docs show for budget control.
- `minimaxai/*`, `meta/llama-4-maverick*`, `mistralai/*`: left unchanged —
  no evidence these are reasoning-capable on NIM, so there's nothing to fix.

---

## PART D — OpenRouter: real convention, not a per-model override

OpenRouter doesn't pass vendor-native fields (`thinking`, bare
`reasoning_effort`) through to the underlying model. It uses one unified
wrapper — `{"reasoning": {"effort": "..."}}` — regardless of which backend
model you've selected. Because this is uniform across every OpenRouter
model rather than a per-model quirk, it gets a real enum variant instead of
a `thinking_overrides` entry repeated 20+ times.

### Step D1 — `core/src/state/provider.rs`: add the variant

**Find:**

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum ThinkingApi {
    #[default]
    Off,
    DeepSeek,
    OpenAI,
    Anthropic,
    Gemini,
    Grok,
}
```

**Replace with:**

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum ThinkingApi {
    #[default]
    Off,
    DeepSeek,
    OpenAI,
    Anthropic,
    Gemini,
    Grok,
    /// OpenRouter's unified reasoning wrapper: {"reasoning": {"effort": ...}}.
    /// Applies regardless of the underlying model (Anthropic, OpenAI, Gemini,
    /// Grok, DeepSeek, etc) — OpenRouter translates this on their end. Use
    /// this for any model routed through OpenRouter instead of that model's
    /// native convention.
    OpenRouter,
}
```

**Find:**

```rust
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::DeepSeek => "DeepSeek",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::Grok => "Grok",
        }
    }
```

**Replace with:**

```rust
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::DeepSeek => "DeepSeek",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::Grok => "Grok",
            Self::OpenRouter => "OpenRouter",
        }
    }
```

**Find:**

```rust
    pub fn variants() -> &'static [ThinkingApi] {
        &[
            ThinkingApi::Off,
            ThinkingApi::DeepSeek,
            ThinkingApi::OpenAI,
            ThinkingApi::Anthropic,
            ThinkingApi::Gemini,
            ThinkingApi::Grok,
        ]
    }
```

**Replace with:**

```rust
    pub fn variants() -> &'static [ThinkingApi] {
        &[
            ThinkingApi::Off,
            ThinkingApi::DeepSeek,
            ThinkingApi::OpenAI,
            ThinkingApi::Anthropic,
            ThinkingApi::Gemini,
            ThinkingApi::Grok,
            ThinkingApi::OpenRouter,
        ]
    }
```

**Checkpoint:** `cargo check -p autocode-core`. **This passes clean — no
error.** That's not a sign anything's wrong; it's a sign there's no
compiler safety net here. Adding an enum variant only breaks the build if
something matches on it exhaustively with no wildcard arm, and `label()`/
`variants()` (the only two things that did that) were just fixed in this
same step. Nothing else will force you to do Steps D2 and D3 — do them
anyway. A clean `cargo check` after this step means "the variant exists and
is unused," not "you're done."

### Step D2 — locate and patch `parse_thinking_api`

This function lives somewhere under `core/src/helpers/` (imported in
`provider.rs` via `crate::helpers::parse_thinking_api`) but wasn't available
when these instructions were written.

```
grep -rn "fn parse_thinking_api" core/src/
```

It's almost certainly a `match` on a `&str`, with one arm per existing
variant (e.g. `"deepseek" => ThinkingApi::DeepSeek`). Add one more arm:

```rust
"openrouter" => ThinkingApi::OpenRouter,
```

**Checkpoint:** `cargo check -p autocode-core` — passes clean either way,
same reason as Step D1. This step is also not compiler-enforced; verify it
landed with:

```
grep -n "\"openrouter\"" core/src/helpers/*.rs
```

This must print the line you just added. If it prints nothing, the edit
didn't land — go back and add it.

### Step D3 — `ai/src/provider/client.rs`: add the wire-format branch

This goes inside the `None => match &req.thinking_api { ... }` block from
Part B, Step B6. Unlike the other branches, this one has **no `if
req.thinking_mode` guard** — OpenRouter needs an explicit "off" sent too
(see the empty-response failure mode in the context note above), not silent
omission.

**This is the step that actually changes behavior, and it is the
*least* protected by the compiler** — the `match` already has a catch-all
`_ => {}` arm, so a missing or malformed `OpenRouter` branch compiles
perfectly and silently does nothing. Do not skip the verification command
at the end of this step; nothing else will catch it before the manual
runtime test at the very end of this whole document.

**Find:**

```rust
            autocode_core::state::ThinkingApi::Grok if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
            }
            _ => {}
        },
    }
```

**Replace with:**

```rust
            autocode_core::state::ThinkingApi::Grok if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
            }
            autocode_core::state::ThinkingApi::OpenRouter => {
                let effort = if req.thinking_mode {
                    req.reasoning_effort.as_str()
                } else {
                    "none"
                };
                body.extra = Some(serde_json::json!({"reasoning": {"effort": effort}}));
            }
            _ => {}
        },
    }
```

**Checkpoint:** `cargo check -p autocode-ai`, then `cargo check --workspace`
(expect clean, as above — this is not the real check). **Then run this
mandatory verification, since the compiler can't do it for you:**

```
grep -n "ThinkingApi::OpenRouter" ai/src/provider/client.rs
```

This must print exactly one line, showing the new match arm inside
`build_request_body`. If it prints nothing, Step D3 was not applied —
go back and apply it before moving on to Step D4.

### Step D4 — `providers.json`: update the `openrouter` block

Every reasoning-capable model's `thinking_api` changes from its old
vendor-native value to `"openrouter"`. `reasoning_efforts` arrays are kept
as-is where they already exist (OpenRouter accepts `low`/`medium`/`high`/
`max`/`xhigh` directly); two previously-empty arrays (Gemini, Grok) are
filled in so the UI has real options instead of relying on the code's
silent `"high"` fallback. Non-reasoning models (`minimax-*`,
`llama-4-maverick`) are untouched.

```json
"openrouter": {
  "label": "OpenRouter",
  "base_url": "https://openrouter.ai/api/v1",
  "chat_endpoint": "{base_url}/chat/completions",
  "models_endpoint": "{base_url}/models",
  "supports_cache_control": true,
  "supports_parallel_tool_calls": true,
  "supports_strict_tools": true,
  "default_model": "deepseek/deepseek-v4-flash",
  "counting_endpoint": "{base_url}/tokenize",
  "auth_type": "Bearer",
  "models": {
    "anthropic/claude-opus-4.8": { "context_window": 1000000, "max_output_tokens": 128000, "max_output_tokens_thinking": 32000, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "anthropic/claude-opus-4.7": { "context_window": 1000000, "max_output_tokens": 128000, "max_output_tokens_thinking": 32000, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "anthropic/claude-opus-4.6": { "context_window": 1000000, "max_output_tokens": 128000, "max_output_tokens_thinking": 32000, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "anthropic/claude-sonnet-4.6": { "context_window": 1000000, "max_output_tokens": 64000, "max_output_tokens_thinking": 32000, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "anthropic/claude-sonnet-4.5": { "context_window": 200000, "max_output_tokens": 64000, "max_output_tokens_thinking": 16384, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "anthropic/claude-haiku-4.5": { "context_window": 200000, "max_output_tokens": 64000, "max_output_tokens_thinking": 16384, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "deepseek/deepseek-v4-pro": { "context_window": 1048576, "max_output_tokens": 192000, "max_output_tokens_thinking": 16384, "thinking_api": "openrouter", "reasoning_efforts": ["high", "max"], "supports_cache_control": true },
    "deepseek/deepseek-v4-flash": { "context_window": 1048576, "max_output_tokens": 65536, "max_output_tokens_thinking": 16384, "thinking_api": "openrouter", "reasoning_efforts": ["high", "max"], "supports_cache_control": true },
    "minimax/minimax-m3": { "context_window": 1048576, "max_output_tokens": 256000, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false },
    "minimax/minimax-m2.7": { "context_window": 204800, "max_output_tokens": 32768, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false },
    "xiaomi/mimo-v2.5-pro": { "context_window": 1048576, "max_output_tokens": 65536, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "xiaomi/mimo-v2.5": { "context_window": 1048576, "max_output_tokens": 65536, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "moonshotai/kimi-k2.6": { "context_window": 262144, "max_output_tokens": 8192, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "tencent/hy3-preview": { "context_window": 262144, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "openai/gpt-5.5": { "context_window": 400000, "max_output_tokens": 128000, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": true },
    "openai/gpt-5.4": { "context_window": 400000, "max_output_tokens": 128000, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": true },
    "openai/gpt-5.4-mini": { "context_window": 400000, "max_output_tokens": 64000, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": true },
    "openai/gpt-5": { "context_window": 400000, "max_output_tokens": 64000, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": true },
    "openai/o4-mini": { "context_window": 200000, "max_output_tokens": 50000, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "openai/o3": { "context_window": 200000, "max_output_tokens": 50000, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": true },
    "openai/gpt-4.1": { "context_window": 1048576, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": true },
    "google/gemini-2.5-pro": { "context_window": 1048576, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": false },
    "google/gemini-2.5-flash": { "context_window": 1048576, "max_output_tokens": 32767, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": false },
    "stepfun/step-3.7-flash": { "context_window": 262144, "max_output_tokens": 16384, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "qwen/qwen3.7-max": { "context_window": 1000000, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "qwen/qwen3.7-plus": { "context_window": 1000000, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "qwen/qwen3.6-plus": { "context_window": 1000000, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["high"], "supports_cache_control": false },
    "meta-llama/llama-4-maverick": { "context_window": 1048576, "max_output_tokens": 8192, "thinking_api": "off", "reasoning_efforts": [], "supports_cache_control": false },
    "x-ai/grok-4.20": { "context_window": 2000000, "max_output_tokens": 32768, "thinking_api": "openrouter", "reasoning_efforts": ["low", "medium", "high"], "supports_cache_control": false }
  }
}
```

### A note on `openai-compatible` and `opencode-go` (not changed in this pass)

- **`openai-compatible`**: its `base_url` points at real `api.openai.com`,
  and the existing `ThinkingApi::OpenAI` convention (bare `reasoning_effort`,
  no wrapper) is genuinely correct for OpenAI's own Chat Completions API. No
  change needed — but if you ever repoint this entry's `base_url` at a
  different OpenAI-compatible backend (a local vLLM server, another
  aggregator), re-check it the same way NIM and OpenRouter needed checking;
  "OpenAI-compatible" describes the URL shape, not the thinking-control
  dialect.
- **`opencode-go`**: this is a third gateway (`opencode.ai/zen`), and I don't
  have reliable public documentation on its exact reasoning-control shape
  the way OpenRouter's and NIM's are documented. It may well have the same
  problem as NIM/OpenRouter did. Don't guess at a fix here — either find
  OpenCode Zen's own API docs for the `reasoning`/`thinking` parameter shape,
  or capture one real outgoing request (temporarily log `body` in
  `client.rs` before send) and a known-good example from their docs/support,
  and treat that as the next pass.

## Definition of done

- [ ] `cargo check --workspace` passes with zero errors and zero new warnings.
- [ ] `providers.json`'s `nvidia-nim` block matches Part C exactly, and is
      still valid JSON (paste it through any JSON validator if unsure —
      trailing commas will break this).
- [ ] In the running app, select `z-ai/glm-5.1` via NVIDIA NIM, turn the
      thinking toggle **off**, send a message, and confirm no `<think>` tags
      appear in the visible reply and no separate reasoning block is shown.
- [ ] Turn thinking **on** for the same model and confirm a reasoning block
      *does* appear (collapsible/live reasoning UI), and the visible answer
      contains no leftover tags.
- [ ] Send a message through `deepseek-ai/deepseek-v4-flash` with thinking on
      and confirm the request completes (does not hang) — this is the
      regression test for the specific bug this override map was built to fix.
- [ ] Select an Anthropic model via OpenRouter (e.g.
      `anthropic/claude-sonnet-4.6`), turn thinking on, and confirm a
      reasoning block actually appears — this was previously a silent no-op.
- [ ] With the same OpenRouter model, turn thinking off and confirm the
      response comes back promptly with no empty-response retry — this is
      the regression test for the "reasoning eats the whole output budget"
      failure mode.

## Explicitly NOT done in this pass (follow-up work)

- `opencode-go` has not been audited — no reliable public docs were found
  for its reasoning-control wire shape. Likely has the same class of bug as
  NIM/OpenRouter did; needs either real docs or a captured known-good
  request before guessing at a fix. Same `thinking_overrides` or new
  `ThinkingApi` variant mechanism will apply once the real shape is known.
- Any future custom entries under `openai-compatible` pointed at a
  non-OpenAI backend (self-hosted vLLM, etc.) have not been audited.
- The Settings UI (`ui/src/settings/providers.rs`) does not yet expose
  `thinking_overrides` for manual editing. That's fine for now since it's
  populated from baked-in manifest defaults via `fill_from_manifest()`, but
  a user can't hand-author a custom override through the UI yet.
- `max_output_tokens_thinking` budgeting for the newly-enabled Nemotron
  models hasn't been tuned — `16384` is a reasonable starting guess, not a
  measured value.