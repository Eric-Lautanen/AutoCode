# Looping Context Window — Implementation Plan

## Concept

A toggleable mode that prunes old assistant/tool message pairs when the session exceeds a threshold (e.g. 150 assistant+tool messages), keeping the system prompt and recent exchanges. A `FileAccessLog` tracks which files the model has accessed so the pruning algorithm can retain messages that reference actively-used files.

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
    /// Message IDs of messages where this file was referenced (tool results, assistant analysis).
    pub msg_ids: Vec<u64>,
}

impl FileAccessLog {
    pub fn new() -> Self;
    /// Record a file access at tool execution time.
    /// `path` = resolved absolute path. `msg_id` = the tool-result ChatMessage's id.
    pub fn record(&mut self, path: &str, op: FileOp, turn: u64, msg_id: u64);
    /// Returns paths accessed within the last `within_turns` turns from `current_turn`.
    pub fn active_working_set(&self, current_turn: u64, within_turns: u64) -> HashSet<&str>;
    /// Returns true if any of the given `msg_ids` overlap with entries
    /// whose paths are in `working_set`.
    pub fn references_working_set(&self, msg_ids: &[u64], working_set: &HashSet<&str>) -> bool;
}

pub enum FileOp {
    Read = 1,
    Edit = 2,
    Grep = 4,
    Glob = 8,
    Search = 16,
}
```

**Storage**: Not persisted to disk. Rebuilt from `ToolMeta.file_path` on session reload. At ~200-300 bytes per entry (HashMap + String overhead), a 200-file session uses ~60KB.

---

### 2. New Fields on `Session` / `SessionMeta`

**File:** `crates/core/src/state/session.rs`

```rust
/// When true, old message pairs are pruned when the session exceeds
/// `loop_max_pairs`. Also disables auto-handoff.
#[serde(default)]
pub looping_window: bool,

/// Max number of assistant+tool message pairs before pruning triggers.
/// 0 = use AppState-level default.
#[serde(default)]
pub loop_max_pairs: u32,

/// Monotonic turn counter. Incremented at the start of each completion
/// cycle in `start_completion()`. Used by FileAccessLog.
#[serde(default)]
pub turn_count: u64,

/// In-memory file access log (not serialized — rebuilt from ToolMeta on load).
#[serde(skip)]
pub access_log: FileAccessLog,
```

Add defaults in `Session::new()`: all `false`/`0`/`FileAccessLog::new()`.

**File:** `crates/core/src/storage/session_meta.rs`

```rust
#[serde(default)]
pub looping_window: bool,
#[serde(default)]
pub loop_max_pairs: u32,
```

Add mapping in `SessionMeta::from_session()` (line 81).

**File:** `crates/core/src/storage/discovery.rs`

The `discover_sessions_from_disk()` function at line 170 constructs `Session` structs by direct field assignment from `SessionMeta`. Add:

```rust
looping_window: meta.looping_window,
loop_max_pairs: meta.loop_max_pairs,
```

---

### 3. New Fields on `AppState`

**File:** `crates/core/src/state/app_state.rs`

```rust
#[serde(default = "default_loop_max_pairs")]
pub loop_max_pairs: u32,
```

**File:** `crates/core/src/helpers/serde_defaults.rs`

```rust
pub fn default_loop_max_pairs() -> u32 { 150 }
```

Export via `crates/core/src/helpers/mod.rs`.

---

## Tool-Layer Instrumentation

**Key constraint**: `execute_tool_with_cache()` in `tools.rs:381` takes `ToolExecCtx` which has no access to `Session` or `AppState`. File access logging cannot happen inside the tool function itself.

### Solution: widen `ToolResult`

**File:** `crates/ai/src/chat/runtime.rs`, `ToolResult` struct (line 66):

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

**File:** `crates/ai/src/chat/polling.rs`, dispatch code at lines 716-775 (inside the spawned thread):

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

**File:** `crates/ai/src/chat/session_ops.rs`, `push_tool_results_to_state()` (line 233):

After the existing loop that converts `ToolResult` → `ChatMessage` and calls `push_to_session()`, add a second pass:

```rust
if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
    let turn = sess.turn_count;
    for tr in results {
        let op = match tr.tool_call.name.as_str() {
            "read_file" | "read_entire_file" | "read_files" => FileOp::Read,
            "write_file" | "patch_file" | "patch_lines" => FileOp::Edit,
            "grep" | "search" => FileOp::Grep,
            "glob" => FileOp::Glob,
            "list_dir" | "project_tree" => FileOp::Search,
            _ => continue,
        };
        for path in &tr.accessed_paths {
            sess.access_log.record(path, op, turn, msg_id);
        }
    }
}
```

Where `msg_id` is the `ChatMessage` id that was assigned in the push loop above (the tool result message).

### Rebuild from disk on session load

**File:** `crates/core/src/storage/discovery.rs` (line 204-209) or `session_io.rs`:

After constructing the `Session` from `SessionMeta`, iterate all loaded messages and rebuild `access_log` from `ToolMeta`:

```rust
for msg in &sess.messages {
    if let Some(meta) = &msg.tool_meta {
        if let Some(path) = &meta.file_path {
            let op = match meta.tool_name.as_str() { /* same match as above */ };
            sess.access_log.record(path, op, 0, msg.id);
        }
    }
}
```

This is O(messages) at session load and gives the full working set immediately.

---

## Core Pruning Algorithm

**File:** `crates/ai/src/chat/session_ops.rs` (new function)

### Pair grouping definition

A "pair group" = one `Role::Assistant` message + all consecutive `Role::Tool` messages that follow it, until the next `Role::User`, `Role::Assistant`, or `Role::System`.

```
[System]  ← pinned, never removed
[User]    ← never removed by looping
[Assistant] ← group 1 start
[Tool]    ← part of group 1
[Tool]    ← part of group 1
[User]    ← never removed
[Assistant] ← group 2 start
[Tool]    ← part of group 2
...
```

`count_assistant_tool_pairs()` = number of `Role::Assistant` messages.

```
fn pair_groups(messages: &[ChatMessage]) -> Vec<(usize, usize)> {
    // Returns Vec of (start_idx, end_idx) for each assistant+tool group.
    // Groups are contiguous. Returns entries for assistant messages only.
}
```

### Scoring

Each pair group gets a score. The score is the sum of individual messages within it:

| Condition | Score modifier |
|---|---|
| Any message in group references a file in `working_set` | +3 |
| Message role is Assistant (analysis is high signal) | +1 |
| Tool result > 2000 tokens AND no working-set references | −2 |

"References a file in working_set" = the message ID appears in any `AccessEntry.msg_ids` whose path is in the working set, checked via `FileAccessLog::references_working_set()`.

### Algorithm

```rust
pub fn apply_looping_window(state: &mut AppState, session_id: &str) -> Option<()> {
    let idx = state.sessions.iter().position(|s| s.id == session_id)?;
    if !state.sessions[idx].looping_window { return None; }

    let max_pairs = {
        let s = &state.sessions[idx];
        if s.loop_max_pairs > 0 { s.loop_max_pairs as usize }
        else { state.loop_max_pairs as usize }
    };
    if max_pairs == 0 { return None; }

    // Pair-group the messages
    let groups = pair_groups(&state.sessions[idx].messages);
    if groups.len() <= max_pairs { return None; }
    let excess = groups.len() - max_pairs;

    let turn = state.sessions[idx].turn_count;
    let working_set = state.sessions[idx].access_log.active_working_set(turn, 10);

    // Floor: always keep the newest 30% of groups
    let keep_floor = (max_pairs as f32 * 0.3) as usize;
    let removable_end = groups.len() - keep_floor;        // oldest N - floor groups are candidates

    // Score candidates (oldest N - floor groups)
    let mut scored: Vec<(usize, i32)> = groups[..removable_end].iter().enumerate().map(|(gi, &(start, end))| {
        let mut score = 0i32;
        let mut msg_ids_in_group: Vec<u64> = Vec::new();
        for msg in &state.sessions[idx].messages[start..=end] {
            msg_ids_in_group.push(msg.id);
            if msg.role == Role::Assistant { score += 1; }
            if msg.role == Role::Tool && msg.full_token_estimate > 2000 { score -= 2; }
        }
        if state.sessions[idx].access_log.references_working_set(&msg_ids_in_group, &working_set) {
            score += 3;
        }
        (gi, score)
    }).collect();

    // Remove lowest-scoring `excess` groups (tie-break: older first)
    scored.sort_by_key(|&(_, s)| s);
    let to_remove: HashSet<u64> = scored.iter()
        .take(excess)
        .flat_map(|&(gi, _)| {
            let (start, end) = groups[gi];
            state.sessions[idx].messages[start..=end].iter().map(|m| m.id).collect::<Vec<_>>()
        })
        .collect();

    if to_remove.is_empty() { return None; }

    // Remove from disk (only if session has a project)
    let pid = state.sessions[idx].project_id.clone();
    if let Some(ref pid) = pid {
        if let Some(proj) = state.projects.iter().find(|p| p.id == *pid) {
            let msg_dir = autocode_core::storage::session_messages_dir(proj, &state.sessions[idx]);
            let _ = autocode_core::storage::remove_messages_by_id(&msg_dir, &to_remove);
        }
    }

    // Remove from RAM
    state.sessions[idx].messages.retain(|m| !to_remove.contains(&m.id));

    // Recompute token estimate from disk (source of truth)
    let sid = session_id.to_string();
    crate::session_ops::recompute_estimate_from_disk(state, &sid);

    // Shrink RAM vec
    state.sessions[idx].messages.shrink_to(0);

    Some(())
}
```

---

## Trigger Points

**File:** `crates/ai/src/chat/polling.rs`

- After `push_tool_results_to_state()` near line 907 (tool results committed)
- After each `push_runtime()` for text-only responses near lines 527 and 830

**File:** `crates/ai/src/chat/completion.rs`

- At the start of `start_completion()` before building the API request, so the context is trimmed before going to the wire

---

## Toggle Button

**File:** `crates/ui/src/toolbar/layout.rs` (after "Reasoning" toggle at line 80)

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

**File:** `crates/ai/src/chat/polling.rs`, `check_auto_handoff()` function

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

## Files Touched (Complete List)

### New Files
| File | Purpose |
|---|---|
| `crates/core/src/state/access_log.rs` | `FileAccessLog`, `AccessEntry`, `FileOp` |

### Modified — Core State
| File | Changes |
|---|---|
| `crates/core/src/state/mod.rs` | Add `pub mod access_log;` |
| `crates/core/src/state/session.rs` | Add `looping_window`, `loop_max_pairs`, `turn_count`, `access_log`; update `new()` |
| `crates/core/src/state/chat.rs` | (if `ToolMeta` needs any accessor helpers) |
| `crates/core/src/state/app_state.rs` | Add `loop_max_pairs` field; `Default::default()` |
| `crates/core/src/helpers/serde_defaults.rs` | Add `default_loop_max_pairs()` → 150 |
| `crates/core/src/helpers/mod.rs` | Export `default_loop_max_pairs` |

### Modified — Session Persistence
| File | Changes |
|---|---|
| `crates/core/src/storage/session_meta.rs` | Add `looping_window`, `loop_max_pairs` to `SessionMeta`; `from_session()` |
| `crates/core/src/storage/session_io.rs` | (if needed) any load-side mapping |
| `crates/core/src/storage/discovery.rs` | Add `looping_window`, `loop_max_pairs` to direct `Session` construction at line 170; rebuild `access_log` from `ToolMeta` |

### Modified — AI / Chat Logic
| File | Changes |
|---|---|
| `crates/ai/src/chat/runtime.rs` | Add `accessed_paths: Vec<String>` to `ToolResult` (line 66) |
| `crates/ai/src/chat/tools.rs` | No change — access recording happens in `polling.rs` at dispatch time |
| `crates/ai/src/chat/polling.rs` | Populate `ToolResult.accessed_paths` at lines 764-770; call `apply_looping_window()` after tool-result flush and text commits; suppress auto-handoff in `check_auto_handoff()` |
| `crates/ai/src/chat/session_ops.rs` | Add `apply_looping_window()`, `pair_groups()`, access recording pass in `push_tool_results_to_state()` |
| `crates/ai/src/chat/completion.rs` | Call `apply_looping_window()`; increment `sess.turn_count` in `start_completion()` |
| `crates/ai/src/chat/mod.rs` | Export `apply_looping_window` |

### Modified — UI
| File | Changes |
|---|---|
| `crates/ui/src/toolbar/layout.rs` | Add "Loop" toggle button |
| `crates/ui/src/toolbar/meters.rs` | Update hover text when looping active |
| `crates/ui/src/app.rs` | Sync `looping_window` on session switch/load |
| `crates/ui/src/chat/session.rs` | Restore `looping_window` on tab switch |
| `crates/ui/src/settings/session.rs` | (optional) `loop_max_pairs` setting |

**Total: 1 new file + 17 modified files = 18 files.**

---

## Data Flow

```
Tool dispatch (polling.rs:716)
    │  extract path from tc.arguments
    ▼
ToolResult.accessed_paths
    │
    ▼
push_tool_results_to_state() (session_ops.rs:233)
    │  record paths → session.access_log
    ▼
FileAccessLog
    │
    ▼
apply_looping_window() (session_ops.rs, new)
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

5. **Token estimates**: `recompute_estimate_from_disk()` runs after pruning so the meter stays accurate.

6. **Turn count continuity**: Persisted across sessions via `#[serde(default)]` — not critical since it resets to 0 on old sessions, but working-set calculation will be less informed on first few turns after reload. The `within_turns` window (10) is generous enough that this is minor.
