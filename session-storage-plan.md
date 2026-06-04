# Session Storage & Eviction Plan

## Goal
Move session storage from a single monolithic `app.ron` to per-session files, evicting inactive sessions from RAM and reloading on demand. Sessions are stored per-project for portability.

## Data Layout

```
$PROJECT_ROOT/.autocode/
  sessions/
    fixing_chat_pruning_a1b2c3.ron
    adding_handoff_b2c3d4.ron
    implementing_glm_support_c3d4e5.ron
```

- Global config (providers, settings, projects list) stays in `$APPDATA/autocode/data/app.ron`
- Session messages live in individual files under `.autocode/sessions/` inside each project
- `AppState.sessions` in memory holds only session metadata (id, label, created_at, total_tokens_used, project_id), NOT `messages`

## File Naming (`name_session` tool)

Model names the session via a hidden tool call — no keyword extraction needed.

### Tool definition
```json
{
  "name": "name_session",
  "description": "Set a descriptive label for this session. Call this early so the session gets a meaningful filename and tab name.",
  "parameters": {
    "name": { "type": "string", "description": "Short descriptive name, e.g. 'fixing_chat_pruning'" }
  }
}
```

### Filename
`{name}_{short_session_id}.ron`

Example: model calls `name_session` with `"implementing_glm_support"` → file is `implementing_glm_support_c3d4e5.ron`

### Hidden from chat
- Assistant message that fires the `name_session` tool call is skipped in rendering (same as existing empty-assistant-with-tool-calls logic)
- The `name_session` tool result message is also hidden via a `hidden: true` flag in `ToolMeta` or by checking `tool_name == "name_session"` in the render loop
- The session tab label updates in real time when the tool fires

## Steps

### Step 1 — `name_session` tool
- Add tool definition to `provider.rs`
- Add handler in `chat.rs` `execute_tool_with_cache`: stores the name, updates the session's label in `AppState`
- Update system prompt: "Call `name_session` early in each session to name it"
- Hide the tool call + result from chat rendering

### Step 2 — Project data dir discovery
- Add `AppState::project_data_dir(&self) -> Option<PathBuf>` returning `$PROJECT_ROOT/.autocode`
- Create dir on first use if missing
- Add `AppState::sessions_dir(&self) -> Option<PathBuf>` returning `$PROJECT_ROOT/.autocode/sessions/`

### Step 3 — Session filename from label
- `Session::filename()` derives the filename from the session label + short ID
- Label is set by `name_session` tool (or falls back to the auto-generated label like `Sd8df0000`)

### Step 4 — Isolate session data from AppState serialization
- Mark `Session.messages` and `Session.reasoning_content` with `#[serde(skip)]` so `save()` doesn't write them to `app.ron`
- Session metadata (id, label, created_at, project_id, actual_tokens_used) still serializes in `app.ron` for tab rendering

### Step 5 — Save session to file
- Add `Session::save_to_disk(&self, dir: &Path)` that:
  - Serializes ONLY messages and reasoning_content to a RON file
  - Writes to `dir / self.filename()`
  - Write to temp file first, then atomically rename

### Step 6 — Load session from file
- Add `Session::load_from_disk(&mut self, dir: &Path)` that:
  - Reads the RON file at `dir / self.filename()`
  - Deserializes and populates `self.messages`
  - On missing/corrupt file: log warning, leave messages empty

### Step 7 — Hook into session switch
- In `ui_chat.rs` `show()`:
  - **Before** switching away: if current session has messages, call `save_to_disk()`, then clear messages
  - **After** switching to new session: if messages are empty and a file exists, call `load_from_disk()` to restore
  - Replaces the current scroll-offset save/restore

### Step 8 — Load on startup
- In `AppState::load()`:
  - After loading from eframe storage, iterate `self.sessions`
  - Only load messages for the ACTIVE session into RAM
  - All other sessions stay evicted (messages empty, filename known from label + id)

### Step 9 — Crash safety
- `save_to_disk` writes to a temp file first, then atomically renames (or uses `fs::rename` which is atomic on the same drive)
- On load, if a session file is missing/corrupt, log a warning and start fresh
- `app.ron` retains enough metadata to rebuild the tab list even if all session files are lost

### Step 10 — Cleanup
- Auto-clean orphaned `.ron` files (files without a matching `Session` in `state.sessions`)
- GC: limit max sessions per project to N (default 50), evict oldest files
- Add "Delete all sessions for this project" button in Settings
- Auto-add `.autocode/` to project `.gitignore` on first use

## Open Questions
- Handle concurrent projects? Each has its own `.autocode/`, switching projects swaps the session set entirely
- Compression? RON is text but gzip could save disk space — probably overkill for chat messages
