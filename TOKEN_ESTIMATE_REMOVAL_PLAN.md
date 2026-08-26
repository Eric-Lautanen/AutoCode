# Plan: Remove Estimated Token Counts — Use Only Actual Provider Token Counts

## Goal

Delete the heuristic token-estimation pipeline entirely and make every token
figure in the app (UI meter, auto-handoff trigger, preflight context check,
looping-window trigger, model-facing context line) derive exclusively from
**actual counts reported by providers**:

1. `usage.prompt_tokens` / `usage.completion_tokens` from each API response
   (`ProviderEvent::Done`) — already implemented.
2. Provider **token-counting endpoints** (`count_input_tokens`) for
   request-time validation — already implemented, needs to become the primary
   (not fallback) source for preflight.

No code is changed by this document; it is the implementation guide.

---

## Part 1 — Audit: Everything Estimate-Related

### 1A. Core crate (`crates/core`)

| Location | Item | Action |
|---|---|---|
| `src/helpers/tokens.rs` (whole file) | `estimate_tokens`, `estimate_message_tokens`, `estimate_single_message_json_tokens`, `estimate_tools_tokens`, `estimate_full_request_tokens` (already dead), `compute_request_estimate`, `estimate_tokens_json`, `is_cjk` | **DELETE FILE** |
| `src/helpers/mod.rs:16-20` | re-exports of all of the above | Remove `tokens::{...}` re-export block |
| `src/tokenizer/mod.rs` (whole file) | `Tokenizer` trait, `HeuristicTokenizer`, `tokenizer_for_model`, `offline_token_count` — zero callers outside itself (dead code) | **DELETE FILE + remove `pub mod tokenizer;` from `src/lib.rs:14`**, update lib.rs doc comment (line 5) |
| `src/state/session.rs:46-53` | fields `estimated_full_tokens`, `estimated_messages_tokens` (write-only, never read) | DELETE fields |
| `src/state/session.rs:99-105` | field `estimated_full_at_request` | DELETE field |
| `src/state/session.rs:92-97` | field `token_correction_ratio` + `default_token_correction_ratio()` (lines 6-10) | DELETE |
| `src/state/session.rs:107-115` | `cached_tool_tokens`, `cached_tool_key` (tool-def estimate cache) | DELETE |
| `src/state/session.rs:155-163` | `Session::token_count()` — sums per-message estimates; **no callers** | DELETE method |
| `src/state/session.rs:165-182` | `recompute_messages_tokens()` — **no callers** | DELETE method |
| `src/state/session.rs:184-208` | `record_actual_usage(prompt, completion)` | **KEEP**, but delete the correction-ratio learning block (lines 192-207); keep only "overwrite when prompt > 0" guard |
| `src/state/session.rs:210-253` | `usage_tokens()` (hybrid actual+delta), `corrected_full_tokens()` (no callers) | Replace with a single accessor returning `actual_tokens_used`; see Part 3 |
| `src/state/chat.rs:56-61` | `ChatMessage.token_count` (heuristic stamped at construction; read nowhere except test literals) | DELETE field + its init in `new()` (line 93) |
| `src/state/chat.rs:66-72` | `ChatMessage.full_token_estimate` | DELETE field + its init (line 100) |
| `src/helpers/utils.rs:219-228` | `update_full_estimate()` | DELETE |
| `src/helpers/utils.rs:312-410` | display block: `session_messages_usage`, `budget_fraction`, `usage_display`, `fmt_tokens` | REWRITE to actual-only (Part 3) |
| `src/storage/session_meta.rs:32-42` | `SessionMeta.estimated_full_at_request`, `.token_correction_ratio` (+ from_session lines 84-85) | DELETE (serde ignores unknown keys on read → old session.json files stay loadable) |
| `src/storage/session_io.rs:283-285` | restore of `estimated_full_at_request`, `token_correction_ratio` | DELETE those two lines (keep `actual_tokens_used`) |
| `src/storage/session_io.rs:305-307` | stale comment about `estimated_full_tokens` set by callers | DELETE comment |
| `src/storage/discovery.rs:207-239` | **only non-test `Session {` struct literal in the workspace** — zero-inits removed fields (224-225, 231-233) | Delete corresponding literals; this file WILL fail to compile until updated |
| `src/lib.rs:5` | doc comment "token estimation (heuristic)" | Update text |

> **Keep-list warning:** `serde_defaults::default_context_tokens`
> (`serde_defaults.rs:18`, AppState `max_context_tokens` default) sounds
> estimate-related but is provider-window configuration — **DO NOT delete**
> during the sweep. Same for all `max_output_tokens*`, `context_window`,
> `max_context_tokens`, `handoff_percent` fields everywhere.

### 1B. AI crate (`crates/ai`)

| Location | Item | Action |
|---|---|---|
| `chat/session_ops.rs:55` | `msg.full_token_estimate = estimate_single_message_json_tokens(...)` inside `push_to_session` | DELETE line |
| `chat/session_ops.rs:65-66` | `push_to_session` tail calls `recompute_estimate_from_disk` on **every push** (forces full disk reload!) | DELETE call; ensure flush behavior preserved (see Risk R6) |
| `chat/session_ops.rs:69-108` | `recompute_estimate_from_disk()` — flush + disk reload + estimate pipeline | DELETE function (all callers listed below); keep a plain forced flush if needed |
| `chat/session_ops.rs:110-157` | `tool_defs_tokens_for_session()`, `refresh_tool_tokens_cache()` | DELETE both |
| `chat/session_ops.rs:189-197` | `update_session_estimate()` | DELETE |
| `chat/session_ops.rs:249` | `replay_to_message` resets `actual_tokens_used = 0` | KEEP |
| `chat/session_ops.rs:261-267` | replay calls recompute + resets correction ratio | DELETE both (correction ratio no longer exists) |
| `chat/session_ops.rs:444-480` | `context_usage_info_for_session()` uses `s.usage_tokens().min(max)` | Switch source to actual-only accessor (Part 3) |
| `chat/completion/preflight.rs` (whole file) | preflight built on `estimated_full_tokens × token_correction_ratio`, counting-API optional w/ heuristic fallback | **REWRITE** per Part 4 |
| `chat/completion/mod.rs:261-268` | snapshot `sess.estimated_full_at_request = sess.estimated_full_tokens` in `start_completion` | DELETE block |
| `chat/completion/mod.rs:486` | `check_auto_handoff` uses `sess.usage_tokens()` | Switch to actual-only accessor |
| `chat/completion/mod.rs:589-590` | `auto_continue_impl` calls `recompute_estimate_from_disk` | DELETE call |
| `chat/session.rs:5` | import `compute_request_estimate` | DELETE |
| `chat/session.rs:135-144` | `prepare_request_messages_for_session` recomputes estimates before every request | DELETE block |
| `chat/session.rs:164-183` | model-facing system message via `context_usage_info_for_session` | Keep mechanism; now reports actual-based numbers |
| `chat/looping.rs:80` | trigger uses `usage_tokens()` | Switch to actual-only accessor |
| `chat/looping.rs:117` | pruning score bonus `msg.full_token_estimate > 2000` | Replace with deterministic size check: `msg.content.len() > 8000` bytes (~2k tokens at ~4 B/token) or drop criterion |
| `chat/looping.rs:229` | post-prune `recompute_estimate_from_disk` | DELETE |
| `chat/polling/stream.rs:98` | Done → `record_actual_usage(...)` | **KEEP** (this is the actual-count entry point) |
| `chat/polling/stream.rs:241-243` | silent-done path calls recompute | DELETE call |
| `chat/polling/stream.rs:106-109` | status "Done -- N prompt + M completion tokens" | KEEP (actuals already) |
| `chat/polling/tools.rs:56-57,123-124` | comments about estimate refresh | Update comments |
| `chat/mod.rs:13-14` | exports of deleted fns | Trim export list |
| `provider/client.rs:383-449` | `count_input_tokens()` | **KEEP** — becomes primary preflight source; fix tool-count gap (Risk R4 / Part 4 step 3) |
| `provider/http.rs:451-460, 554, 640-646, 822-835` | usage parsing into `Done` | **KEEP** unchanged |
| `provider/types.rs:114-121` | `Done { prompt_tokens, completion_tokens }` | **KEEP** |

### 1C. UI crate (`crates/ui`)

| Location | Item | Action |
|---|---|---|
| `toolbar/layout.rs:42-43` | `budget_fraction(state)` drives meter | Keep call signature; internals change in core |
| `toolbar/meters.rs:74` | `core_helpers::usage_display(state)` label | Keep call; string format changes |
| `toolbar/meters.rs:83-85` | hover text mentions heuristic vs actual | Rewrite: single "API-reported context tokens from the last request" |
| `app.rs:131-132` | session-open calls `recompute_estimate_from_disk` | DELETE call (loading a session needs no estimate pass) |
| `chat/session.rs:101-104` | session-load calls `recompute_estimate_from_disk` | DELETE call |

### 1D. Compile-breakage points (struct literals)

Rust struct literals fail to compile when fields are removed. The **complete**
list of literal construction sites in the workspace:

- `Session { ... }` — `crates/core/src/storage/discovery.rs:207` only.
- `ChatMessage { ... }` — `crates/core/tests/stability.rs:84,157,215,273`
  (4 sites).
- `SessionMeta { ... }` — `crates/core/tests/stability.rs:35`
  (`make_session_dir`) only.

Everything else constructs via `::new()` / `Default`, which are updated
in place.

### 1E. Tests

| Location | Item | Action |
|---|---|---|
| `crates/core/tests/stability.rs:93,166,224,282` | ChatMessage literals with `token_count:` / `full_token_estimate:` | Remove those struct fields (all 4 literal blocks, lines 84-101, 157-…, 215-…, 273-…) |
| `stability.rs:49-51` | SessionMeta literals `actual_tokens_used/estimated_full_at_request/token_correction_ratio` | Keep `actual_tokens_used`; delete other two |
| `stability.rs:320-360` | `test_token_correction_ratio_survives_restart` | Delete test; replace with an `actual_tokens_used` save/load round-trip assertion |

This is the **only** integration-test file in the workspace
(`crates/{ai,ui,fs}` have no `tests/` dirs).

### 1F. Docs & config

| Location | Item | Action |
|---|---|---|
| `README.md:22` | "Token Management \| 2-tier counting (API → heuristic)" | Reword: "Actual token counts from API responses; counting-endpoint validation before requests" |
| `README.md:69` | "**2-tier token estimation** - API counting endpoint → heuristic fallback" | Reword to actual-counts-only description |
| `README.md:167` | crate table: "token estimator" listed for autocode-core | Remove from the feature list cell |
| `.github/workflows/ci.yml` | CI gates (see Part 10) | No change; use as verification bar |
| `assets/providers.json:12,55,140` | `counting_endpoint` entries | **KEEP** (these are actual counts) |

### 1G. Verified-clean areas (checked, no estimate code — do not touch)

- `crates/fs/` — "token" hits are fuzzy-search text tokenization
  (`explorer/fuzzy.rs`, `explorer/comment.rs`). Unrelated.
- `crates/autocode/` — binary shell only (`main.rs`, `build.rs`, `app.rc`).
- `crates/ui/src/settings/providers.rs` — all "tokens" hits are
  max-output/context-window settings UI. Legit config, stays.
- `crates/ui/src/explorer/viewer.rs:235` — "row_h estimate" is layout math,
  unrelated (expected grep-noise survivor post-refactor).
- `crates/ai/src/chat/runtime.rs` — no token fields on `ChatRuntime`.
- `crates/core/src/state/app_state.rs` system/handoff prompts — line 52 says
  "The result includes your current token usage" (refers to the per-request
  context line, which REMAINS, now actual-based); handoff trigger prompt text
  mentions "context window near its limit" — both still accurate. No changes.
- `crates/ai/src/chat/polling/mod.rs:50-55` comment about
  `actual_tokens_used` jump ordering — still accurate post-refactor.
- `crates/ai/src/helpers/misc.rs::project_context_string` — no token content.
- `crates/core/src/utils/extract.rs` — HTML-extraction test fixture text;
  unrelated.
- Skills directory (`.md` reference docs) and `assets/` besides providers.json.

---

## Part 2 — Existing Actual-Token Infrastructure (keep & build on)

Already working end-to-end today:

1. **Streaming**: `stream_options.include_usage` requested
   (`client.rs:250-252`); SSE parser captures final usage chunk
   (`http.rs:640-646`) → `ProviderEvent::Done`.
2. **Non-streaming**: body `usage.prompt_tokens/completion_tokens`
   (`http.rs:451-460`).
3. **Storage**: `Session.actual_tokens_used` persisted as
   `SessionMeta.actual_tokens_used` (survives restart).
4. **Counting endpoints** (`client.rs::count_input_tokens`):
   - OpenAI `{base_url}/responses/input_tokens`
   - Anthropic `{base_url}/messages/count_tokens`
   - OpenRouter / NVIDIA NIM / generic `{base_url}/tokenize`
5. Guard: `record_actual_usage` ignores `prompt == 0` responses so the last
   known real value is never clobbered.

---

## Part 3 — New Single Source of Truth

### Step 3.1 — One accessor on `Session`

In `crates/core/src/state/session.rs`, replace `usage_tokens()` /
`corrected_full_tokens()` with:

```rust
/// Context size in tokens as last reported by the provider
/// (`usage.prompt_tokens` of the most recent request). Zero until the
/// first response arrives. This is the ONLY token figure used anywhere:
/// display, handoff, preflight, looping trigger, model-facing context line.
pub fn context_tokens(&self) -> usize {
    self.actual_tokens_used
}
```

Then mechanically update all consumers (compiler will find them):

- `helpers/utils.rs`: `session_messages_usage` → returns
  `(s.context_tokens(), actual)`; simplify `budget_fraction` and
  `usage_display` to show one number:
  `"~{used} / {max} (handoff @{threshold})"` — drop the "est / actual"
  dual column and the word "est".
- `ai/chat/session_ops.rs::context_usage_info_for_session`:
  `s.usage_tokens().min(max)` → `s.context_tokens().min(max)`.
- `ai/chat/completion/mod.rs::check_auto_handoff`:
  `sess.usage_tokens()` → `sess.context_tokens()`.
- `ai/chat/looping.rs:80`: same substitution.

Semantics note (document in the accessor's doc-comment): between requests the
number lags reality by exactly the messages appended since the last response.
That lag is inherent to "actual counts only" and is accepted everywhere.

### Step 3.2 — Simplify `record_actual_usage`

```rust
pub fn record_actual_usage(&mut self, prompt: usize, _completion: usize) {
    // Providers occasionally omit usage in terminal chunks (prompt == 0);
    // never overwrite the last known good value with zero.
    if prompt > 0 {
        self.actual_tokens_used = prompt;
    }
}
```

---

## Part 4 — Preflight Rewrite (`completion/preflight.rs`)

Replace the whole estimate/correction pipeline. New logic, in order:

1. `trim_session_ram(state, session_id)` (unchanged).
2. Compute `known = sess.context_tokens()` (last actual; 0 on fresh sessions).
3. If `provider.has_counting_api()`:
   - Build the request body **including tools** (see step 4 below), call
     `count_input_tokens(provider, body, model, timeout)` synchronously
     (existing behavior, timeout 5 s).
   - On success use that count as `estimated`; optionally cache it back into
     nothing — just use the return value locally.
   - On failure fall through to `known`.
4. **Fix the tool-token gap**: today `recompute_estimate` adds a *heuristic*
   `tool_tokens` on top of the API count (`preflight.rs:113,138`). Instead,
   include the real `tool_definitions(strict, handoff)` array inside the JSON
   body passed to `count_input_tokens` (all three supported endpoint formats
   accept `tools`). Anthropic's count endpoint requires `tools` at top level —
   verify per-kind shaping in `count_input_tokens` and add a `tools` key next
   to the existing `messages` key there rather than at call sites.
5. Decision math (identical shape to today, new input):
   - `if used + max_output > max_context`:
     - `room < 1000 && handoff_enabled` → drain + `handle_handoff`, return None.
     - `room < 256` → error message + abort (update wording: drop the word
       "estimated", report the counted/last-known number).
     - else clamp `max_tokens = room`.
6. Delete: `model_changed` ratio reset, correction lookup, cached-estimate fast
   path, `recompute_estimate`'s heuristic branches.

Note: with counting APIs unavailable and `known == 0` (fresh session),
preflight cannot validate — that is correct under "actual only"; the provider's
own context-length error path (retry/backoff in `polling/stream.rs`) remains
the safety net.

---

## Part 5 — Looping Window Adjustments (`chat/looping.rs`)

1. Trigger check (line 80): `let used_tokens = state.sessions[idx].context_tokens();`
   — first-turn lag means pruning starts one turn later than before;
   acceptable and documented.
2. Tool-output size scoring (line 117): replace
   `msg.full_token_estimate > 2000` with a byte threshold constant, e.g.
   `const LARGE_TOOL_OUTPUT_BYTES: usize = 8_000;` and
   `msg.content.len() > LARGE_TOOL_OUTPUT_BYTES`. Bytes are measured, not
   estimated.

---

## Part 6 — Push Path Cleanup (`chat/session_ops.rs`)

After deleting `full_token_estimate` stamping and the per-push
`recompute_estimate_from_disk`, `push_to_session` becomes: timestamp-stamp →
assign id/turn → queue pending write (non-Error) → push to RAM window. Verify:

- Disk persistence still happens via the rate-limited writer
  (`ui/src/app.rs:74,96-97,252` timer flush) **and** the forced
  `flush_pending_writes(true)` in `prepare_request_messages_for_session`
  before every request. Both remain; nothing else needed.
- `ui/src/app.rs:131-132` and `ui/src/chat/session.rs:101-104`: remove the
  `recompute_estimate_from_disk` calls made after session open/load.
- `replay_to_message` (session_ops.rs:261-267): drop the recompute + ratio
  reset; keep truncation + meta save + `actual_tokens_used = 0`.

---

## Part 7 — Storage Compatibility

- Removed `Session`/`ChatMessage`/`SessionMeta` fields are all
  `#[serde(default)]`; serde **ignores unknown keys** when deserializing, so
  every existing `session.json` and messages JSONL loads unchanged. Old files
  simply stop carrying those keys after their next save.
- `discovery.rs` / `session_io.rs`: update struct literal sites after field
  removal (compiler-driven).
- No disk-format migration needed.

---

## Part 8 — Suggested Implementation Order

Each phase compiles and passes tests independently.

1. **Phase 1 — Introduce the new source (additive).**
   Add `Session::context_tokens()`; switch the four consumers
   (utils display, context_usage_info_for_session, check_auto_handoff,
   looping trigger). Behavior identical-ish (hybrid → actual-only).
2. **Phase 2 — Preflight rewrite** (Part 4) incl. tools-in-body fix in
   `count_input_tokens`.
3. **Phase 3 — Delete the estimation core.**
   `helpers/tokens.rs`, `tokenizer/` module, `lib.rs` mod decl + docs,
   `helpers/mod.rs` re-exports, `update_full_estimate`,
   `recompute_estimate_from_disk`, `tool_defs_tokens_for_session`,
   `refresh_tool_tokens_cache`, `update_session_estimate`, preflight leftovers,
   push-path stamping + recompute calls (Part 6), looping byte-threshold swap,
   UI call-site deletions, comment updates in `polling/tools.rs`.
4. **Phase 4 — Delete state & storage fields**
   (Session fields, ChatMessage fields, SessionMeta fields, accessors
   `token_count()`, `recompute_messages_tokens()`, `corrected_full_tokens()`,
   `default_token_correction_ratio`, ratio logic inside
   `record_actual_usage`), then fix `discovery.rs` / `session_io.rs` /
   `session_meta.rs` literals.
5. **Phase 5 — UI copy.** Meter hover text, `usage_display` string.
6. **Phase 6 — Tests.** Fix `stability.rs` literals; delete
   `test_token_correction_ratio_survives_restart`; add:
   - actual-tokens round-trip across save/load,
   - `context_tokens()` stays stale-but-valid when provider reports 0,
   - preflight clamps using counting-API result (unit-test the decision math
     extracted into a pure fn),
   - looping score uses byte threshold.
7. **Phase 7 — Dead-code & docs sweep.** `cargo build` warnings for unused
   imports (`compute_request_estimate` etc.), trim `chat/mod.rs` exports
   (`refresh_tool_tokens_cache`, `recompute_estimate_from_disk`,
   `update_session_estimate`, `tool_defs_tokens_for_session`),
   update `README.md:22,69,167` (Part 1F), stale comments in
   `polling/tools.rs` / `session_io.rs`, then run the grep-clean gate from
   Part 10.

---

## Part 9 — Risks & Mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | Handoff/preflight decisions lag up to one turn behind true context (no delta guessing anymore) | Accepted trade-off; provider-side context errors still retry/handoff via existing paths; counting endpoints cover the gap where available |
| R2 | Fresh sessions show 0 tokens until first response | UI shows "—" or 0%; document in hover text |
| R3 | Some providers omit usage even with `include_usage` (prompt=0 chunks) | Existing `prompt > 0` guard retains last known value |
| R4 | Counting-endpoint bodies differ per provider; tools may need top-level placement (Anthropic) | Shape `tools` inside `count_input_tokens` per kind; keep 64 KB cap; unit-test body shaping |
| R5 | Counting endpoint latency (sync HTTP ≤5 s) on every preflight | Only called when `has_counting_api()`; consider caching result keyed by (message_count, last_msg_id) if profiling shows pain |
| R6 | Removing per-push `recompute_estimate_from_disk` also removes its forced flush | Confirmed safe: UI frame timer flushes rate-limited (`ui/src/app.rs`) and `prepare_request_messages_for_session` force-flushes before each request |
| R7 | Old persisted `token_correction_ratio` semantics disappear silently | Fine — field simply stops being written; old files load via serde ignore-unknown |
| R8 | Model-facing "Context: X/Y" line now lags one turn | Line already describes a snapshot; wording unchanged |

---

## Part 10 — Verification

CI (`.github/workflows/ci.yml`) is the bar — match it exactly, including
`RUSTFLAGS: -Dwarnings` so clippy warnings fail:

```
$env:RUSTFLAGS='-Dwarnings'          # PowerShell; CI sets it globally
cargo fmt --check
cargo clippy --workspace --all-targets
cargo build --workspace
cargo test --workspace
```

CI additionally builds/tests `--target x86_64-pc-windows-msvc` and
`aarch64-apple-darwin` on toolchain 1.96 — no platform-specific token code
exists, but run the Windows target locally before pushing.

**Grep-clean gate** — these must return zero hits in `crates/` after Phase 7
(the only permitted survivors are `viewer.rs:235` "row_h estimate",
`proof.rs:300` "Detection heuristics", and fuzzy-search `token*` in
`crates/fs/explorer/`):

```
estimated_full_tokens | estimated_messages_tokens | estimated_full_at_request
full_token_estimate   | token_correction_ratio     | actual "est /"
estimate_tokens       | compute_request_estimate   | recompute_estimate_from_disk
refresh_tool_tokens_cache | tool_defs_tokens_for_session | update_session_estimate
update_full_estimate  | corrected_full_tokens      | usage_tokens()
tokenizer_for_model   | offline_token_count        | HeuristicTokenizer
count_input_tokens    # allowed ONLY in preflight.rs + client.rs definition
```

Manual checks:

1. Fresh session → send message → toolbar shows the provider's prompt count
   after first response; no "est" wording anywhere.
2. Long conversation → handoff fires near configured percent based on actual
   counts.
3. Provider with counting endpoint (Anthropic/OpenAI/OpenRouter): fill context
   deliberately → preflight clamps/hands off before provider error.
4. Provider without counting endpoint: overflow produces provider error →
   retry/handoff path still works.
5. Reopen old sessions created pre-refactor → they load, meter shows last
   stored actual.
