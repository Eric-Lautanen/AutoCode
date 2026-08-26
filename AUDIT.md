# AutoCode Audit — Sub-agents, Throughput, Context Attachments

Phase 1 deliverable. Maps every file/function the three features touch, lists design
decisions with options and recommendations, and flags conflicts with existing architecture.
No feature code has been written.

### Hard constraints (bind every design decision below)

1. **Disk stays the single source of truth**, exactly as today: append-only per-session
   JSONL + tiny atomic meta writes; RAM holds display windows only. Every feature stores
   its state in these files or under the session directory — never in side channels.
2. **No new crates.** Everything ships on the existing dependency set (std, serde,
   rustls/webpki-roots in `ai`; egui/eframe/image/rfd in `ui`). Where std lacks a
   primitive (e.g. no `Semaphore`), a minimal hand-rolled Mutex+Condvar gate is used.
3. **Minimal, accurate code**: no `#[allow(...)]` attributes, no `cfg(feature)` gates,
   no dead code, no placeholder TODOs. Any module a change supersedes is deleted, not
   parked (clippy `-Dwarnings` enforces the rest).

---

## 0. Method

Read the full load-bearing surface: `crates/ui/src/app.rs` (frame loop), `crates/ai/src/chat/`
(completion, polling/stream 937 lines, runtime, session, session_ops, polling/tools, polling/shell),
`crates/ai/src/provider/` (client, http incl. ReqMsg/RequestBody + SSE parser, thread_pool,
tool_defs, types, rate_limit), `crates/core/src/state/` (app_state, session, chat, provider,
manifest) and `crates/core/src/storage/` (messages, session_io, session_meta, discovery,
persistence), UI chat panel/input/state/tabs/messages, settings window pattern, CI workflow,
assets/providers.json. Findings below cite file:line where it matters.

## 1. Architecture as-found — invariants the features must respect

1. **One pump thread (the UI thread).** `AutocodeApp::logic` → `chat::update_all(state, runtimes)`
   polls every runtime's channels per frame (`polling/mod.rs:108`). All state mutation is
   single-threaded; workers only produce events into mpsc channels. This is the house pattern.
2. **Disk is truth.** Messages append to `sessions/<id>_<label>/messages.jsonl` via
   rate-limited `pending_writes` drained by a background `PersistenceThread`; meta is tiny
   atomic temp+rename JSON. RAM holds only a display window (`trim_session_ram`,
   default 50). Requests rebuild full history from disk each time
   (`session.rs::prepare_request_messages_for_session`).
3. **Actual-only tokens.** `ProviderEvent::Done{prompt_tokens}` →
   `Session::record_actual_usage` → `persist_session_meta`. Nothing estimates.
4. **Tool batches**: non-shell tools run SEQUENTIALLY on one spawned thread per batch
   (`stream.rs:729-826`); shell calls stream one-at-a-time via `live_shell` channel; results
   commit through `poll_tool_results` / `commit_tool_results` → `push_tool_results_to_state`
   → `start_completion`.
5. **Runtimes are keyed by session id** in `HashMap<String, ChatRuntime>`;
   `update_all` re-keys on handoff and drains zombies whose session vanished
   (`polling/mod.rs:122-137`). Anything that is a real `Session` gets polled for free.
6. **Wire layer is hand-rolled**: `RequestBody`/`ReqMsg` (`http.rs:990-1038`) serialize String
   content; SSE parsing accumulates tool calls by index, repairs truncated args, filters
   `<think>` tags, emits KeepAlive for comment pings. Cancel = drop `CompletionStream`
   (sets an AtomicBool; blocked reads wake every 1s).
7. **Provider pool** is a fixed small ThreadPool, `clamp(2,8)` threads (`client.rs:29-37`);
   each request pins a worker for its entire life (blocking socket reads).
8. **Storage compat discipline**: every persisted struct uses `#[serde(default)]` for new
   fields; `SessionMeta` ↔ `Session` ↔ literal construction in
   `discovery.rs:207-233` must all be updated in lockstep when Session grows fields.

## 2. Cross-cutting findings (affect at least one feature)

| ID | Finding | Impact |
|----|---------|--------|
| C1 | **Per-session model is not actually used for requests.** `select_provider` clones the shared `ApiProvider` and requests go out with `provider.model`. The UI mutates `prov.model` on every session switch/model pick (`ui/chat/session.rs:140-141`, `toolbar/pickers.rs:180-181`, `settings/providers.rs`). A background session retrying after the user switched sessions silently uses the *other session's* model today — latent bug. Sub-agents (per-agent `model?`) make it structural. | F1 blocker |
| C2 | **Tool context snapshots come from the active session**, not the batch's session: `current_todo`/`current_project_tasks` in `stream.rs:726-728` call `state.todo_list()`/`state.project_task_list()` which read the *active* session's/project's disk lists. A background runtime's `todo_list read` returns someone else's list. | F1 (mitigated by excluding task tools from agents); pre-existing |
| C3 | `MAX_SESSIONS` prune does `self.sessions.remove(0)` (`app_state.rs:679-681`) — can evict a live background/agent session mid-run (`still_owns_session` then fails → drain). Must prefer evicting closed sessions with no live runtime. | F1 blocker |
| C4 | **Counting-endpoint preflight blocks the UI thread** up to its 5 s timeout every request build (`preflight.rs:88-137` → `web::native_post`). Throughput + UX issue independent of features. | F2 |
| C5 | Dead code near touched paths: `_per_tool_timeout` computed but unused (`stream.rs:706-713`), empty `if !tc.name.is_empty() {}` (`stream.rs:442`). Clean under the grep gate when those phases touch the file. | hygiene |
| C6 | Adding a `Session` field requires synchronized edits to `state/session.rs`, `storage/session_meta.rs`, `storage/discovery.rs` (literal init), `load_session` — compile-enforced but easy to miss semantics (e.g. `closed: true` on discovery). Checklist item. | F1, F3 |
| C7 | Rate limiter keys on `(provider_label, model)` globally (`rate_limit.rs`) — parallel agents on the same model correctly share the RPH budget but will serialize starts. Document, don't fix. | F1 |
| C8 | **Forced preflight handoff is global-state driven and active-session mutating.** `preflight.rs:54` gates on `state.handoff_enabled` (the global toggle, not the session's) and calls `handle_handoff`, which creates a new session via `new_session_for_project(state.active_project_id)` and repoints `runtime.active_session_id`. Fired inside an agent or background runtime, it would hijack the user's active session and chain handoffs indefinitely. Must become session-scoped: consult the session's own `handoff_enabled`; runtimes without handoff (agents) get the context-exceeded error path instead. | F1 blocker |

---

## 3. Feature 1 — Sub-agents

### 3.1 What gets reused as-is

- Full completion loop: a sub-agent is a normal `Session` + `ChatRuntime`; `update_all`
  polls it automatically once its session id exists in `state.sessions` (invariant 5).
  Stall watchdog, retry/backoff-forever, silent-done recovery, reasoning salvage,
  loop detection all come free per-runtime.
- Persistence: agent transcripts are ordinary `SessionMeta` + `messages.jsonl` pairs,
  nested under the parent at `sessions/<parent>/agents/<id>_<label>/` and written by
  the unchanged `save_session_meta` / append path via an override-aware dir resolver
  (D1). Parent JSONL stores only the spawn_agent call args + final result content
  (the agent's last assistant turn) — satisfies the mission requirement.
- Window chrome: `settings/window.rs` pattern (borderless egui::Window, header frame,
  close-on-outside-click). Live-stream rendering: `show_live_turn` machinery +
  `NetworkStatus` spinner already render exactly "text deltas, reasoning, tool calls,
  status line" from runtime state.

### 3.2 Design decisions

**D1 — Where sub-agent state lives (user-directed revision).**
Agents nest under the parent session's directory, one folder per `spawn_agent` call;
folder naming follows the existing `{id}_{safe_label}` convention and is refined by the
agent's own `name_session` call (sub-agents KEEP `name_session` — it drives their folder
name):

```
projects/<d>/sessions/<parent_id>_<parent_label>/
├── session.json
├── messages.jsonl
└── agents/
    ├── <agent_id>_unnamed/          # initial safe_label fallback
    │   ├── session.json             # SessionMeta incl. agent: AgentMeta
    │   └── messages.jsonl           # full transcript
    └── <agent_id>_<named_label>/    # after the agent calls name_session
```

- `AgentMeta` (`parent_session_id, goal, status: Running|Done|Failed|Cancelled, error,
  started_at, finished_at`) rides in `SessionMeta.agent: Option<AgentMeta>` with
  `#[serde(default)]` — the flag-in-meta requirement, satisfied literally.
- **Registration strategy:** agent Sessions ARE registered in `state.sessions`
  (closed, hidden by UI filters) with a `#[serde(skip)] storage_override:
  Option<PathBuf>` on `Session` holding the parent's `agents/` root. All path
  derivation funnels through an override-aware root resolver used by
  `session_messages_dir` and `save_session_meta`'s rename scan. Rationale: the
  alternative (a separate registry outside `state.sessions`) forces `poll_stream`,
  `push_*`, `persist_session_meta`, and provider selection to grow parallel
  lookup-free variants — high regression risk in a battle-tested 900-line loop for
  zero functional gain. With registration, the entire completion loop, the update
  pump, token persistence, and rate limiting work unchanged; the override is ~4 call
  sites in `session_io.rs`.
- Consequences that come free: `name_session` inside an agent renames its folder
  within the agents root (same atomic move-merge logic as top-level sessions, once the
  root is override-aware); deleting a parent removes its folder tree recursively AND
  drops its agent Sessions from `state.sessions` (one retain by `parent_session_id` in
  the delete path); the restart sweep scans `agents/*/session.json` for
  `status == Running` with no live runtime ⇒ marks Failed("interrupted by app restart").
- **Prune coupling (ordering + resolution):** the agents scan must run inside
  `AppState::load` *before* any prune pass, and `prune_disk_state`'s staleness check
  (step 3, `app_state.rs:484-517`) must resolve directories through the same
  override-aware helper instead of its manual `dir.join(dirname)` + top-level prefix
  scan — otherwise every agent folder reads as missing on the first periodic prune
  (~30 s after launch), the session is evicted from RAM, and `update_all` drains its
  live runtime mid-flight.
- **The sweep must also append a synthetic ToolResult to the PARENT session's JSONL**
  ("[agent interrupted by app restart]", is_error). Reason: if the parent dies
  mid-agent, its JSONL ends with an assistant `spawn_agent` tool_call that has no
  matching tool result — and `prepare_request_messages_for_session` strips exactly such
  orphaned pairs from RAM *and disk* (`chat/session.rs:84-132`) on the next request
  build, silently erasing the record and the pairing. Appending the missing result
  keeps the exchange valid forever; append-only, so the disk-truth discipline is
  untouched.
- Deviation note: the original brief said agent sessions live "under the project like
  normal sessions"; this nests them under the parent instead — same project tree,
  stronger lifecycle coupling (rename/delete move agents with the parent atomically),
  per user direction.

**D2 — Runtime ownership.**
- A: Same `runtimes: HashMap<String, ChatRuntime>`, keyed by agent session id.
- B: Separate AgentManager + event fan-out channel per agent.
Recommendation: **A**. The mission's "fan events to a per-agent UI channel" is already how
the app works — ProviderEvents land in the runtime's channel and egui polls *state*, not
events. Rendering windows straight off the agent's ChatRuntime fields avoids duplicating
state and keeps one source of truth. No new channels needed anywhere in F1.

**D3 — Parent pause semantics (the core integration problem).**
The OpenAI wire format requires an assistant `tool_calls` message to be followed by its
tool results before the next request, so the parent cannot continue until the agent
finishes regardless of API shape. Options:
- A: Model agents like shell calls: `spawn_agent` calls are intercepted in
  `poll_stream` Step 5/6 split, tracked in a new `runtime.pending_agents:
  Vec<AgentHandle>`; when all agents in the batch settle, a ToolResult per agent is
  pushed and the normal `commit_tool_results` → `start_completion` path resumes the parent.
- B: Return "agent started" immediately; deliver output later as a synthetic message.
Recommendation: **A**, strictly — B breaks tool_call/result pairing or forces the same
wait anyway with more state. Multiple `spawn_agent` calls in one batch run concurrently
(F2 payoff); parent commits when the last settles. **Result content = the agent's final
assistant message** — the last turn of its transcript, nothing else crosses into the
parent (no summary format, no structured extraction; read from the agent's RAM buffer
when alive, tail of its `messages.jsonl` when settling after restart).

**D4 — Nesting policy.** Forbid in v1: sub-agent tool sets simply omit `spawn_agent`
(same gating mechanism as `handoff_enabled` in `tool_definitions(strict, handoff_enabled)` —
add a third param or an options struct). Documented in the tool description.

**D5 — Sub-agent tool access.**
Full set minus: `spawn_agent` (D4), `handoff` (meaningless mid-agent; pass
`handoff_enabled=false` so the def is omitted AND auto-handoff can't fire — set the agent
session's `handoff_enabled=false` at creation; C8 closure covers the forced-preflight
path), `todo_list`/`project_task_list` (C2: their read/write plumbing is
active-session-coupled; excluding avoids cross-session clobbering; agents report
progress via final output instead). `name_session` is **kept** — it names the agent's
folder under the parent's agents root (D1) and costs nothing since the handler is
per-runtime. `allow_project_escape` inherited from the parent session's provider config.

**D6 — Per-agent model (depends on fixing C1).**
Fix: `start_completion` resolves the request model from the *session* (`sess.model`),
overriding the cloned provider's model, and derives max-tokens/thinking from that model
via `model_or_safe` (already parameterized by model everywhere it matters). The shared
`prov.model` remains toolbar working-state only. `spawn_agent(model?)` then just sets the
new session's `model` + `provider_label`; unknown models fall back to the parent's.

**D7 — Seeding via the handoff pattern (user-directed).**
Spawn mirrors `handle_handoff`'s session-seeding skeleton (`completion/mod.rs:328-341`):
build the identical system prompt (system prompt + HOST ENVIRONMENT + project context,
requiring `sysinfo::is_ready()`; if not ready, defer the first completion one frame
exactly like `pending_start`), then deliver the agent's brief as a **simulated USER
message** — goal, optional context, and the return contract ("your final response is
returned verbatim to the caller as the tool result"). No system-preamble edits, no
second prompt to maintain; agents inherit every tool-judgment rule from the stock
system prompt, exactly as handoff sessions do.

**D8 — Agent card in parent chat.**
History-safe approach: the committed assistant message with the `spawn_agent` tool_call +
the committed Tool result (final output) already persist everything; the live card is
rendered in the parent's live-turn area (like `live_tool_call` preview) listing running
agents (id, goal, elapsed, status), plus a static card renderer over the committed pair
for history. Clicking opens the window. No mutable placeholder messages — append-only
JSONL stays honest. `file_path` in the result's ToolMeta carries the agent session id so
history cards can link without new ChatMessage fields (alternative: dedicated field —
rejected as unnecessary). The agent window itself renders committed history by loading
the agent's own `session.json` + `messages.jsonl` from its folder under the parent's
agents root — disk stays the source of truth for everything but the live tail, which
comes from the runtime's stream buffers; no agent state is ever held outside that
folder.

**D9 — Lifecycle, cancellation, and busy-gating (three integration points that are easy
to miss).**
States Running→Done|Failed|Cancelled persisted in `AgentMeta` via the normal atomic
`save_session_meta` path. Cancel button in the agent window: `stopped_by_user = true;
drain();` mark Cancelled, push ToolResult "[agent cancelled]" (is_error) to parent,
resume parent.

Beyond the button, the parent's existing gates must learn about pending agents — today
they key only on `stream_rx || tool_rx || live_shell_rx`:

1. `ChatRuntime::is_busy()` (`runtime.rs:229`) must include `!self.pending_agents.is_empty()`.
   Without it, `send_message` accepts new input while an agent batch is unresolved,
   inserting a user message between assistant tool_calls and their tool results — a
   provider-side orphaned-tool-calls error.
2. `check_auto_handoff` (`completion/mod.rs:460`) guards on `live_shell_rx.is_some()`;
   add the same guard for pending agents so auto-handoff can't fire mid-wait.
3. Parent `drain()` (Stop button, replay, handoff, session delete) must settle pending
   agents: cancel each running child (child drain + kill shells), persist Cancelled/
   Failed, and push error ToolResults for every unsettled `spawn_agent` call — same
   pattern `handle_handoff` already uses for `pending_tool_remaining`
   (`completion/mod.rs:284-300`). App exit drains all runtimes; the D1 startup sweep
   converts anything still marked Running into Failed("interrupted by app restart").
4. **C8 closure**: agents never handoff — not via `handoff_enabled=false` alone, because
   the forced preflight path (`preflight.rs:54`) checks the *global* flag. The fix makes
   that path consult the session's own flag; an agent runtime exceeding its context
   window takes the error branch, which surfaces as a failed agent + error ToolResult to
   the parent instead of hijacking the active session.

**D10 — Concurrency cap.** Reject-at-cap (error ToolResult telling the model to wait),
default 4 concurrent agents, no queue ("no cleverness", bounded RAM: each agent ≈ one
socket thread + response buffers).

### 3.3 File/function map (F1)

| File | Change |
|------|--------|
| `crates/core/src/state/session.rs` | `AgentMeta` type (+status enum); `Session.storage_override: Option<PathBuf>` (`#[serde(skip)]` — C6 lockstep edits still compile-enforced at the 4 literal sites) |
| `crates/core/src/storage/session_meta.rs` | `agent: Option<AgentMeta>` w/ `#[serde(default)]`, carried in `from_session`; agent status transitions persist via the normal atomic meta path |
| `crates/core/src/storage/session_io.rs` | override-aware root resolver used by `session_messages_dir` + `save_session_meta`'s rename scan, so agent `name_session` renames land inside the parent's agents/ root and never scatter top-level dirs |
| `crates/core/src/storage/discovery.rs` | scan `<session dir>/agents/*/session.json` on load; reconstruct flagged Sessions (`closed=true`, storage_override set); literal init gains `storage_override: None` |
| `crates/core/src/state/app_state.rs` | agents scan ordering in `load` (before prune); orphan sweep (Running→Failed + parent ToolResult append); `prune_disk_state` staleness check routed through the override-aware resolver; fix C3 eviction |
| `crates/ai/src/provider/tool_defs.rs` | `spawn_agent` def, gated via an options param on `tool_definitions`; both call sites updated (`client.rs:255`, `preflight.rs:128`) |
| `crates/ai/src/chat/polling/stream.rs` | intercept spawn_agent in Step 5 split; await-settle hook alongside shell queue |
| `crates/ai/src/chat/agents.rs` (new) | spawn/create session+runtime, poll/settle → ToolResult, cancel, cap enforcement, card data accessor |
| `crates/ai/src/chat/polling/mod.rs` | new poll step in `update_runtime` for parent-side agent settlement (runs while `stream_rx` is None) |
| `crates/ai/src/chat/completion/mod.rs` | C1 fix (session-scoped model resolution); spawn flow mirrors the handle_handoff seeding skeleton (D7); auto-handoff guard (D9.2) |
| `crates/ai/src/chat/completion/preflight.rs` | C8 closure: session-scoped handoff gate on the forced-handoff path |
| `crates/ai/src/chat/runtime.rs` | `pending_agents: Vec<AgentHandle>` field; `is_busy()` + `drain()` extensions (D9.1/D9.3) |
| `crates/ui/src/chat/tabs.rs`, `crates/ui/src/toolbar/pickers.rs` | filter agent-flagged sessions out of the tab bar AND the session dropdown (`pickers.rs:82-85` lists/reorders sessions) |
| `crates/ui/src/chat/tabs.rs` | hide agent-flagged sessions |
| `crates/ui/src/agents/mod.rs` (new) | window (settings-window pattern), card renderers, cancel button |
| `crates/ui/src/app.rs` | open-windows state, per-frame `agents::show_windows(ctx, …)` |

Conflicts: C1 and C8 (preconditions), C2 (mitigated by D5), C3 (must fix), C7 (document).
Token accounting needs zero changes: the agent's Done feeds its own session; the parent's
next Done reports the enlarged prompt including the result content.

---

## 4. Feature 2 — Throughput / the async question

### 4.1 Bottleneck inventory (code-inspection)

| ID | Bottleneck | Site |
|----|-----------|------|
| B1 | Non-shell tool batch executes strictly sequentially on ONE spawned thread | `stream.rs:729-826` |
| B2 | Shell calls stream one-at-a-time by design (live view) | `shell.rs:start_next_live_shell` |
| B3 | Provider pool fixed clamp(2,8); every in-flight request pins a worker for its whole life; overflow queues invisibly | `client.rs:29-56` |
| B4 | Counting-endpoint preflight blocks the UI thread ≤5 s per request | `preflight.rs:130` |
| B5 | Full-history JSONL reload on the UI thread each request build | `session.rs:62-74` |
| B6 | Per-frame `Vec` allocation of runtime keys | `polling/mod.rs:110` |

### 4.2 The async decision (honest answer)

Workload reality: this is a desktop agent with tens of concurrent blocking sockets at
peak (≤4 agents × (1 stream + occasional shell/web) × user sessions), never thousands.
Threads parking on socket reads cost nothing while waiting — the kernel does the waiting.

- **(c) Full migration.** Rewrite ~11k lines of `autocode-ai` around tokio + reqwest/hyper.
  Cost: ~30 new deps, big compile-time/binary hit, replacement of the hand-rolled SSE
  parser (which encodes hard-won provider quirk fixes: think-tag splitting, index-keyed
  tool accumulation, arg repair, keep-alive semantics), loss of the no-runtime property
  README advertises, RAM budget risk on the target hardware (old laptops). Benefit at our
  concurrency: none measurable. **Verdict: reject.**
- **(b) Tokio boundary for I/O fan-out only.** A runtime inside the provider layer,
  mpsc bridge unchanged. Cost: still imports a runtime + dep tree into the leanest crate;
  two concurrency models to reason about; benefit appears only at ~100+ simultaneous
  streams, i.e. never here. **Verdict: reject for now**; legitimate only if agent caps
  ever grow ~50×.
- **(a) Stay thread-based, parallelize.** **Recommended.** Concrete work:
  - T1: Parallel non-shell tool execution. Partition the batch into path-conflict groups
    (normalized absolute path; writes/patches/deletes/renames touching the same path —
    or rename from/to pairs — serialize; reads always parallel; web/proof tools treated
    as independent except `verify_proof` groups on its attempts.jsonl). Execute groups on
    `std::thread::scope` workers (min(batch_len, 4)); collect results and commit in the
    original tool_call order (wire-valid, deterministic diffs). Extract grouping to a pure
    unit-testable fn (`tools/parallel.rs`). Sites: `stream.rs` Step 6 block.
    Hidden coupling: `LruPathCache` is `&mut`-swapped per batch and reused across batches
    (`stream.rs:684`) — parallel workers cannot share it. Workers get fresh per-worker
    caches; after join, merge resolved entries back into the runtime-owned cache so the
    cross-batch reuse property survives. Cache contents are pure derived data, so this is
    a performance detail, not a correctness one — but it must be handled explicitly.
  - T2: Pool sizing. std has no `Semaphore` and new crates are out, so: spawn one
    dedicated thread per in-flight request, gated by a hand-rolled permit counter
    (Mutex\<usize\> + Condvar, ~15 lines, cap 16). Preserves cancel/drop semantics
    (`CompletionStream` untouched) and removes the silent-queue failure mode of the
    fixed pool. Consequence under the no-dead-code rule: if `thread_pool.rs` ends up
    unreferenced after the switch it is deleted outright, not left parked. Alternative
    (raise the fixed pool constant) rejected: still a hard ceiling with no backpressure
    signal, and idle workers linger forever.
  - T3: Preflight freshness skip: if the last known actual count is fresh (≤ K messages
    appended since the Done that reported it, K≈2), skip the counting call entirely;
    drop its timeout 5 s→2 s. Pure-function heuristic, unit-testable. Removes most B4
    stalls without restructuring start_completion's synchronous flow.
  - T4: Timing log behind `AUTOCODE_TIMING=1`: eprintln `[timing]` lines — tool-batch wall
    time (before/after T1 is the headline number), per-tool durations (already in
    ToolMeta.duration_ms), request start→Done, agent wall time. Measurement protocol:
    identical scripted batch (e.g. 8 mixed reads/greps/writes) against a local fixture
    project, N=10 runs, compare medians.
  - Optional micro: reuse key buffer in `update_all` (B6); leave B5 (bounded by display
    window growth, measured before optimizing), leave B2 sequential deliberately — live
    streaming UX and side-effect ordering; documented trade-off.

Minimum-bar mapping: concurrent tool batches = T1; sub-agents concurrent with responsive
UI = F1 design (all work stays on worker threads; the UI frame loop only polls channels);
pool starvation = T2; measurement = T4.

### 4.3 File/function map (F2)

`stream.rs` (batch dispatch), new `tools/parallel.rs` (grouping + scoped executor,
pure std — `std::thread::scope`, no new deps), `client.rs` (T2 permit gate),
`provider/thread_pool.rs` (deleted if unreferenced after T2), `preflight.rs` (T3),
new timing helper in ai crate helpers (env-gated `eprintln!`, matching the house error-
logging style), unit tests for grouping order and result-order preservation.

---

## 5. Feature 3 — Context attachments (+ button)

### 5.1 Today

`ReqMsg.content: Option<&str>` (`http.rs:993`), `ApiMessage.content: String`
(`types.rs:47`). No multipart content anywhere. Vision: nothing. The UI crate already
depends on `image` 0.25 (png/jpeg/gif/bmp) and `rfd`, and has an established texture
pattern (`explorer/viewer.rs:501-531`, `ColorImage` → `load_texture`). Thumbnails need
zero new dependencies; the file picker needs zero new dependencies.

### 5.2 Design decisions

**D1 — Wire format.** Change `ReqMsg.content` to an untagged enum
`Content::Text(&str) | Content::Parts(Vec<Part>)` with `Part::Text{text}` /
`Part::ImageUrl{image_url:{url}}` serializing exactly as OpenAI content-parts. Owned side:
`ApiMessage` gains `parts: Option<Vec<ContentPart>>` (`#[serde(skip)]`-style — ApiMessage
is never serialized). Parts are assembled in `prepare_request_messages_for_session`, which
is the one place that knows both message attachments and provider capability; plain-String
path untouched. Old request bodies byte-identical when no attachments (grep-gate: no
`ReqMsg { content:` string-only constructions left).

**D2 — Capability flag plumbing.** `ModelManifest.supports_vision: bool` with
`#[serde(default)]` (= false, safe for unknown providers); mirrored on
`provider_file::ModelEntry` (user overrides) **and** carried through
`merge_manifest` (`core/helpers/utils.rs:89-107` rebuilds ModelManifest from disk entries —
missing it there silently resets the flag for everyone who ever saved providers.json).
Helper `model_supports_vision(kind, model)`. Seed `assets/providers.json`: vision=true for
Claude/GPT-4o+/GPT-5/o-series/Gemini entries; false otherwise. Note: providers.json seeds
only on first launch — document that existing users may need to let the manifest merge
supply defaults (it will, since baked-in values fill absent disk fields).

**D3 — Attachment persistence.**
- A: base64 inline in `ChatMessage.attachments` — self-contained JSONL but bloats every
  subsequent full-history reload (invariant 2 makes this a per-request cost; a few MB per
  image violates the RAM discipline).
- B (recommended): copy bytes at stage time into `sessions/<id>_<label>/attachments/`;
  `ChatMessage.attachments: Vec<Attachment>` (serde default, skip when empty) stores
  `{id, kind: Image|File, name, mime, bytes, rel_path}`. Restart restores chips/history
  thumbnails from those files; send-time assembly reads each file exactly once. Draft
  chips persist via `SessionMeta.draft_attachments` (same rel_path scheme), mirroring how
  `draft_input` works. Because staged copies live inside the session directory, session
  deletion cleans them with zero extra code — same disk-truth lifecycle as messages.

**D4 — Injection rules (documented fallback matrix).**

| Attachment | vision model | non-vision model |
|---|---|---|
| Image | `image_url` part, `data:<mime>;base64,<…>` | text block `[Image attached: name (WxH, size) — model lacks vision]` |
| Text/doc file | labeled text part `[Attachment: name]\n<content>` | same labeled text block |
| Binary/unknown doc | notice block with name+size (no dump) | same |

Read happens at send time only; later turns see the injected content as ordinary message
history (system prompt's file-tracking rules apply — we never refresh from disk).

**D5 — Caps & validation (stage time).** Images ≤ 8 MB and ≤ 20 MP decoded; total staged
per message ≤ 32 MB; text injection capped at 128 KB via existing `truncate_middle`;
unsupported binaries rejected with an error chip state.

**D6 — Accounting.** Unchanged: actual-only. Attachments consume context; the next Done
reports it. One sync required: `preflight.rs:106-122` builds the counting-endpoint body by
hand (`{"role", "content"}`) — it must mirror parts or vision-provider counts will be wrong
(exact-count invariant broken precisely when images are present).

**D7 — UI.** "+" button left of the input row (`input.rs`); `rfd::FileDialog::pick_files`
on a spawned thread + channel (exact pattern already in `app.rs:385-397`). Drag-drop via
egui raw input `hovered_files`/`dropped_files` on the chat panel with hover highlight.
Chips row above the TextEdit inside the existing input Frame; image thumbnails decoded via
`image` crate, cached as `TextureHandle`s in `ChatPanelState` keyed by (rel_path, len)
with removal on chip removal. Remove button per chip. Sent messages render attachment
thumbnails inline in the user bubble (`messages.rs::show_user_bubble` extension reading
`msg.attachments`).
`send_message(state, runtimes, text)` gains an attachments parameter — sole caller is
`input.rs:181`.

### 5.3 File/function map (F3)

| File | Change |
|------|--------|
| `crates/core/src/state/chat.rs` | `Attachment`, `ChatMessage.attachments` (serde default) |
| `crates/core/src/storage/session_meta.rs` | `draft_attachments` (serde default) |
| `crates/core/src/state/manifest.rs` | `ModelManifest.supports_vision` |
| `crates/core/src/helpers/utils.rs` | merge_manifest carries supports_vision; helper accessor |
| `crates/core/src/storage/provider_file.rs` | `ModelEntry.supports_vision` |
| `assets/providers.json` | per-model flags |
| `crates/core/src/storage/attachments.rs` (new) | stage/copy/resolve/delete helpers (pure fs, testable) |
| `crates/ai/src/provider/types.rs` | `ContentPart`, `ApiMessage.parts` |
| `crates/ai/src/provider/http.rs` | `ReqMsg.content` enum serialization |
| `crates/ai/src/provider/client.rs` | ReqMsg construction in `build_request_body` picks text vs parts (`client.rs:159-178`) |
| `crates/ai/src/chat/session.rs` | parts assembly gated on vision |
| `crates/ai/src/chat/completion/preflight.rs` | counting body mirrors parts |
| `crates/ai/src/chat/completion/mod.rs` | `send_message` signature |
| `crates/ui/src/chat/input.rs` + new `attachments.rs` | + button, drag-drop, chips, staging |
| `crates/ui/src/chat/state.rs` | pending chips + texture cache |
| `crates/ui/src/chat/messages.rs` | history thumbnails in user bubbles |
| `crates/ui/src/app.rs` / `panel.rs` | drop-target plumbing |

Sequencing within F3 (each independently committable): (1) wire format + capability
plumbing + tests, no UI; (2) staging/storage + send_message plumbing with text-only
fallback; (3) vision parts; (4) UI (button/chips/drag-drop/thumbnails/history).

---

## 6. Dependency graph & risk register

```
P0  C1 fix (session-scoped model) ──┐
P0  C8 fix (session-scoped handoff)─┤
P0  C3 fix (safe session pruning) ──┼─► Feature 1 (sub-agents)
P1  F2 T1 parallel tools ───────────┘
P1  F2 T2 pool permit gate ─► raises F1 concurrency ceiling
P1  F2 T3 preflight skip, T4 timing log (T4 first: baseline before T1/T2)
Fx  Feature 3 phases 1-4 (independent track; phase 1 any time)
```

Top risks:
1. C1 regression risk: making requests session-scoped touches every completion start;
   mitigated by keeping behavior identical when `sess.model == prov.model` (the common case).
2. Parallel tool exec changes failure semantics (partial batch failure ordering) — results
   commit in request order; panics already caught per-tool; add test for interleaved
   write/read on the same path asserting serialization.
3. Storage compat: all new serde fields defaulted; stability-style tests with old-format
   fixtures must pass unchanged (mission hard rule).
4. egui texture cache growth with many image chips — bound the cache (LRU, ~16 textures).
5. Windows path handling for attachment copies goes through `fsutil::extended_path` like
   all other file I/O.
6. Lifecycle gating (D9) is behavior-critical: the manual scripts must cover
   send-while-agent-running (must be a no-op), Stop-during-agent (parent resumes with
   error results), and parent handoff/delete during agent run.
7. Per-phase grep-clean gates: superseded symbols (e.g. `ThreadPool`, `pool()` if T2
   lands) must be fully removed; zero new `#[allow]`/`expect`/`cfg(feature)` attributes
   anywhere; `cargo clippy --workspace --all-targets` with `-Dwarnings` stays clean.
8. Vision models behind gateways that reject `image_url` parts will surface provider
   errors that flow through the existing retry/backoff path — acceptable, but the manual
   script matrix must include one vision send per shipped provider to catch format
   quirks early (OpenAI-style content-parts is the only wire shape we emit).

## 7. Test strategy summary (detailed plan lands in IMPLEMENTATION_PLAN.md)

- Unit: path-conflict grouping/order; Content enum serde snapshots (string vs parts);
  fallback-matrix builders; tool-def gating (spawn_agent absent for agents);
  preflight freshness heuristic; timing-log formatting.
- Integration (`crates/core/tests/stability.rs` style): old session JSONL loads with new
  fields absent; AgentMeta roundtrip + restart orphan sweep marks Failed **and appends
  the synthetic ToolResult to the parent JSONL**; agent `name_session` rename moves the
  folder within the agents root (no stray top-level dirs); parent-delete cascades to
  agent folders; attachment copy → restart restore → delete-session cleanup;
  draft_attachments roundtrip.
- Manual scripts: two agents streaming while typing in parent (UI responsiveness); cancel
  mid-stream (parent resumes, status Cancelled persisted); png drop onto vision vs
  non-vision provider (part vs fallback block); AUTOCODE_TIMING before/after medians.

— END OF AUDIT. Awaiting approval before Phase 2 (IMPLEMENTATION_PLAN.md) or any feature code.
