# Looping Context Window — Implementation Plan

## Concept

A toggleable mode that prunes old assistant/tool message pairs when the session's token usage crosses a configurable threshold, keeping the system prompt and recent exchanges. Pruning aggressiveness is configured per-model in provider settings — some models handle aggressive pruning well, others need a more conservative approach. A `FileAccessLog` tracks which files the model has accessed so the pruning algorithm can retain messages that reference actively-used files. One group is removed per trigger cycle so the algorithm always has fresh working-set data before making the next removal decision.

---

## File Organization

This is a distinct subsystem from the existing "trim RAM" logic already in `session_ops.rs` (372 lines — push/replay/trim/format) — that's a dumb count-based trim; this is scoring-based semantic pruning with its own state, file ops, and disk side-effects. It earns its own files rather than growing the existing ones further:

- **`core/src/state/access_log.rs`** *(new)* — `FileAccessLog`, `AccessEntry`, `FileOp`. Fits the existing pattern of one state type per file (`chat.rs`, `session.rs`, `todo.rs`, etc.) under `core/src/state/`.
- **`ai/src/chat/looping.rs`** *(new)* — `apply_looping_window()`, `pair_groups()`, `is_unverified_edit_group()`, scoring, breadcrumb construction, dry-run logging. Register with `pub mod looping;` in `ai/src/chat/mod.rs` and export `apply_looping_window`. Keeping this separate from `session_ops.rs` means the existing trim-RAM path and the new scoring path can't accidentally tangle, and the ~150 lines of scoring logic gets its own home instead of pushing `session_ops.rs` past 500 lines.

Everything else (new fields on `Session`/`AppState`/`ChatMessage`, the toggle button, settings) is a small, localized addition to an existing file and stays there — no need to split those out.

**Note on the real chat module layout**: `polling.rs` is actually a directory now (`polling/mod.rs` 131 lines, `polling/stream.rs` 702 lines, `polling/tools.rs` 127 lines), and `ai/src/chat/tools.rs` (1,261 lines) is a *different* file from `polling/tools.rs` — the former executes the 21 tools, the latter collects results and calls `commit_tool_results`. The original draft of this plan was written against generic guessed paths/line numbers before the structure doc existed; the sections below are corrected against the real layout.

---

## New Data Structures

### 1. `FileAccessLog` (new file)

```
crates/core/src/state/access_log.rs      ← new file
crates/core/src/state/mod.rs             ← add `pub mod access_log;`
```

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileAccessLog {
    pub entries: HashMap<String, AccessEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessEntry {
    /// Turn number of most recent access (monotonic per-session counter).
    pub last_turn: u64,
    /// Turn number of first access.
    pub first_turn: u64,
    /// Total number of times this file was accessed.
    pub access_count: u64,
    /// Bitmask of operation types (Read=1, Edit=2, Grep=4, Glob=8, Search=16).
    pub ops: u8,
}

impl FileAccessLog {
    pub fn new() -> Self;
    /// Record a file access. `path` = resolved absolute path, `turn` = the
    /// turn this access happened on (use `ChatMessage.turn`, see below —
    /// this makes the log fully reconstructable from messages, including
    /// their *original* turn numbers on reload, not just turn 0).
    pub fn record(&mut self, path: &str, op: FileOp, turn: u64);
    /// Returns paths accessed within the last `within_turns` turns from `current_turn`.
    pub fn active_working_set(&self, current_turn: u64, within_turns: u64) -> HashSet<&str>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOp {
    Read = 1,
    Edit = 2,
    Grep = 4,
    Glob = 8,
    Search = 16,
}

/// Single shared mapping from tool name to FileOp, used by every call site
/// below (recording, disk rebuild, scoring) so the match arms aren't
/// duplicated three times and drifting out of sync with each other.
pub fn tool_name_to_op(tool_name: &str) -> Option<FileOp> {
    match tool_name {
        "read_file" | "read_entire_file" | "read_files" => Some(FileOp::Read),
        "write_file" | "patch_file" | "patch_lines" => Some(FileOp::Edit),
        "grep" | "search" => Some(FileOp::Grep),
        "glob" => Some(FileOp::Glob),
        "list_dir" | "project_tree" => Some(FileOp::Search),
        _ => None,
    }
}
```

**Why no `msg_ids` field**: an earlier draft of this plan tracked which `ChatMessage` ids referenced a path and cross-checked that against a group's message ids to decide "does this group touch a working-set path." That's unnecessary indirection — each message's `tool_meta.file_path` already says which path it touched, so a group can just check its own messages' paths directly against `active_working_set()`. Simpler and removes a whole field + method.

**Storage**: Not persisted to disk. Rebuilt from `ToolMeta.file_path` on session reload. At ~200-300 bytes per entry (HashMap + String overhead), a 200-file session uses ~60KB.

---

### 2. New Fields on `Session` / `SessionMeta`

**File:** `crates/core/src/state/session.rs`

```rust
/// When true, old message pairs are pruned when the session's token usage
/// crosses the model's configured trigger threshold. Also disables auto-handoff.
#[serde(default)]
pub looping_window: bool,

/// Monotonic turn counter. Incremented at the start of each completion
/// cycle in `start_completion()`. Used by FileAccessLog.
#[serde(default)]
pub turn_count: u64,

/// In-memory file access log (not serialized — rebuilt from ToolMeta on load).
#[serde(skip)]
pub access_log: FileAccessLog,
```

Add defaults in `Session::new()`: `false`/`0`/`FileAccessLog::new()`. `loop_max_pairs` is removed — the trigger threshold comes from the active model's `LoopAggressiveness` config.

**File:** `crates/core/src/state/chat.rs`

```rust
/// Which session turn this message was created on (== sess.turn_count at
/// push time). Persisted, so it survives reload — this is what lets
/// `looping.rs` compute a group's turn directly instead of guessing, and
/// lets `turn_count` itself be recovered accurately after a reload (see
/// Rebuild section below).
#[serde(default)]
pub turn: u64,
```

Set this on every `ChatMessage` at the moment it's pushed via `push_to_session()` (`crates/ai/src/chat/session_ops.rs`) — `msg.turn = sess.turn_count`. This applies to *all* roles (User/Assistant/Tool/Error/System), not just tool results, since `pair_groups()` needs the Assistant message's `turn` to anchor each group.

**File:** `crates/core/src/storage/session_meta.rs`

```rust
#[serde(default)]
pub looping_window: bool,
```

`loop_max_pairs` is not persisted — the threshold is derived from the model config at runtime. Add mapping in `SessionMeta::from_session()` (line ~81, but verify against current file).

**File:** `crates/core/src/storage/discovery.rs`

The `discover_sessions_from_disk()` function at line 170 constructs `Session` structs by direct field assignment from `SessionMeta`. Add:

```rust
looping_window: meta.looping_window,
```

---

### 3. `LoopAggressiveness` — Per-Model Pruning Config

**File:** `crates/core/src/state/provider.rs` (452 lines — "ApiProvider config, ProviderKind, ThinkingApi enum, model defaults")

Add alongside the existing per-model fields (context window size, handoff threshold, etc.):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoopAggressiveness {
    /// Trigger at 85% context full. Remove 1 group. Keep newest 40% as floor.
    /// Best for models that struggle with lost context or tend to repeat work.
    Conservative,
    /// Trigger at 75% context full. Remove 1 group. Keep newest 30% as floor.
    /// Default. Good balance for most models on most workloads.
    #[default]
    Balanced,
    /// Trigger at 65% context full. Remove 1 group. Keep newest 20% as floor.
    /// For models that pick up from breadcrumbs well and you want a tight window.
    Aggressive,
}

impl LoopAggressiveness {
    /// What % of the model's context window triggers a prune pass (0.0–1.0).
    pub fn trigger_pct(self) -> f32 {
        match self {
            Self::Conservative => 0.85,
            Self::Balanced     => 0.75,
            Self::Aggressive   => 0.65,
        }
    }
    /// How many groups to remove per trigger. Always 1 — one removal at a
    /// time gives the algorithm fresh working-set data before the next
    /// decision. The aggressiveness difference is in *when* we trigger and
    /// *how protected* the recency floor is, not in bulk-removal quantity.
    pub fn remove_per_trigger(self) -> usize { 1 }
    /// Fraction of groups always kept (newest N%). Floor is min 2 groups.
    pub fn recency_floor_pct(self) -> f32 {
        match self {
            Self::Conservative => 0.40,
            Self::Balanced     => 0.30,
            Self::Aggressive   => 0.20,
        }
    }
}
```

Add to the per-model config struct (whatever holds context window size and handoff threshold):

```rust
#[serde(default)]
pub loop_aggressiveness: LoopAggressiveness,
```

**File:** `crates/core/src/helpers/serde_defaults.rs`

Remove `default_loop_max_pairs()` — no longer needed. The trigger is token-based via `loop_aggressiveness`.

**File:** `crates/core/src/state/app_state.rs`

Remove `loop_max_pairs` field — superseded. Token usage is already tracked per-session; `apply_looping_window()` reads it directly.



---

## Tool-Layer Instrumentation

**Key constraint**: `execute_tool_with_cache()` (in `ai/src/chat/tools.rs`, the 1,261-line file that executes all 21 tools on the bg thread) takes `ToolExecCtx` which has no access to `Session` or `AppState`. File access logging cannot happen inside the tool function itself.

### Solution: widen `ToolResult`

**File:** `crates/ai/src/chat/runtime.rs`, `ToolResult` struct:

```rust
pub struct ToolResult {
    pub tool_call: ToolCall,
    pub content: String,
    pub meta: ToolMeta,
    pub accessed_paths: Vec<String>,        // ← new: resolved absolute paths touched by this tool
    pub todo_update: Option<(String, Vec<TodoItem>)>,
    pub project_todo_update: Option<(String, Vec<TodoItem>)>,
}
```

**File:** `crates/ai/src/chat/polling/tools.rs` (127 lines — "tool result collection, handoff detection, commit_tool_results"; this is the layer that wraps each call to `execute_tool_with_cache()`, not `chat/tools.rs` itself):

For each tool call, after `execute_tool_with_cache()` returns, extract file paths from the tool call arguments. Each tool handler already parses `args[path]` (or equivalent). Use the same parsing logic to fill `accessed_paths`:

```rust
let accessed_paths = match tc.name.as_str() {
    "read_file" | "read_entire_file" | "write_file"
    | "patch_file" | "patch_lines" | "delete_file"
    | "list_dir" | "grep" =>
        args["path"].as_str().map(|p| vec![p.to_string()]).unwrap_or_default(),
    "read_files" =>
        args["paths"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
    "rename_file" => {
        let mut paths = Vec::new();
        if let Some(p) = args["path"].as_str() { paths.push(p.to_string()); }
        if let Some(p) = args["new_path"].as_str() { paths.push(p.to_string()); }
        paths
    }
    _ => vec![],
};
results.push(ToolResult {
    tool_call: tc.clone(),
    content: result.to_string(),
    meta,
    accessed_paths,       // ← new
    todo_update,
    project_todo_update,
});
```

### Recording into FileAccessLog

**File:** `crates/ai/src/chat/session_ops.rs`, `push_tool_results_to_state()`:

After the existing loop that converts `ToolResult` → `ChatMessage` and calls `push_to_session()`, add a second pass:

```rust
if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
    let turn = sess.turn_count;
    for tr in results {
        let Some(op) = tool_name_to_op(&tr.tool_call.name) else { continue };
        for path in &tr.accessed_paths {
            sess.access_log.record(path, op, turn);
        }
    }
}
```

No message-id correlation needed — `FileAccessLog::record()` only takes `path`/`op`/`turn` now (see the simplified struct above), so this pass doesn't need to know which `ChatMessage` id each `ToolResult` became.

### Rebuild from disk on session load

**File:** `crates/core/src/storage/discovery.rs` (228 lines — "disk discovery: load/save project meta, discover projects/sessions, identity migration"; this is the right home, not `session_io.rs`):

After constructing the `Session` from `SessionMeta`, iterate all loaded messages and rebuild `access_log` from `ToolMeta`. Because `ChatMessage.turn` is now persisted (see above), this recovers *real* turn numbers instead of guessing `0` for everything:

```rust
for msg in &sess.messages {
    if let Some(meta) = &msg.tool_meta {
        if let (Some(path), Some(op)) = (&meta.file_path, tool_name_to_op(&meta.tool_name)) {
            sess.access_log.record(path, op, msg.turn);
        }
    }
}
// Recover the turn counter itself from the messages, instead of resetting
// to 0 on every reload — fixes the "less-informed after reload" edge case
// from the original draft entirely.
sess.turn_count = sess.messages.iter().map(|m| m.turn).max().unwrap_or(0);
```

This is O(messages) at session load and gives the full working set — and an accurate `turn_count` — immediately, with no degradation after reload.

---

## Core Pruning Algorithm

**File:** `crates/ai/src/chat/looping.rs` *(new — see File Organization above)*

### Pair grouping definition

A "pair group" = one `Role::Assistant` message + all consecutive `Role::Tool` **or** `Role::Error` messages that follow it, until the next `Role::User`, `Role::Assistant`, or `Role::System`. (A failed tool call shows up as `Role::Error`, not `Role::Tool` — make sure the boundary check only stops on User/Assistant/System, so Error messages get absorbed into the group like Tool messages do, not treated as an unexpected boundary.)

```
[System]  ← pinned, never removed
[User]    ← never removed by looping
[Assistant] ← group 1 start
[Tool]    ← part of group 1
[Error]   ← part of group 1 (a tool call that failed)
[User]    ← never removed
[Assistant] ← group 2 start
[Tool]    ← part of group 2
...
```

```rust
fn pair_groups(messages: &[ChatMessage]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        if messages[i].role == Role::Assistant {
            let start = i;
            let mut end = i;
            i += 1;
            while i < messages.len()
                && matches!(messages[i].role, Role::Tool | Role::Error)
            {
                end = i;
                i += 1;
            }
            groups.push((start, end));
        } else {
            i += 1; // System/User/anything else: not part of a group, skip
        }
    }
    groups
}
```

### Scoring

A small helper computes the few path-related facts a group needs, so the scoring loop and the unverified-edit filter don't each re-derive them differently:

```rust
struct GroupSignals {
    has_unverified_edit: bool,   // group made the most recent Edit to some path
    superseded_reference: bool,  // group's Read/Grep/Search on some path is now stale
    in_working_set: bool,        // group touches a path in active_working_set()
}

fn group_signals(sess: &Session, (start, end): (usize, usize), working_set: &HashSet<&str>) -> GroupSignals {
    let group_turn = sess.messages[start].turn; // Assistant message anchors the group's turn
    let mut s = GroupSignals { has_unverified_edit: false, superseded_reference: false, in_working_set: false };
    for msg in &sess.messages[start..=end] {
        let Some(meta) = msg.tool_meta.as_ref() else { continue };
        let Some(path) = meta.file_path.as_deref() else { continue };
        let Some(op) = tool_name_to_op(&meta.tool_name) else { continue };
        if working_set.contains(path) { s.in_working_set = true; }
        let Some(entry) = sess.access_log.entries.get(path) else { continue };
        match op {
            FileOp::Edit if entry.last_turn == group_turn => s.has_unverified_edit = true,
            FileOp::Read | FileOp::Grep | FileOp::Search if entry.last_turn > group_turn => {
                s.superseded_reference = true;
            }
            _ => {}
        }
    }
    s
}
```

| Condition | Score modifier |
|---|---|
| `signals.in_working_set` | +3 |
| Message role is Assistant (analysis is high signal) — counted per Assistant message in the group, normally just 1 | +1 |
| Tool result > 2000 tokens AND not `in_working_set` | −2 |
| Message role is `Role::Error` (the tool call failed) | −3 per Error message |
| `signals.superseded_reference` (a Read/Grep/Search the group did is now stale — a later group touched the same path) | −3, and this *overrides* the `in_working_set` bonus for that group |
| `signals.has_unverified_edit` | exempt — never enters the candidate pool, regardless of score (see below) |

**Unverified-edit exemption**: filter these groups out of the candidate pool *before* scoring the rest. They hold the only record of why a file looks the way it does, and must survive until something re-reads or re-edits the file (at which point `entry.last_turn` moves past `group_turn` and the exemption lifts naturally).

### Algorithm

```rust
pub fn apply_looping_window(state: &mut AppState, session_id: &str) -> Option<()> {
    let idx = state.sessions.iter().position(|s| s.id == session_id)?;
    if !state.sessions[idx].looping_window { return None; }

    // Resolve the active model's aggressiveness config.
    // provider.rs already has the pattern for looking up the active model — follow it.
    let agg = state.active_model_aggressiveness(session_id)
        .unwrap_or(LoopAggressiveness::Balanced);

    // Token-based trigger: only prune when the session is sufficiently full.
    // session.estimated_tokens and model context_window are already tracked —
    // verify the exact field names against session.rs and provider.rs.
    let ctx_window = state.active_model_context_window(session_id).unwrap_or(200_000);
    let used_tokens = state.sessions[idx].estimated_tokens;
    let trigger_pct = agg.trigger_pct();
    if used_tokens as f32 / ctx_window as f32 < trigger_pct {
        return None; // not full enough yet
    }

    // Pair-group the messages
    let groups = pair_groups(&state.sessions[idx].messages);
    if groups.len() < 2 { return None; } // need at least 2 to have anything removable

    let turn = state.sessions[idx].turn_count;
    let working_set = state.sessions[idx].access_log.active_working_set(turn, 10);

    // Floor: always keep the newest N% of groups (min 2).
    let keep_floor = ((groups.len() as f32 * agg.recency_floor_pct()) as usize).max(2);
    let removable_end = groups.len().saturating_sub(keep_floor);
    if removable_end == 0 { return None; }

    // Score candidates. Remove exactly 1 group per trigger — one removal at a
    // time means the next trigger sees updated token counts and working-set
    // data before making the next decision, rather than bulk-removing N groups
    // with stale information and potentially over-pruning.
    let mut scored: Vec<(usize, i32)> = groups[..removable_end].iter().enumerate()
        .filter_map(|(gi, &(start, end))| {
            let signals = group_signals(&state.sessions[idx], (start, end), &working_set);
            if signals.has_unverified_edit { return None; }
            let mut score = 0i32;
            for msg in &state.sessions[idx].messages[start..=end] {
                if msg.role == Role::Assistant { score += 1; }
                if msg.role == Role::Tool && msg.full_token_estimate > 2000 && !signals.in_working_set {
                    score -= 2;
                }
                if msg.role == Role::Error { score -= 3; }
            }
            if signals.superseded_reference {
                score -= 3;
            } else if signals.in_working_set {
                score += 3;
            }
            Some((gi, score))
        }).collect();

    if scored.is_empty() { return None; }

    // Pick the single lowest-scoring group (tie-break: older first via stable sort).
    scored.sort_by_key(|&(_, s)| s);
    let to_remove: HashSet<u64> = scored.iter()
        .take(agg.remove_per_trigger())
        .flat_map(|&(gi, _)| {
            let (start, end) = groups[gi];
            state.sessions[idx].messages[start..=end].iter().map(|m| m.id).collect::<Vec<_>>()
        })
        .collect();

    if to_remove.is_empty() { return None; }

    // Build one breadcrumb message per removed group summarizing what was
    // dropped, keyed by group start index, so the model sees a marker
    // instead of a silent gap (cheap: ~1 line of text per group). Give the
    // breadcrumb the *current* turn — it's a synthetic message created now,
    // not a record of when the original group happened.
    let breadcrumb_for: HashMap<usize, ChatMessage> = scored.iter().take(excess)
        .map(|&(gi, _)| {
            let (start, end) = groups[gi];
            let paths: HashSet<String> = state.sessions[idx].messages[start..=end].iter()
                .filter_map(|m| m.tool_meta.as_ref()?.file_path.clone())
                .collect();
            let summary = if paths.is_empty() {
                "[pruned: 1 turn, no file activity]".to_string()
            } else {
                format!("[pruned: 1 turn — touched {}]", paths.into_iter().collect::<Vec<_>>().join(", "))
            };
            let mut bc = ChatMessage::prune_marker(summary);
            bc.turn = turn;
            (start, bc)
        }).collect();

    // Remove from disk (only if session has a project)
    let pid = state.sessions[idx].project_id.clone();
    if let Some(ref pid) = pid {
        if let Some(proj) = state.projects.iter().find(|p| p.id == *pid) {
            let msg_dir = autocode_core::storage::session_messages_dir(proj, &state.sessions[idx]);
            // remove_messages_by_id is in crates/core/src/storage/session_io.rs (389 lines —
            // "session dir management, atomic JSON writes, load/save/delete session meta & messages").
            // Verify the exact function signature there before calling.
            let _ = autocode_core::storage::remove_messages_by_id(&msg_dir, &to_remove);
            // Append each breadcrumb through whatever disk-write path
            // push_to_session() already uses for new messages (the chunked
            // JSONL appender in core/src/storage/chunked_jsonl.rs) — don't
            // add a second, separate "append_message" entry point.
            for bc in breadcrumb_for.values() {
                let _ = autocode_core::storage::chunked_jsonl::append(&msg_dir, bc);
            }
        }
    }

    // Rebuild RAM vec in one linear pass: at each removed group's start
    // index, emit the breadcrumb once and skip the group's original
    // messages; everything else (kept groups, System/User messages
    // between groups) passes through unchanged. Avoids index drift from
    // splicing into a vec after a retain().
    let old_messages = std::mem::take(&mut state.sessions[idx].messages);
    let mut new_messages = Vec::with_capacity(old_messages.len());
    let mut i = 0;
    while i < old_messages.len() {
        if let Some(bc) = breadcrumb_for.get(&i) {
            new_messages.push(bc.clone());
            // skip to end of this removed group
            let (_, end) = groups.iter().find(|&&(s, _)| s == i)
                .expect("breadcrumb key is always a group start index");
            i = end + 1;
        } else if to_remove.contains(&old_messages[i].id) {
            // mid-group message of a removed group whose start we already
            // passed (shouldn't happen given groups are contiguous, but
            // guard anyway) — skip without re-emitting a breadcrumb
            i += 1;
        } else {
            new_messages.push(old_messages[i].clone());
            i += 1;
        }
    }
    state.sessions[idx].messages = new_messages;

    // Recompute token estimate from disk (source of truth)
    let sid = session_id.to_string();
    crate::session_ops::recompute_estimate_from_disk(state, &sid);

    // Shrink RAM vec to release excess capacity after removals
    state.sessions[idx].messages.shrink_to_fit();

    Some(())
}
```

---

## Trigger Points

**File:** `crates/ai/src/chat/polling/mod.rs` (131 lines — owns the `update_runtime`/`update_all` frame loop that ties `stream.rs` and `tools.rs` together)

- One call to `apply_looping_window()` at the end of the frame loop, after a tool-commit *or* a text-only completion has landed for that session. This replaces the original 3-scattered-call-sites idea (after tool commit in `polling/tools.rs`, after streaming text in `polling/stream.rs`) — calling once per frame from the loop that already sequences both paths is simpler and avoids redundant O(n) scans when several pushes happen in one frame.

**File:** `crates/ai/src/chat/completion.rs`

- A second call at the start of `start_completion()`, before building the API request — the belt-and-suspenders guarantee that context is trimmed immediately before going to the wire even if something landed outside the normal frame-loop path.

---

## Toggle Button

**File:** `crates/ui/src/toolbar/layout.rs` (85 lines total — add after the "Reasoning" toggle near the end of the right-side toggles block; verify exact insertion point against current file)

```rust
// Looping window toggle (lights up when enabled).
let looping_active = active_session.map(|s| s.looping_window).unwrap_or(false);
if buttons::lit_btn(ui, "Loop", looping_active)
    .on_hover_text("Looping window: prune old messages when context fills")
    .clicked()
{
    if let Some(sid) = state.active_session_id.as_ref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == *sid)
    {
        sess.looping_window = !sess.looping_window;
        state.session_meta_dirty = true;
    }
}
```

---

## UI: Token Meter Help Text

**File:** `crates/ui/src/toolbar/meters.rs`

When `sess.looping_window` is true, append to hover text:  
`"Looping: keep ~N pairs (max: {sess.loop_max_pairs})"`

---

## Disable Handoff When Looping

**File:** `crates/ai/src/chat/completion.rs`, `check_auto_handoff()` function — this file already "handles handoff & auto-continue," so this is where the existing function lives, not `polling.rs`/`polling/mod.rs`:

```rust
// Suppress auto-handoff when looping window is active.
if let Some(sess) = state.sessions.iter().find(|s| s.id == session_id) {
    if sess.looping_window { return; }
}
```

Only auto-handoff is suppressed. Manual handoff tool calls still work.

---

## Turn Count Increment

**File:** `crates/ai/src/chat/completion.rs`, `start_completion()`

At the start of each completion cycle (after the early-return checks, before building the request):

```rust
// Increment turn counter for FileAccessLog working-set calculations.
if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
    sess.turn_count = sess.turn_count.saturating_add(1);
}
```

---

## Prune-Marker Message

**File:** `crates/core/src/state/chat.rs`

Breadcrumbs are a lightweight `ChatMessage` variant so they render distinctly in the UI and never get mistaken for real assistant/tool content:

```rust
impl ChatMessage {
    /// A synthetic, non-pinned marker left behind when `apply_looping_window`
    /// removes a group. Role::System so it can't break tool_use/tool_result
    /// alternation, but tagged `is_prune_marker` so the UI renders it as a
    /// muted one-liner instead of a real system message, and so the pruning
    /// pass itself never re-scores or removes a marker it already placed.
    pub fn prune_marker(summary: String) -> Self;
}
```

Add `is_prune_marker: bool` (default `false`) to `ChatMessage`. `pair_groups()` should skip marker messages entirely when grouping (they sit outside any group, same as System/User).

Also add `is_error: bool` to `ToolMeta` in the same file (default `false`). Set this to `true` in `commit_tool_results()` (`crates/ai/src/chat/polling/tools.rs`) when the tool result content begins with the structured JSON error prefix produced by `tool_error.rs`. This lets the UI (`crates/ui/src/chat/tool_result.rs`) and `looping.rs` scoring distinguish a tool call that errored from one that succeeded without re-parsing the content string — the `Role::Error` on `ChatMessage` already signals this at the message level, but `ToolMeta.is_error` makes it available when iterating a group's messages without re-checking the role on each.

---

## Dry-Run Mode

**File:** `crates/core/src/state/session.rs` / `crates/ai/src/chat/session_ops.rs`

```rust
#[serde(default)]
pub loop_dry_run: bool,
```

**Important**: `loop_dry_run` is intentionally **not** added to `SessionMeta` and is **not** persisted to disk. It is a development/debugging flag only — set it at runtime (e.g. via `AppState` or a compile-time constant) before a session starts, not via the per-session settings UI. This prevents it from accidentally shipping enabled in a saved session file. The files-touched table lists `session.rs` and `session_ops.rs` for this field; `session_meta.rs` is not in scope for it.

When set, `apply_looping_window()` computes `to_remove` and the breadcrumb summaries exactly as normal but skips the disk/RAM mutation, logging the candidate groups, scores, and reasons (working-set hit, superseded, error, unverified-edit-exempt) at `info` level instead. Lets us validate the scoring heuristics against real long sessions before flipping the toggle on by default — flip `loop_dry_run` off once the logs look sane for a given workload.

---

## Files Touched (Complete List)

### New Files
| File | Purpose |
|---|---|
| `crates/core/src/state/access_log.rs` | `FileAccessLog`, `AccessEntry`, `FileOp` |
| `crates/ai/src/chat/looping.rs` | `apply_looping_window()`, `pair_groups()`, `is_unverified_edit_group()`, scoring, breadcrumb construction, dry-run logging, plus inline `#[cfg(test)]` unit tests on the pure scoring/grouping functions |

### Modified — Core State
| File | Changes |
|---|---|
| `crates/core/src/state/mod.rs` | Add `pub mod access_log;` |
| `crates/core/src/state/session.rs` | Add `looping_window`, `loop_dry_run`, `turn_count`, `access_log`; remove `loop_max_pairs`; update `new()` |
| `crates/core/src/state/chat.rs` | Add `is_prune_marker: bool` to `ChatMessage`; add `ChatMessage::prune_marker()` constructor; add `is_error: bool` to `ToolMeta` |
| `crates/core/src/state/app_state.rs` | Add helper methods `active_model_aggressiveness()` and `active_model_context_window()` for use by `looping.rs`; remove `loop_max_pairs` field |
| `crates/core/src/state/provider.rs` | Add `LoopAggressiveness` enum; add `loop_aggressiveness: LoopAggressiveness` to per-model config struct |
| `crates/core/src/helpers/serde_defaults.rs` | Remove `default_loop_max_pairs()` — no longer needed |
| `crates/core/src/helpers/mod.rs` | Remove export of `default_loop_max_pairs` |

### Modified — Session Persistence
| File | Changes |
|---|---|
| `crates/core/src/storage/session_meta.rs` | Add `looping_window` to `SessionMeta`; `from_session()`. **Not** `loop_max_pairs` (removed) or `loop_dry_run` (not persisted). |
| `crates/core/src/storage/discovery.rs` | Add `looping_window` to direct `Session` construction; rebuild `access_log` from `ToolMeta` on load |

### Modified — AI / Chat Logic
| File | Changes |
|---|---|
| `crates/ai/src/chat/runtime.rs` | Add `accessed_paths: Vec<String>` to `ToolResult` |
| `crates/ai/src/chat/tools.rs` | No change — this file only executes tools; access recording happens one layer up in `polling/tools.rs` |
| `crates/ai/src/chat/polling/tools.rs` | Populate `ToolResult.accessed_paths` per tool call, inside the existing result-collection pass that builds each `ToolResult` before `commit_tool_results` |
| `crates/ai/src/chat/polling/mod.rs` | Call `looping::apply_looping_window()` once at the end of the frame loop, after either a tool-commit or a text-only completion lands |
| `crates/ai/src/chat/session_ops.rs` | Add the access-recording pass in `push_tool_results_to_state()` only — the scoring/removal algorithm itself lives in `looping.rs`, not here |
| `crates/ai/src/chat/looping.rs` *(new)* | `apply_looping_window()`, `pair_groups()`, `is_unverified_edit_group()`, breadcrumb construction, `loop_dry_run` log-only branch |
| `crates/ai/src/chat/completion.rs` | Call `looping::apply_looping_window()` at the start of `start_completion()`; increment `sess.turn_count`; suppress auto-handoff in `check_auto_handoff()` when `looping_window` is set |
| `crates/ai/src/chat/mod.rs` | Add `pub mod looping;`; export `apply_looping_window` |

### Modified — UI
| File | Changes |
|---|---|
| `crates/ui/src/toolbar/layout.rs` | Add "Loop" toggle button (alongside the existing "Reasoning" toggle among the right-side toggles) |
| `crates/ui/src/toolbar/meters.rs` | Update hover text when looping active to show current aggressiveness setting |
| `crates/ui/src/chat/session.rs` | Restore `looping_window` on tab switch (this file already owns save_old/load_new lifecycle, so no separate `app.rs` change needed) |
| `crates/ui/src/settings/providers.rs` | Add `LoopAggressiveness` picker per model (Conservative / Balanced / Aggressive) alongside the existing handoff threshold control. This is the right home since aggressiveness is per-model config, not per-session. Add `loop_dry_run` toggle here too, clearly labeled as a developer/debug option. |

**Note on settings location**: `loop_dry_run` moves out of `settings/session.rs` and into `settings/providers.rs` — it's a per-model debug flag, not a per-session user setting. `settings/session.rs` no longer needs any changes for this feature since `loop_max_pairs` is removed.

**Total: 2 new files + 19 modified files = 21 files.**

---

## Testing & Validation

Before enabling the toggle by default:

1. **API-shape invariant test** (the one that matters most): after any `apply_looping_window()` run, assert that the resulting `messages` slice never has an `Assistant` message with tool calls that isn't immediately followed by matching `Tool` results, and never has an orphaned `Tool` message with no preceding `Assistant`. `core/tests/stability.rs` already simulates 70 sessions / 7,000 messages and crash-recovery round-trips — that's the natural place to add a looping-enabled variant of that simulation and assert the invariant holds under realistic load, rather than standing up a separate test harness. Pure-function cases (varying group sizes, error mixes, interleaved markers) can stay as fast inline `#[cfg(test)]` unit tests in `looping.rs` itself.
2. **Token threshold test**: confirm that with a session at 74% of context window and `Balanced` aggressiveness (trigger 75%), `apply_looping_window()` returns `None` without touching messages; at 76% it enters the scoring path. Test all three aggressiveness levels at their boundaries.
3. **Single-group removal test**: confirm that even when many groups are scoreable, exactly 1 is removed per call, and a second call after re-checking the threshold may or may not fire depending on whether tokens dropped below the threshold after the first removal.
4. **Unverified-edit exemption test**: a session with an Edit on `path` and no later access to `path` — confirm that group is never in `to_remove` even when above the trigger threshold.
5. **Dry-run first**: run with `loop_dry_run = true` on a handful of real long sessions, eyeball the logged candidates/scores, then flip it off for that workload.

---

## Data Flow

```
Tool dispatch (polling/tools.rs)
    │  extract path from tc.arguments
    ▼
ToolResult.accessed_paths
    │
    ▼
push_tool_results_to_state() (session_ops.rs)
    │  record paths → session.access_log
    ▼
FileAccessLog
    │
    ▼
apply_looping_window() (looping.rs, new)
    │  query working_set → score groups → remove lowest
    ▼
Session.messages (RAM) + remove_messages_by_id() (disk)
    │
    ▼
Next API request sees only the kept window
```

---

## Edge Cases

1. **Access log empty on reload**: Rebuilt from `ToolMeta.file_path` in `discovery.rs` at session load time.

2. **No project for session**: `remove_messages_by_id()` is skipped when `project_id` is `None`. RAM-only sessions still prune.

3. **Replay**: Cannot replay to a message earlier than the first retained message. Acceptable — replay is for rewinding within the current window.

4. **Handoff suppression**: Only auto-handoff is blocked. Manual `handoff` tool call still works.

5. **Token estimates**: `recompute_estimate_from_disk()` runs after pruning so the meter stays accurate and the next trigger check has fresh numbers.

6. **Turn count continuity**: Recovered accurately from persisted `ChatMessage.turn` values on reload — no degradation after reload.

7. **Breadcrumb accumulation**: Over a very long looping session, breadcrumbs themselves accumulate (1 per pruned group). They're tiny (~1 line) and excluded from `pair_groups()`, so they don't get re-pruned or re-scored, but a future pass could coalesce consecutive breadcrumbs into one if this becomes visually noisy.

8. **All candidates edit-exempt**: if every scoreable group has `has_unverified_edit`, `scored` is empty and the function returns `None`. The session will run over the trigger threshold temporarily rather than discard the only record of a write. Worth logging when this happens — it may mean the model is writing files without ever reading them back, which is itself a signal of a problem.

9. **Single removal may not drop below threshold**: one group removal might not be enough tokens to drop below the trigger percentage. That's fine — the next frame-loop tick will fire again and remove another. Converges naturally across a few cycles without bulk-removing with stale data.

10. **Aggressiveness mismatch across models**: if a user switches the active model mid-session, the next trigger check will use the new model's aggressiveness and context window. This is correct — the new model is what's about to receive the context, so its limits are what matter.