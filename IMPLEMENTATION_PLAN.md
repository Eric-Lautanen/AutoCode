# AutoCode Implementation Plan (from AUDIT.md §6)

Order: P0 fixes → F2 (T4 baseline first) → F1 sub-agents → F3 phases 1–4.
Every phase compiles and passes the full CI bar (`cargo fmt --check` +
`RUSTFLAGS=-Dwarnings cargo clippy --workspace --all-targets` + `cargo test
--workspace`) plus grep gates before its checkpoint commit.

## Phases

### P0 — Cross-cutting blockers (3 commits)
1. **C1** — `select_provider` overrides the cloned provider's `model` with the
   session's own `model` when non-empty. Single point of fix in
   `ai/chat/completion/provider.rs`; everything downstream (max-token
   derivation via `model_or_safe`, counting-endpoint model, request body)
   follows automatically. Behavior identical while `sess.model == prov.model`.
2. **C8** — forced preflight handoff gate consults the session's own
   `handoff_enabled` (the `session_handoff` param already passed into
   `preflight_context_check`); `check_auto_handoff` and the HANDOFF tool-result
   gates use `handoff_enabled_for(session)`. Agents later get the error path.
3. **C3** — `new_session_for_project` takes a protected-id set (runtime keys +
   active session). Prune picks, among unprotected sessions, a closed one
   oldest-first, else the oldest open; breaks out (allowing >50) rather than
   evicting anything live.

Tests: existing stability suite must pass unchanged; C3 exercised by a unit
test on victim selection order.

### F2 — Throughput (4 commits, T4 first)
4. **T4** — timing log behind `AUTOCODE_TIMING=1`: `[timing]` eprintlns for
   tool-batch wall time, per-tool duration, request start→Done, agent wall
   time. Baseline measurements recorded before T1/T2.
5. **T3** — preflight freshness skip: if ≤ K messages (K=2) appended since the
   Done that reported `actual_tokens_used`, skip the counting call; drop its
   timeout 5s→2s. Pure fn + unit tests.
6. **T2** — permit gate: one thread per in-flight request behind a
   Mutex<usize>+Condvar counter (cap 16); delete `provider/thread_pool.rs`.
   Grep gate: no `ThreadPool`, no `pool()` references remain.
7. **T1** — parallel non-shell tools: pure path-conflict grouping fn
   (`tools/parallel.rs`: normalized absolute path keys; writes/patches/deletes/
   renames touching the same path or rename from/to pairs serialize; reads
   parallel; web/proof independent except verify_proof groups on
   attempts.jsonl), executed on `std::thread::scope` workers
   min(batch, 4), results committed in original tool_call order; fresh
   per-worker LruPathCache merged back after join. Unit tests for grouping +
   order preservation + interleaved write/read serialization.

### F1 — Sub-agents (multiple commits, D1–D10)
8. Storage foundation: `AgentMeta` + status enum in `state/session.rs`;
   `Session.storage_override: Option<PathBuf>` (`#[serde(skip)]`);
   `SessionMeta.agent` with `#[serde(default)]`; override-aware root resolver
   in `session_io.rs`; discovery scans `agents/*/session.json`; C6 lockstep at
   all literal sites. Integration tests: AgentMeta roundtrip, restart sweep
   appends synthetic ToolResult to parent JSONL, agent rename stays inside
   agents/ root, parent-delete cascades.
9. Lifecycle core: spawn/settle/cancel module (`chat/agents.rs`),
   `pending_agents: Vec<AgentHandle>` on runtime, D9 gates (is_busy,
   auto-handoff guard, drain settles children + pushes error ToolResults),
   D10 cap 4 reject-at-cap, D7 seeding via handoff skeleton, D5 tool gating
   (options struct on `tool_definitions`; agents omit spawn_agent/handoff/
   todo_list/project_task_list), D6 per-agent model (rides on C1).
10. UI: agent windows (settings-window pattern), live cards in parent chat,
    tab/dropdown filters, cancel button. Manual scripts from §7.

### F3 — Context attachments (4 commits)
11. Phase 1: wire format (`Content` enum on ReqMsg, `ApiMessage.parts`,
    ContentPart) + capability plumbing (`supports_vision` through manifest/
    provider_file/merge_manifest/assets seed) + serde snapshot tests.
12. Phase 2: staging/storage (`storage/attachments.rs`, ChatMessage.attachments,
    draft_attachments), send_message plumbing with text-only fallback matrix.
13. Phase 3: vision parts assembly in prepare_request_messages + counting-body
    mirror (D6 sync).
14. Phase 4: UI (+ button, drag-drop, chips w/ bounded texture cache ~16,
    history thumbnails).

## Test plan per phase (§7 expanded)
- **P0**: stability suite green; new unit test for prune ordering; manual:
  background session keeps streaming while switching model on another tab.
- **T4**: unit test formatting; manual N=10 scripted batch medians logged.
- **T3**: unit tests for freshness heuristic (0/K/K+1 messages, stale Done);
  manual: observe skipped counting calls in timing log.
- **T2**: grep gate clean; manual: 5 concurrent sessions stream without silent
  queuing.
- **T1**: unit tests for grouping/order/write-read serialization; manual: 8-call
  mixed batch median vs T4 baseline.
- **F1**: integration tests listed in phase 8; unit test tool-def gating;
  manual scripts: two agents streaming while typing in parent (responsive UI);
  send-while-agent-running is a no-op; Stop during agent run → parent resumes
  with error results; kill mid-agent → restart sweep marks Failed + parent JSONL
  gets synthetic ToolResult; cancel button mid-stream → Cancelled persisted;
  cap rejection message at 4 concurrent agents; agent name_session renames
  folder inside agents/.
- **F3**: serde snapshots string-vs-parts byte-identical for legacy shape;
  fallback-matrix builder tests; draft_attachments roundtrip; attachment copy →
  restart restore → session-delete cleanup; manual png drop on vision vs
  non-vision provider.
