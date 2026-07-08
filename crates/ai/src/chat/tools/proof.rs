// proof.rs -- Comprehensive proof checker for the Yang-Mills mass-gap
// Millennium Prize problem (and general mathematical proof verification).
//
// Replaces the former `verify_proof` stub. The checker:
//   1. Auto-detects the target verifier system (Lean / Coq / Z3) from the
//      proof code content when `system = "auto"`.
//   2. Discovers a verifier backend via, in priority order:
//        a. `$AUTOCODE_VERIFIER` env var (path to an executable script), or
//        b. `verify/<system>.sh` / `verify/<system>.cmd` in the project root.
//   3. Writes the proof code to a temp file with the correct extension and
//      invokes the verifier as a subprocess with a hard timeout.
//   4. Captures stdout, stderr, and exit code.
//   5. Parses the verifier output for success/failure markers specific to
//      each backend (Lean's "no goals" / "messages", Coq's "Error" / "No
//      more goals", Z3's "unsat" / "sat" / "unknown").
//   6. Runs structural sanity checks tuned for Yang-Mills mass-gap claims,
//      flagging the two well-known failure modes (Pattern A: redefining
//      axioms outside Wightman/OS; Pattern B: physics-style argument that
//      skips 4D renormalization).
//   7. Appends a full JSONL record of every attempt to
//      `<project_root>/proofs/attempts.jsonl` so the agent can read its
//      prior attempts with the existing read_file/grep tools and avoid
//      repeating dead ends. A compact history summary is also returned in
//      the tool result.
//   8. Returns a single structured, human-readable result string.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Maximum proof-code size we accept (1 MiB). Larger inputs are rejected to
/// avoid pathological temp-file writes and subprocess argument limits.
const MAX_PROOF_BYTES: usize = 1024 * 1024;
/// Default verifier subprocess timeout (seconds). Can be overridden via
/// `$AUTOCODE_VERIFIER_TIMEOUT`.
const DEFAULT_TIMEOUT_SECS: u64 = 120;
/// Hard cap on the verifier timeout to prevent runaway processes.
const MAX_TIMEOUT_SECS: u64 = 600;
/// Maximum bytes of combined stdout+stderr we retain for the result.
const MAX_OUTPUT_BYTES: usize = 16 * 1024;

/// The supported verifier backends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifierSystem {
    Lean,
    Coq,
    Z3,
}

impl VerifierSystem {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lean => "lean",
            Self::Coq => "coq",
            Self::Z3 => "z3",
        }
    }

    /// File extension used for temp proof files.
    fn extension(self) -> &'static str {
        match self {
            Self::Lean => "lean",
            Self::Coq => "v",
            Self::Z3 => "smt2",
        }
    }

    /// Parse a system string from the tool arguments.
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "lean" => Some(Self::Lean),
            "coq" => Some(Self::Coq),
            "z3" => Some(Self::Z3),
            _ => None,
        }
    }
}

/// Outcome of a verification run.
#[derive(Clone, Debug)]
pub enum VerifyOutcome {
    /// Verifier accepted the proof (exit 0 and/or success markers found).
    Verified {
        system: VerifierSystem,
        backend: String,
        duration_ms: u64,
        output: String,
    },
    /// Verifier ran but rejected the proof.
    Rejected {
        system: VerifierSystem,
        backend: String,
        duration_ms: u64,
        exit_code: i32,
        output: String,
        reason: String,
    },
    /// Verifier process timed out.
    Timeout {
        system: VerifierSystem,
        backend: String,
        timeout_secs: u64,
        partial_output: String,
    },
    /// No verifier backend could be found for the requested system.
    NoBackend {
        system: VerifierSystem,
        searched: Vec<String>,
    },
    /// Input was invalid before any verifier ran.
    InvalidInput(String),
    /// Verifier was found but failed to spawn or crashed.
    SpawnError {
        system: VerifierSystem,
        backend: String,
        error: String,
    },
}

impl VerifyOutcome {
    /// One-word status label for the result header.
    pub fn status_label(&self) -> &'static str {
        match self {
            Self::Verified { .. } => "VERIFIED",
            Self::Rejected { .. } => "REJECTED",
            Self::Timeout { .. } => "TIMEOUT",
            Self::NoBackend { .. } => "NO_BACKEND",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::SpawnError { .. } => "SPAWN_ERROR",
        }
    }

    /// The verifier system involved, when known.
    pub fn system(&self) -> Option<VerifierSystem> {
        match self {
            Self::Verified { system, .. }
            | Self::Rejected { system, .. }
            | Self::Timeout { system, .. }
            | Self::NoBackend { system, .. }
            | Self::SpawnError { system, .. } => Some(*system),
            Self::InvalidInput(_) => None,
        }
    }

    /// Backend label, when a backend was involved.
    pub fn backend_label(&self) -> Option<String> {
        match self {
            Self::Verified { backend, .. }
            | Self::Rejected { backend, .. }
            | Self::Timeout { backend, .. }
            | Self::SpawnError { backend, .. } => Some(backend.clone()),
            _ => None,
        }
    }

    /// Subprocess exit code, when available.
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Rejected { exit_code, .. } => Some(*exit_code),
            _ => None,
        }
    }

    /// Wall-clock duration in milliseconds, when available.
    pub fn duration_ms(&self) -> Option<u64> {
        match self {
            Self::Verified { duration_ms, .. }
            | Self::Rejected { duration_ms, .. } => Some(*duration_ms),
            _ => None,
        }
    }

    /// Parsed failure reason, when rejected.
    pub fn reason(&self) -> Option<String> {
        match self {
            Self::Rejected { reason, .. } => Some(reason.clone()),
            _ => None,
        }
    }

    /// Captured verifier output, when available.
    pub fn output(&self) -> Option<String> {
        match self {
            Self::Verified { output, .. }
            | Self::Rejected { output, .. } => Some(output.clone()),
            Self::Timeout { partial_output, .. } => Some(partial_output.clone()),
            _ => None,
        }
    }
}

/// Entry point invoked by the tool dispatcher. `project_root` is the resolved
/// project root (used to locate `verify/` scripts and the `proofs/` log).
/// `args` is the parsed JSON arguments object for the `verify_proof` tool
/// call.
pub fn run_verify_proof(project_root: &str, args: &serde_json::Value) -> String {
    let statement = args["statement"].as_str().unwrap_or("").trim();
    let proof_code = args["proof_code"].as_str().unwrap_or("");
    let system_str = args["system"].as_str().unwrap_or("auto");

    // Resolve the attempts log path up front so we can report history and
    // detect duplicates even on early-exit paths.
    let log_path = attempts_path(project_root);
    let prior_count = log_path
        .as_ref()
        .map(|p| count_attempts(p))
        .unwrap_or(0);
    let hash = proof_hash(proof_code);

    // Duplicate detection: if this exact proof was already submitted, note it
    // in the report but still re-run (the verifier may have changed).
    let dup_status = log_path
        .as_ref()
        .and_then(|p| find_duplicate(p, &hash));

    if statement.is_empty() {
        let outcome = VerifyOutcome::InvalidInput("missing 'statement' argument".into());
        let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
        log_outcome(&log_path, prior_count, statement, "auto", "auto", proof_code, &hash, &outcome, None);
        return report;
    }
    if proof_code.trim().is_empty() {
        let outcome = VerifyOutcome::InvalidInput("missing or empty 'proof_code' argument".into());
        let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
        log_outcome(&log_path, prior_count, statement, "auto", "auto", proof_code, &hash, &outcome, None);
        return report;
    }
    if proof_code.len() > MAX_PROOF_BYTES {
        let outcome = VerifyOutcome::InvalidInput(format!(
            "proof_code is {} bytes; maximum is {} bytes",
            proof_code.len(),
            MAX_PROOF_BYTES
        ));
        let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
        log_outcome(&log_path, prior_count, statement, "auto", "auto", proof_code, &hash, &outcome, None);
        return report;
    }

    // Resolve the target system, auto-detecting when requested.
    let (system, system_source) = if system_str.eq_ignore_ascii_case("auto") {
        match detect_system(proof_code) {
            Some(s) => (s, "auto"),
            None => {
                let outcome = VerifyOutcome::InvalidInput(format!(
                    "system='auto' but the proof language could not be detected. \
                     Set 'system' explicitly to 'lean', 'coq', or 'z3'.\n\
                     Detection heuristics look for: Lean (`theorem`/`lemma`/`by`), \
                     Coq (`Theorem`/`Proof`/`Qed`), Z3 (`(declare-fun`/`(assert`/`(check-sat`)."
                ));
                let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
                log_outcome(&log_path, prior_count, statement, "auto", "auto", proof_code, &hash, &outcome, None);
                return report;
            }
        }
    } else {
        match VerifierSystem::parse(system_str) {
            Some(s) => (s, "explicit"),
            None => {
                let outcome = VerifyOutcome::InvalidInput(format!(
                    "unknown verifier system '{}'. Use 'lean', 'coq', 'z3', or 'auto'.",
                    system_str
                ));
                let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
                log_outcome(&log_path, prior_count, statement, system_str, "explicit", proof_code, &hash, &outcome, None);
                return report;
            }
        }
    };

    // Locate a verifier backend.
    let backend = match find_verifier(project_root, system) {
        Some(b) => b,
        None => {
            let mut searched = Vec::new();
            if let Ok(env_path) = std::env::var("AUTOCODE_VERIFIER") {
                searched.push(format!("$AUTOCODE_VERIFIER={}", env_path));
            }
            let verify_dir = Path::new(project_root).join("verify");
            for ext in script_extensions() {
                searched.push(
                    verify_dir
                        .join(format!("{}{}", system.as_str(), ext))
                        .to_string_lossy()
                        .to_string(),
                );
            }
            searched.push(format!("PATH lookup for `lean`/`coqc`/`z3`"));
            let outcome = VerifyOutcome::NoBackend { system, searched };
            let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
            log_outcome(&log_path, prior_count, statement, system.as_str(), system_source, proof_code, &hash, &outcome, None);
            return report;
        }
    };

    let timeout_secs = resolve_timeout();

    // Write proof to a temp file and run the verifier.
    let outcome = run_verifier(system, &backend, proof_code, timeout_secs);
    let structural = structural_check(statement, proof_code);
    let report = outcome_report(&outcome, statement, proof_code, &log_path, prior_count, &dup_status);
    log_outcome(&log_path, prior_count, statement, system.as_str(), system_source, proof_code, &hash, &outcome, Some(&structural));
    report
}

/// Build a `ProofAttempt` from the outcome and append it to the JSONL log.
fn log_outcome(
    log_path: &Option<PathBuf>,
    prior_count: usize,
    statement: &str,
    system: &str,
    system_source: &str,
    proof_code: &str,
    hash: &str,
    outcome: &VerifyOutcome,
    structural: Option<&StructuralReport>,
) {
    let Some(path) = log_path else { return };
    let attempt = ProofAttempt {
        attempt: prior_count + 1,
        timestamp: autocode_core::helpers::unix_now(),
        statement: statement.to_string(),
        system: system.to_string(),
        system_source: system_source.to_string(),
        proof_code: proof_code.to_string(),
        proof_hash: hash.to_string(),
        status: outcome.status_label().to_string(),
        backend: outcome.backend_label(),
        exit_code: outcome.exit_code(),
        duration_ms: outcome.duration_ms(),
        reason: outcome.reason(),
        output: outcome.output(),
        structural: structural.map(StructuralReportJson::from),
    };
    log_attempt(path, &attempt);
}

// ---------------------------------------------------------------------------
// System detection
// ---------------------------------------------------------------------------

/// Heuristically detect the verifier system from proof-code content.
/// Returns `None` when no backend's signature is clearly present.
fn detect_system(code: &str) -> Option<VerifierSystem> {
    // Score each backend by counting distinctive markers.
    let lean_score = count_markers(
        code,
        &[
            "theorem ", "lemma ", " def ", "theorem\n", "lemma\n", " by ",
            "begin", "end", "instance ", "noncomputable", "axiom ", "Lean",
        ],
    );
    let coq_score = count_markers(
        code,
        &[
            "Theorem ", "Lemma ", "Proof.", "Qed.", "Definition ", "Inductive ",
            "Require Import", "Require Export", "intros", "exact", "apply",
            "rewrite", "destruct", "Coq",
        ],
    );
    let z3_score = count_markers(
        code,
        &[
            "(declare-fun", "(declare-const", "(assert", "(check-sat)",
            "(declare-sort", "(define-fun", "(push)", "(pop)", "(get-model)",
            "set-logic", "set-info",
        ],
    );

    let mut best = None;
    let mut best_score = 0usize;
    for (sys, score) in [
        (VerifierSystem::Lean, lean_score),
        (VerifierSystem::Coq, coq_score),
        (VerifierSystem::Z3, z3_score),
    ] {
        if score > best_score {
            best = Some(sys);
            best_score = score;
        }
    }
    best
}

fn count_markers(haystack: &str, markers: &[&str]) -> usize {
    let mut count = 0;
    for m in markers {
        let mut start = 0;
        while let Some(idx) = haystack[start..].find(m) {
            count += 1;
            start += idx + m.len();
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Verifier discovery
// ---------------------------------------------------------------------------

/// A discovered verifier backend: either a script path or a bare executable
/// name to be looked up on PATH.
#[derive(Clone, Debug)]
struct VerifierBackend {
    /// Human-readable label for the result (e.g. "verify/lean.sh" or "lean (PATH)").
    label: String,
    /// The executable to run.
    program: String,
    /// Whether `program` is a path to a script (true) or a bare command name
    /// to resolve via PATH (false).
    is_script: bool,
}

fn find_verifier(project_root: &str, system: VerifierSystem) -> Option<VerifierBackend> {
    // 1. $AUTOCODE_VERIFIER env var — explicit override, used for any system.
    if let Ok(path) = std::env::var("AUTOCODE_VERIFIER") {
        let p = PathBuf::from(&path);
        if p.is_file() {
            return Some(VerifierBackend {
                label: format!("$AUTOCODE_VERIFIER ({})", path),
                program: path,
                is_script: true,
            });
        }
    }

    // 2. verify/<system>.<ext> in the project root.
    let verify_dir = Path::new(project_root).join("verify");
    for ext in script_extensions() {
        let candidate = verify_dir.join(format!("{}{}", system.as_str(), ext));
        if candidate.is_file() {
            return Some(VerifierBackend {
                label: format!("verify/{}{}", system.as_str(), ext),
                program: candidate.to_string_lossy().to_string(),
                is_script: true,
            });
        }
    }

    // 3. Fall back to the bare executable on PATH.
    let exe = match system {
        VerifierSystem::Lean => "lean",
        VerifierSystem::Coq => "coqc",
        VerifierSystem::Z3 => "z3",
    };
    if which(exe) {
        return Some(VerifierBackend {
            label: format!("{} (PATH)", exe),
            program: exe.to_string(),
            is_script: false,
        });
    }

    None
}

/// Platform-appropriate script extensions to search for in `verify/`.
fn script_extensions() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &[".cmd", ".bat", ".ps1", ".sh"]
    } else {
        &[".sh", ".bash"]
    }
}

/// Lightweight `which` — checks PATH for an executable. Avoids spawning a
/// shell so it works even when `where`/`which` are unavailable.
fn which(exe: &str) -> bool {
    let path_env = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let ext = if cfg!(target_os = "windows") {
        std::env::var_os("PATHEXT").map(|e| {
            e.to_string_lossy()
                .split(';')
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
    } else {
        None
    };
    for dir in std::env::split_paths(&path_env) {
        let candidate = dir.join(exe);
        if candidate.is_file() {
            return true;
        }
        if let Some(exts) = &ext {
            for e in exts {
                let with_ext = candidate.with_extension(e.trim_start_matches('.'));
                if with_ext.is_file() {
                    return true;
                }
            }
        }
    }
    false
}

fn resolve_timeout() -> u64 {
    if let Ok(v) = std::env::var("AUTOCODE_VERIFIER_TIMEOUT") {
        if let Ok(secs) = v.parse::<u64>() {
            return secs.min(MAX_TIMEOUT_SECS);
        }
    }
    DEFAULT_TIMEOUT_SECS
}

// ---------------------------------------------------------------------------
// Verifier execution
// ---------------------------------------------------------------------------

/// Write the proof code to a temp file and invoke the verifier, returning the
/// outcome. Handles timeout via a polling wait on the child process.
fn run_verifier(
    system: VerifierSystem,
    backend: &VerifierBackend,
    proof_code: &str,
    timeout_secs: u64,
) -> VerifyOutcome {
    // Write the proof to a temp file with the correct extension.
    let temp_dir = std::env::temp_dir();
    let id = autocode_core::helpers::generate_id();
    let proof_path = temp_dir.join(format!("ac_proof_{}.{}", id, system.extension()));
    if let Err(e) = write_proof_file(&proof_path, proof_code) {
        return VerifyOutcome::SpawnError {
            system,
            backend: backend.label.clone(),
            error: format!("failed to write temp proof file: {}", e),
        };
    }
    autocode_core::utils::fsutil::track_temp_file(proof_path.clone());

    let proof_str = proof_path.to_string_lossy().to_string();
    let mut cmd = build_command(system, backend, &proof_str);

    // Suppress console windows on Windows.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            cleanup_temp(&proof_path);
            return VerifyOutcome::SpawnError {
                system,
                backend: backend.label.clone(),
                error: e.to_string(),
            };
        }
    };

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    // Poll for completion with timeout.
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                cleanup_temp(&proof_path);
                return VerifyOutcome::SpawnError {
                    system,
                    backend: backend.label.clone(),
                    error: format!("wait failed: {}", e),
                };
            }
        }
    }

    // Collect output. After kill on timeout, wait must be called to reap.
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    let _ = child.wait();

    cleanup_temp(&proof_path);

    let duration_ms = start.elapsed().as_millis() as u64;

    // Combine and cap output.
    let mut combined = String::with_capacity(stdout.len() + stderr.len() + 16);
    if !stdout.is_empty() {
        combined.push_str("--- stdout ---\n");
        combined.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str("--- stderr ---\n");
        combined.push_str(&stderr);
    }
    if combined.is_empty() {
        combined.push_str("(no output)");
    }
    truncate_output(&mut combined, MAX_OUTPUT_BYTES);

    if timed_out {
        return VerifyOutcome::Timeout {
            system,
            backend: backend.label.clone(),
            timeout_secs,
            partial_output: combined,
        };
    }

    let exit_code = child
        .wait()
        .ok()
        .and_then(|s| s.code())
        .unwrap_or(-1);

    // Parse the output for success/failure markers.
    let (verified, reason) = parse_verifier_output(system, exit_code, &combined);

    if verified {
        VerifyOutcome::Verified {
            system,
            backend: backend.label.clone(),
            duration_ms,
            output: combined,
        }
    } else {
        VerifyOutcome::Rejected {
            system,
            backend: backend.label.clone(),
            duration_ms,
            exit_code,
            output: combined,
            reason,
        }
    }
}

/// Build the `Command` for the given backend and proof file path.
fn build_command(
    system: VerifierSystem,
    backend: &VerifierBackend,
    proof_path: &str,
) -> Command {
    if backend.is_script {
        // Run the script directly. On Windows, .cmd/.bat/.ps1 need the shell.
        if cfg!(target_os = "windows")
            && (backend.program.ends_with(".cmd") || backend.program.ends_with(".bat"))
        {
            let mut c = Command::new("cmd");
            c.args(["/C", &backend.program, proof_path]);
            return c;
        }
        if cfg!(target_os = "windows") && backend.program.ends_with(".ps1") {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &backend.program, proof_path]);
            return c;
        }
        // .sh on any platform — run via sh for portability.
        let mut c = Command::new("sh");
        c.args([&backend.program, proof_path]);
        return c;
    }

    // Bare executable on PATH — invoke with the proof file as argument.
    // Z3 reads SMT-LIB2 from stdin when given "-" or a file path directly.
    match system {
        VerifierSystem::Lean => {
            let mut c = Command::new(&backend.program);
            c.arg(proof_path);
            c
        }
        VerifierSystem::Coq => {
            let mut c = Command::new(&backend.program);
            c.arg("-q");
            c.arg(proof_path);
            c
        }
        VerifierSystem::Z3 => {
            let mut c = Command::new(&backend.program);
            c.arg("-smt2");
            c.arg(proof_path);
            c
        }
    }
}

fn write_proof_file(path: &Path, content: &str) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(content.as_bytes())?;
    if !content.ends_with('\n') {
        f.write_all(b"\n")?;
    }
    f.flush()
}

fn cleanup_temp(path: &Path) {
    let _ = autocode_core::utils::fsutil::remove_file(path);
    autocode_core::utils::fsutil::untrack_temp_file(path);
}

fn truncate_output(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let head = max_bytes / 2;
    let tail = max_bytes - head - 40;
    // Find safe UTF-8 boundaries.
    let head_end = s.floor_char_boundary(head.min(s.len()));
    let tail_start = s.ceil_char_boundary(s.len().saturating_sub(tail));
    let omitted = tail_start.saturating_sub(head_end);
    let mut new = String::with_capacity(max_bytes + 64);
    new.push_str(&s[..head_end]);
    new.push_str(&format!("\n[... {} bytes omitted ...]\n", omitted));
    new.push_str(&s[tail_start..]);
    *s = new;
}

// ---------------------------------------------------------------------------
// Output parsing
// ---------------------------------------------------------------------------

/// Parse verifier output to determine success/failure. Returns
/// `(verified, reason)`.
fn parse_verifier_output(system: VerifierSystem, exit_code: i32, output: &str) -> (bool, String) {
    let lower = output.to_ascii_lowercase();

    // Hard failure markers common across systems.
    let hard_errors = ["error:", "syntax error", "type mismatch", "unknown identifier"];
    for he in hard_errors {
        if lower.contains(he) {
            return (false, format!("verifier reported '{}'", he.trim_end_matches(':')));
        }
    }

    match system {
        VerifierSystem::Lean => {
            // Lean 4: success when no errors and no "messages" with severity.
            // A clean compile exits 0. "no goals" indicates a completed proof.
            if exit_code == 0 && !lower.contains("error") {
                if lower.contains("no goals") || lower.contains("goals accomplished") {
                    return (true, "all goals discharged".into());
                }
                // Exit 0 with no error text is a successful type-check.
                return (true, "type-checked with no errors".into());
            }
            (false, format!("lean exited {}", exit_code))
        }
        VerifierSystem::Coq => {
            // Coq: "Qed." succeeds; "Error" / "Cannot find" fail.
            if lower.contains("error") || lower.contains("cannot find") {
                return (false, "coq reported an error".into());
            }
            if exit_code == 0 {
                if lower.contains("no more goals") || lower.contains("qed") {
                    return (true, "proof completed (Qed)".into());
                }
                return (true, "compiled with no errors".into());
            }
            (false, format!("coqc exited {}", exit_code))
        }
        VerifierSystem::Z3 => {
            // Z3: "unsat" = proven (for a negated-goal encoding), "sat" =
            // counterexample, "unknown" = inconclusive.
            if lower.contains("unsat") {
                return (true, "goal is unsatisfiable (proven)".into());
            }
            if lower.contains("sat") {
                return (false, "goal is satisfiable — countermodel found".into());
            }
            if lower.contains("unknown") {
                return (false, "z3 returned 'unknown' — inconclusive".into());
            }
            if exit_code == 0 {
                return (true, "z3 exited 0".into());
            }
            (false, format!("z3 exited {}", exit_code))
        }
    }
}

// ---------------------------------------------------------------------------
// Yang-Mills structural sanity checks
// ---------------------------------------------------------------------------

/// Result of the structural sanity checks for a Yang-Mills mass-gap claim.
#[derive(Clone, Debug, Default)]
pub struct StructuralReport {
    /// True when the statement looks like a Yang-Mills mass-gap claim.
    pub is_yang_mills: bool,
    /// Detected failure-pattern warnings (Pattern A / Pattern B, etc.).
    pub warnings: Vec<String>,
    /// Required-ingredient checklist (each present/absent).
    pub checklist: Vec<(String, bool)>,
}

/// Run structural sanity checks on a claimed Yang-Mills mass-gap proof.
/// These do NOT verify correctness — they flag the well-known failure modes
/// documented in the `yang_mills_mass_gap` skill so the agent does not waste
/// verifier cycles on claims that are structurally incomplete.
pub fn structural_check(statement: &str, proof_code: &str) -> StructuralReport {
    let s_lower = statement.to_ascii_lowercase();
    let p_lower = proof_code.to_ascii_lowercase();
    let is_ym = s_lower.contains("yang")
        && (s_lower.contains("mills") || s_lower.contains("mill"))
        && (s_lower.contains("mass gap") || s_lower.contains("mass-gap"));

    let mut report = StructuralReport {
        is_yang_mills: is_ym,
        warnings: Vec::new(),
        checklist: Vec::new(),
    };

    if !is_ym {
        return report;
    }

    // --- Pattern A: redefining axioms outside Wightman/OS ---------------
    // A legitimate construction must reference the Wightman or
    // Osterwalder-Schrader axioms (or an equivalent rigorous framework).
    let mentions_wightman = p_lower.contains("wightman");
    let mentions_os = p_lower.contains("osterwalder") || p_lower.contains("schrader");
    let mentions_axioms = mentions_wightman || mentions_os;
    report
        .checklist
        .push(("References Wightman or OS axioms".into(), mentions_axioms));

    if !mentions_axioms {
        report.warnings.push(
            "Pattern A risk: no reference to Wightman or Osterwalder-Schrader axioms. \
             A construction that redefines the framework outside these axioms is a \
             known failure mode and will not be accepted by the constructive-QFT community."
                .into(),
        );
    }

    // --- Pattern B: skipping 4D renormalization -------------------------
    // A 4D construction must address renormalization / continuum limit.
    let mentions_renorm = p_lower.contains("renormal")
        || p_lower.contains("continuum limit")
        || p_lower.contains("continuum-limit");
    let mentions_4d = p_lower.contains("4d") || p_lower.contains("four-dimensional") || p_lower.contains("4-dimensional");
    report
        .checklist
        .push(("Addresses 4D renormalization / continuum limit".into(), mentions_renorm));

    if mentions_4d && !mentions_renorm {
        report.warnings.push(
            "Pattern B risk: the claim is 4D but does not address renormalization or the \
             continuum limit. Physics-style arguments that skip 4D renormalization are a \
             known failure mode."
                .into(),
        );
    }

    // --- Required ingredients for a mass-gap proof ----------------------
    let mentions_mass_gap = p_lower.contains("mass gap") || p_lower.contains("δ > 0") || p_lower.contains("delta > 0");
    report
        .checklist
        .push(("States a strictly positive mass gap Δ > 0".into(), mentions_mass_gap));

    let mentions_gauge_group = p_lower.contains("gauge group")
        || p_lower.contains("compact simple")
        || p_lower.contains("su(")
        || p_lower.contains("so(")
        || p_lower.contains("sp(");
    report
        .checklist
        .push(("Specifies a compact simple gauge group".into(), mentions_gauge_group));

    let mentions_construction = p_lower.contains("construct")
        || p_lower.contains("measure")
        || p_lower.contains("stochastic")
        || p_lower.contains("lattice")
        || p_lower.contains("continuum");
    report
        .checklist
        .push(("Provides a constructive definition of the theory".into(), mentions_construction));

    let mentions_spectral = p_lower.contains("spectral")
        || p_lower.contains("transfer operator")
        || p_lower.contains("hamiltonian")
        || p_lower.contains("energy gap");
    report
        .checklist
        .push(("Argues the spectral gap / transfer operator".into(), mentions_spectral));

    // Aggregate missing-ingredient warnings.
    let missing: Vec<&str> = report
        .checklist
        .iter()
        .filter(|(_, present)| !*present)
        .map(|(name, _)| name.as_str())
        .collect();
    if !missing.is_empty() {
        report.warnings.push(format!(
            "Missing ingredients: {}. A complete mass-gap proof should address all of these.",
            missing.join("; ")
        ));
    }

    report
}

// ---------------------------------------------------------------------------
// Attempt logging (JSONL)
// ---------------------------------------------------------------------------

/// A single proof-verification attempt, serialized as one JSONL line into
/// `proofs/attempts.jsonl`. Stores everything the model needs to recall what
/// it tried and why it failed, so it can avoid repeating dead ends across
/// sessions.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ProofAttempt {
    /// Monotonic attempt number (1-based) across all attempts in this file.
    pub attempt: usize,
    /// Unix timestamp (seconds) of the attempt.
    pub timestamp: u64,
    /// The theorem/claim statement being proved.
    pub statement: String,
    /// The verifier system used (lean/coq/z3).
    pub system: String,
    /// How the system was chosen: "auto" (detected) or "explicit".
    pub system_source: String,
    /// The full proof code submitted.
    pub proof_code: String,
    /// SHA-1-style content hash of the proof code (first 16 hex chars) for
    /// cheap dedup detection without reading the whole proof.
    pub proof_hash: String,
    /// Outcome status label: VERIFIED / REJECTED / TIMEOUT / NO_BACKEND /
    /// INVALID_INPUT / SPAWN_ERROR.
    pub status: String,
    /// Verifier backend label, when a backend ran.
    pub backend: Option<String>,
    /// Subprocess exit code, when available.
    pub exit_code: Option<i32>,
    /// Wall-clock duration in milliseconds, when available.
    pub duration_ms: Option<u64>,
    /// Parsed failure reason, when rejected.
    pub reason: Option<String>,
    /// Truncated verifier output (stdout+stderr), capped at MAX_OUTPUT_BYTES.
    pub output: Option<String>,
    /// Yang-Mills structural report, when the statement was a mass-gap claim.
    pub structural: Option<StructuralReportJson>,
}

/// JSON-serializable form of `StructuralReport`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct StructuralReportJson {
    pub is_yang_mills: bool,
    pub warnings: Vec<String>,
    pub checklist: Vec<(String, bool)>,
}

impl From<&StructuralReport> for StructuralReportJson {
    fn from(r: &StructuralReport) -> Self {
        Self {
            is_yang_mills: r.is_yang_mills,
            warnings: r.warnings.clone(),
            checklist: r.checklist.clone(),
        }
    }
}

/// Directory (relative to project root) where proof attempts are logged.
const PROOFS_DIR: &str = "proofs";
/// JSONL file name within the proofs directory.
const ATTEMPTS_FILE: &str = "attempts.jsonl";

/// Compute a lightweight content hash (FNV-1a 64-bit, rendered as 16 hex
/// chars). Not cryptographic — just for dedup detection.
fn proof_hash(code: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in code.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Path to the attempts JSONL file, creating the proofs directory if needed.
fn attempts_path(project_root: &str) -> Option<PathBuf> {
    let dir = Path::new(project_root).join(PROOFS_DIR);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[proof] failed to create proofs dir {}: {}", dir.display(), e);
        return None;
    }
    Some(dir.join(ATTEMPTS_FILE))
}

/// Count existing attempts in the JSONL file (by counting non-empty lines).
/// Returns 0 if the file does not exist or cannot be read.
fn count_attempts(path: &Path) -> usize {
    match std::fs::read_to_string(path) {
        Ok(content) => content.lines().filter(|l| !l.trim().is_empty()).count(),
        Err(_) => 0,
    }
}

/// Append a `ProofAttempt` as one JSONL line. Failure is non-fatal — the
/// verification result is still returned to the caller; we just log a
/// warning to stderr.
fn log_attempt(path: &Path, attempt: &ProofAttempt) {
    // Ensure the parent directory exists (defensive — attempts_path already
    // creates it, but direct callers like tests may not).
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = match serde_json::to_string(attempt) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("[proof] failed to serialize attempt: {}", e);
            return;
        }
    };
    let mut line = json;
    line.push('\n');
    if let Err(e) = append_line(path, &line) {
        eprintln!("[proof] failed to write attempt log {}: {}", path.display(), e);
    }
}

/// Append a string as a new line to a file, creating it if needed. Uses
/// append mode so concurrent tool calls don't clobber each other.
fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(line.as_bytes())?;
    f.flush()
}

/// Compact one-line history summary of prior attempts, for inclusion in the
/// tool result. Example:
///   `Prior attempts: 7 (verified:0 rejected:5 timeout:1 no_backend:1) | last: REJECTED "lean exited 1"`
/// Returns empty string if there are no prior attempts.
fn history_summary(path: &Path) -> String {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let mut total = 0usize;
    let mut verified = 0usize;
    let mut rejected = 0usize;
    let mut timeout = 0usize;
    let mut no_backend = 0usize;
    let mut other = 0usize;
    let mut last_status = String::new();
    let mut last_reason: Option<String> = None;
    let mut last_statement: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        total += 1;
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let status = v["status"].as_str().unwrap_or("");
        last_status = status.to_string();
        match status {
            "VERIFIED" => verified += 1,
            "REJECTED" => rejected += 1,
            "TIMEOUT" => timeout += 1,
            "NO_BACKEND" => no_backend += 1,
            _ => other += 1,
        }
        if let Some(r) = v["reason"].as_str() {
            last_reason = Some(r.to_string());
        }
        if let Some(s) = v["statement"].as_str() {
            last_statement = Some(s.to_string());
        }
    }

    if total == 0 {
        return String::new();
    }

    let mut summary = format!(
        "Prior attempts: {} (verified:{} rejected:{} timeout:{} no_backend:{} other:{})",
        total, verified, rejected, timeout, no_backend, other,
    );
    if !last_status.is_empty() {
        summary.push_str(&format!(" | last: {}", last_status));
        if let Some(r) = &last_reason {
            summary.push_str(&format!(" \"{}\"", truncate_one_line(r, 80)));
        }
        if let Some(s) = &last_statement {
            summary.push_str(&format!(" [{}]", truncate_one_line(s, 60)));
        }
    }
    summary
}

/// Check whether a proof with the given hash has already been attempted
/// (i.e. an exact-duplicate submission). Returns the prior status if found.
fn find_duplicate(path: &Path, hash: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        if v["proof_hash"].as_str() == Some(hash) {
            return v["status"].as_str().map(String::from);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Result formatting
// ---------------------------------------------------------------------------

/// Format a `VerifyOutcome` plus structural report into the tool result string.
fn outcome_report(
    outcome: &VerifyOutcome,
    statement: &str,
    proof_code: &str,
    log_path: &Option<PathBuf>,
    prior_count: usize,
    dup_status: &Option<String>,
) -> String {
    let mut out = String::new();

    let status = outcome.status_label();
    let sys_label = outcome
        .system()
        .map(|s| s.as_str())
        .unwrap_or("unknown");

    out.push_str(&format!("verify_proof: {}\n", status));
    out.push_str(&format!("System: {}\n", sys_label));
    out.push_str(&format!("Statement: {}\n", truncate_one_line(statement, 200)));
    out.push_str(&format!("Proof code: {} chars\n", proof_code.chars().count()));

    // Attempt number for this run.
    out.push_str(&format!("Attempt: #{}\n", prior_count + 1));

    // Duplicate detection note.
    if let Some(prev) = dup_status {
        out.push_str(&format!(
            "NOTE: identical proof code was previously submitted (prior status: {}). Re-running.\n",
            prev
        ));
    }

    // Compact history summary of all prior attempts.
    if let Some(path) = log_path {
        let hist = history_summary(path);
        if !hist.is_empty() {
            out.push_str(&format!("History: {}\n", hist));
        }
    }
    out.push('\n');

    match outcome {
        VerifyOutcome::Verified {
            system: _,
            backend,
            duration_ms,
            output,
        } => {
            out.push_str(&format!("Backend: {}\n", backend));
            out.push_str(&format!("Duration: {} ms\n\n", duration_ms));
            out.push_str("The verifier accepted the proof.\n\n");
            out.push_str("--- Verifier output ---\n");
            out.push_str(output);
        }
        VerifyOutcome::Rejected {
            system: _,
            backend,
            duration_ms,
            exit_code,
            output,
            reason,
        } => {
            out.push_str(&format!("Backend: {}\n", backend));
            out.push_str(&format!("Duration: {} ms\n", duration_ms));
            out.push_str(&format!("Exit code: {}\n", exit_code));
            out.push_str(&format!("Reason: {}\n\n", reason));
            out.push_str("The verifier rejected the proof.\n\n");
            out.push_str("--- Verifier output ---\n");
            out.push_str(output);
        }
        VerifyOutcome::Timeout {
            system: _,
            backend,
            timeout_secs,
            partial_output,
        } => {
            out.push_str(&format!("Backend: {}\n", backend));
            out.push_str(&format!(
                "Timed out after {} s. The verifier was killed.\n\n",
                timeout_secs
            ));
            out.push_str("--- Partial output ---\n");
            out.push_str(partial_output);
        }
        VerifyOutcome::NoBackend { system, searched } => {
            out.push_str(&format!(
                "No verifier backend found for '{}'.\n\nSearched:\n",
                system.as_str()
            ));
            for s in searched {
                out.push_str(&format!("  - {}\n", s));
            }
            out.push_str("\nTo enable verification, do one of:\n");
            out.push_str("  1. Set $AUTOCODE_VERIFIER to a verifier script path, or\n");
            out.push_str("  2. Create verify/<system>.sh (or .cmd on Windows) in the project root, or\n");
            out.push_str("  3. Install the verifier (lean / coqc / z3) on your PATH.\n");
        }
        VerifyOutcome::InvalidInput(msg) => {
            out.push_str(&format!("Invalid input: {}\n", msg));
        }
        VerifyOutcome::SpawnError { system: _, backend, error } => {
            out.push_str(&format!("Backend: {}\n", backend));
            out.push_str(&format!("Failed to spawn verifier: {}\n", error));
        }
    }

    // Append Yang-Mills structural sanity checks when the statement is a
    // mass-gap claim. These run regardless of verifier outcome.
    let report = structural_check(statement, proof_code);
    if report.is_yang_mills {
        out.push_str("\n--- Yang-Mills mass-gap structural check ---\n");
        if report.warnings.is_empty() {
            out.push_str("No structural warnings detected.\n");
        } else {
            out.push_str("Warnings:\n");
            for w in &report.warnings {
                out.push_str(&format!("  ! {}\n", w));
            }
        }
        out.push_str("\nIngredient checklist:\n");
        for (name, present) in &report.checklist {
            let mark = if *present { "[x]" } else { "[ ]" };
            out.push_str(&format!("  {} {}\n", mark, name));
        }
        out.push_str(
            "\nNOTE: Structural checks do NOT verify correctness. They flag the two \
             known failure modes (Pattern A: redefining axioms; Pattern B: skipping 4D \
             renormalization) and required ingredients. A passing structural check is \
             necessary but not sufficient.\n",
        );
    }

    // Tell the model where the full log lives so it can read it with
    // read_file/grep when it needs the complete history.
    if let Some(path) = log_path {
        out.push_str(&format!(
            "\nFull attempt log: {} (read with read_file or grep)\n",
            path.display()
        ));
    }

    out
}

fn truncate_one_line(s: &str, max_chars: usize) -> String {
    let single = s.replace('\n', " ").replace('\r', "");
    if single.chars().count() <= max_chars {
        return single;
    }
    let mut t: String = single.chars().take(max_chars).collect();
    t.push_str("...");
    t
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_lean() {
        let code = "theorem foo : True := by trivial\nlemma bar : True := by trivial";
        assert_eq!(detect_system(code), Some(VerifierSystem::Lean));
    }

    #[test]
    fn test_detect_coq() {
        let code = "Theorem foo : True.\nProof. exact I. Qed.\nRequire Import Coq.Init.";
        assert_eq!(detect_system(code), Some(VerifierSystem::Coq));
    }

    #[test]
    fn test_detect_z3() {
        let code = "(declare-fun x () Int)\n(assert (> x 0))\n(check-sat)";
        assert_eq!(detect_system(code), Some(VerifierSystem::Z3));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_system("hello world"), None);
    }

    #[test]
    fn test_parse_system() {
        assert_eq!(VerifierSystem::parse("lean"), Some(VerifierSystem::Lean));
        assert_eq!(VerifierSystem::parse("COQ"), Some(VerifierSystem::Coq));
        assert_eq!(VerifierSystem::parse("z3"), Some(VerifierSystem::Z3));
        assert_eq!(VerifierSystem::parse("foo"), None);
    }

    #[test]
    fn test_parse_verifier_output_z3_unsat() {
        let (ok, _) = parse_verifier_output(VerifierSystem::Z3, 0, "unsat");
        assert!(ok);
    }

    #[test]
    fn test_parse_verifier_output_z3_sat() {
        let (ok, _) = parse_verifier_output(VerifierSystem::Z3, 0, "sat");
        assert!(!ok);
    }

    #[test]
    fn test_parse_verifier_output_coq_error() {
        let (ok, _) = parse_verifier_output(VerifierSystem::Coq, 1, "Error: foo");
        assert!(!ok);
    }

    #[test]
    fn test_parse_verifier_output_lean_clean() {
        let (ok, _) = parse_verifier_output(VerifierSystem::Lean, 0, "no output");
        assert!(ok);
    }

    #[test]
    fn test_structural_check_non_yang_mills() {
        let r = structural_check("foo bar", "some code");
        assert!(!r.is_yang_mills);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn test_structural_check_yang_mills_pattern_a() {
        let r = structural_check(
            "Yang-Mills mass gap exists",
            "we define a new notion of quantum field theory",
        );
        assert!(r.is_yang_mills);
        // No Wightman/OS reference -> Pattern A warning.
        assert!(r.warnings.iter().any(|w| w.contains("Pattern A")));
    }

    #[test]
    fn test_structural_check_yang_mills_pattern_b() {
        let r = structural_check(
            "Yang-Mills mass gap in 4D",
            "Wightman axioms hold. The 4D theory has a mass gap.",
        );
        assert!(r.is_yang_mills);
        // 4D but no renormalization -> Pattern B warning.
        assert!(r.warnings.iter().any(|w| w.contains("Pattern B")));
    }

    #[test]
    fn test_structural_check_complete() {
        let r = structural_check(
            "Yang-Mills mass gap",
            "Wightman axioms. Osterwalder-Schrader. renormalization. continuum limit. \
             gauge group SU(3) compact simple. construct measure. spectral gap. mass gap Δ > 0.",
        );
        assert!(r.is_yang_mills);
        // All ingredients present -> no missing-ingredient warning.
        assert!(!r.warnings.iter().any(|w| w.contains("Missing ingredients")));
    }

    #[test]
    fn test_truncate_output_short() {
        let mut s = "hello".to_string();
        truncate_output(&mut s, 100);
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_truncate_output_long() {
        let mut s = "a".repeat(10_000);
        truncate_output(&mut s, 1000);
        assert!(s.len() < 1100);
        assert!(s.contains("bytes omitted"));
    }

    #[test]
    fn test_invalid_input_missing_statement() {
        let args = serde_json::json!({"statement": "", "proof_code": "x"});
        let r = run_verify_proof("/tmp", &args);
        assert!(r.contains("INVALID_INPUT"));
        assert!(r.contains("missing 'statement'"));
    }

    #[test]
    fn test_invalid_input_empty_proof() {
        let args = serde_json::json!({"statement": "foo", "proof_code": "  "});
        let r = run_verify_proof("/tmp", &args);
        assert!(r.contains("INVALID_INPUT"));
        assert!(r.contains("proof_code"));
    }

    #[test]
    fn test_no_backend_reports_searched() {
        // Use a temp dir with no verify/ and no verifier on PATH (unlikely
        // to have lean in a test env). We can't guarantee PATH state, so
        // just check the report format when no backend is found by clearing
        // the env var for this test.
        // SAFETY: tests are single-threaded by default; no other thread is
        // reading the environment concurrently.
        unsafe { std::env::remove_var("AUTOCODE_VERIFIER"); }
        let dir = std::env::temp_dir().join(format!("ac_proof_test_{}", autocode_core::helpers::generate_id()));
        let _ = std::fs::create_dir_all(&dir);
        let args = serde_json::json!({
            "statement": "test theorem",
            "proof_code": "theorem x : True := by trivial",
            "system": "lean"
        });
        let r = run_verify_proof(&dir.to_string_lossy(), &args);
        // Either NO_BACKEND (no lean on PATH) or a real outcome (lean present).
        // We only assert the report is well-formed.
        assert!(r.contains("verify_proof:"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_proof_hash_stable() {
        let h1 = proof_hash("theorem x : True := by trivial");
        let h2 = proof_hash("theorem x : True := by trivial");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
        // Different content -> different hash.
        let h3 = proof_hash("theorem y : True := by trivial");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_log_and_history() {
        let dir = std::env::temp_dir().join(format!("ac_proof_log_{}", autocode_core::helpers::generate_id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("proofs").join("attempts.jsonl");

        // No file yet -> empty history.
        assert_eq!(history_summary(&path), "");

        // Log a rejected attempt.
        let attempt = ProofAttempt {
            attempt: 1,
            timestamp: 1000,
            statement: "Yang-Mills mass gap".into(),
            system: "lean".into(),
            system_source: "auto".into(),
            proof_code: "theorem x".into(),
            proof_hash: proof_hash("theorem x"),
            status: "REJECTED".into(),
            backend: Some("lean (PATH)".into()),
            exit_code: Some(1),
            duration_ms: Some(42),
            reason: Some("lean exited 1".into()),
            output: Some("error".into()),
            structural: None,
        };
        log_attempt(&path, &attempt);
        assert_eq!(count_attempts(&path), 1);

        // Log a verified attempt.
        let attempt2 = ProofAttempt {
            attempt: 2,
            timestamp: 2000,
            statement: "trivial theorem".into(),
            system: "z3".into(),
            system_source: "explicit".into(),
            proof_code: "(check-sat)".into(),
            proof_hash: proof_hash("(check-sat)"),
            status: "VERIFIED".into(),
            backend: Some("z3 (PATH)".into()),
            exit_code: Some(0),
            duration_ms: Some(10),
            reason: None,
            output: Some("unsat".into()),
            structural: None,
        };
        log_attempt(&path, &attempt2);
        assert_eq!(count_attempts(&path), 2);

        // History summary should reflect both.
        let hist = history_summary(&path);
        assert!(hist.contains("Prior attempts: 2"));
        assert!(hist.contains("verified:1"));
        assert!(hist.contains("rejected:1"));
        // Last attempt was VERIFIED.
        assert!(hist.contains("last: VERIFIED"));

        // Duplicate detection finds the first attempt by hash.
        let dup = find_duplicate(&path, &proof_hash("theorem x"));
        assert_eq!(dup, Some("REJECTED".to_string()));
        let no_dup = find_duplicate(&path, &proof_hash("totally different"));
        assert_eq!(no_dup, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_attempt_logged_on_invalid_input() {
        let dir = std::env::temp_dir().join(format!("ac_proof_inv_{}", autocode_core::helpers::generate_id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let args = serde_json::json!({"statement": "", "proof_code": "x"});
        let _r = run_verify_proof(&dir.to_string_lossy(), &args);
        // Even invalid input should be logged.
        let path = dir.join("proofs").join("attempts.jsonl");
        assert!(path.exists(), "attempts.jsonl should be created even on invalid input");
        assert_eq!(count_attempts(&path), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_report_includes_attempt_number_and_log_path() {
        let dir = std::env::temp_dir().join(format!("ac_proof_rep_{}", autocode_core::helpers::generate_id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let args = serde_json::json!({"statement": "foo", "proof_code": "  "});
        let r = run_verify_proof(&dir.to_string_lossy(), &args);
        assert!(r.contains("Attempt: #1"));
        assert!(r.contains("attempts.jsonl"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
