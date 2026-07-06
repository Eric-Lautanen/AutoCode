# AutoCode State Persistence Audit

This audit maps every field of the in-RAM `AppState` struct to its storage layer,
identifies the source of truth for each, and documents the specific conflict points
where RAM / `app.ron` overwrites the disk-backed source of truth incorrectly.

**No source files were modified.** This document is the deliverable.

---

## 1. The three storage layers

| Layer | File | Source of truth for | Written by |
|---|---|---|---|
| **app.ron** | `AutoCode_data/app.ron` | eframe persisted UI/global state (the serialized `AppState` minus `#[serde(skip)]` fields) | `eframe::set_value` in `AutocodeApp::save` → `AppState::save` (`app_state.rs:506`); auto-saved every 10s (`app.rs:500`) |
| **providers.json** | `AutoCode_data/providers.json` | provider configs (api keys, base urls, models, per-model sampling params) | `save_providers_file` (`provider_file.rs:112`); called from `AppState::save` (`app_state.rs:503`) and settings close (`window.rs:171`) |
| **Per-project/session disk** | `AutoCode_data/projects/{data_dir}/meta.json`, `sessions/{id}_{label}/session.json`, `sessions/{id}_{label}/*.jsonl` | project identity + project_task_list; session metadata + session todo_list; append-only message history | `save_project_meta`, `save_session_meta`, `append_messages_to_jsonl` |

The disk (layers 2 and 3) is the source of truth. `app.ron` is a convenience cache
of the *last active session's* UI state. The bugs below arise because `app.ron`
holds per-session values as if they were global, and those values leak across
sessions and across restarts.

---

## 2. AppState field-by-field table

Struct definition: `crates/core/src/state/app_state.rs:154-274`.

| Field | serde | Stored in | Source of truth | Correct? |
|---|---|---|---|---|
| `projects: Vec<Project>` | `skip` | disk `meta.json` (per project) | disk | ✅ skipped — loaded from disk on startup (`app_state.rs:336`) |
| `active_project_id: Option<String>` | persist | app.ron | disk (project must exist) | ⚠️ persisted; validated only indirectly via `prune_disk_state` |
| `providers: HashMap<String, ApiProvider>` | `skip` | providers.json | providers.json | ✅ skipped — loaded from disk (`app_state.rs:356`) |
| `active_provider: String` | persist | app.ron | **session.json** (`provider_label`) | ❌ **CONFLICT** — see §3.1 |
| `sessions: Vec<Session>` | `skip` | disk `session.json` | disk | ✅ skipped — discovered from disk (`app_state.rs:346`) |
| `active_session_id: Option<String>` | persist | app.ron | disk (session must exist) | ⚠️ persisted; orphaned id cleared in `load`/`prune_disk_state` |
| `system_prompt: String` | persist | app.ron | app.ron (global) | ✅ genuinely global |
| `handoff_trigger_prompt` | persist | app.ron | app.ron (global) | ✅ genuinely global |
| `handoff_continuation_prompt` | persist | app.ron | app.ron (global) | ✅ genuinely global |
| `handoff_enabled: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `shell_tasks: Vec<ShellTask>` | `skip` | in-memory only | n/a | ✅ skipped |
| `show_explorer: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `explorer_width: f32` | persist | app.ron | app.ron (global UI) | ✅ genuinely global UI metric |
| `expanded_dirs: Vec<String>` | persist | app.ron | app.ron (global) | ✅ global, pruned on session delete |
| `show_todo: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `todo_user_dismissed: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `show_project_tasks: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `show_reasoning_inline: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `settings_open: bool` | persist | app.ron | **session.json** | ❌ **CONFLICT** — see §3.2 |
| `sysinfo` | persist | app.ron | app.ron (cached host info) | ✅ global; re-detected if stale |
| `stream_idle_timeout_secs` | persist | app.ron | app.ron (global) | ✅ global |
| `request_timeout_secs` | persist | app.ron | app.ron (global) | ✅ global |
| `tool_timeout_secs` | persist | app.ron | app.ron (global) | ✅ global |
| `shell_timeout_secs` | persist | app.ron | app.ron (global) | ✅ global |
| `shell_timeout_max_secs` | persist | app.ron | app.ron (global) | ✅ global |
| `max_retries` | persist | app.ron | app.ron (global) | ✅ global |
| `max_retry_wait_secs` | persist | app.ron | app.ron (global) | ✅ global |
| `ui_display_window` | persist | app.ron | app.ron (global) | ✅ global |
| `disk_read_delay_ms` | persist | app.ron | app.ron (global) | ✅ global |
| `web_rate_limit_ms` | persist | app.ron | app.ron (global) | ✅ global |
| `disk_write_rate_ms` | persist | app.ron | app.ron (global) | ✅ global |
| `pending_writes: PendingWrites` | `skip` | in-memory only | n/a | ✅ skipped |
| `session_meta_dirty: bool` | `skip` | in-memory only | n/a | ✅ skipped |

### Session struct fields (for cross-reference)

`crates/core/src/state/session.rs:7-125`. All of these are serialized into
`session.json` via `SessionMeta::from_session` (`session_meta.rs:83`). The ones
that also exist as "working copies" on `AppState` are the conflict candidates:

| Session field | Also on AppState? | Notes |
|---|---|---|
| `show_todo`, `todo_user_dismissed`, `handoff_enabled`, `show_explorer`, `settings_open`, `show_reasoning_inline`, `show_project_tasks` | ✅ yes | duplicated global working copies — §3.2 |
| `provider_label`, `model` | `active_provider` only | provider sync — §3.1 |
| `temperature`, `top_p`, `frequency_penalty`, `presence_penalty`, `requests_per_hour`, `handoff_percent` | on `ApiProvider` only | provider param sync — §3.1 |
| `thinking_mode`, `reasoning_effort` | on `ApiProvider` only (but read from session at request time) | see §3.3 |
| `looping_window` | no (session-only) | toggled directly on `sess` (`layout.rs:91`), saved via `session_meta_dirty` |
| `draft_input` | no (lives on `ChatPanelState.input`) | copied in/out in `save_old_session`/`load_new_session` |
| `messages`, `access_log`, `cached_tool_*`, `loop_dry_run` | `skip` | in-memory only |

---

## 3. Conflict points (RAM / app.ron overwrites disk)

### 3.1 Provider identity & sampling params — bidirectional sync hazard

**The duplication.** The active provider's identity and sampling parameters live in
THREE places simultaneously:

1. `providers.json` → `ApiProvider` (the global provider config: `kind`, `model`,
   `temperature`, `top_p`, `frequency_penalty`, `presence_penalty`,
   `requests_per_hour`, `handoff_percent`, `thinking_mode`, `reasoning_effort`,
   `max_context_tokens`, `max_output_tokens`, etc.) — `provider.rs:206-295`.
2. `session.json` → `Session` snapshot (`provider_label`, `model`,
   `temperature`, `top_p`, `frequency_penalty`, `presence_penalty`,
   `requests_per_hour`, `handoff_percent`, `thinking_mode`, `reasoning_effort`) —
   `session.rs:21-69`.
3. `AppState.active_provider: String` (just the label) — persisted to app.ron.

**Restore direction (disk → RAM) — at startup, `restore_active_session`
(`app.rs:161-187`):**
```rust
state.active_provider = label.clone();          // app.rs:164
prov.model = model.clone();                     // app.rs:178
prov.fill_from_config();                        // app.rs:179  ← reloads from providers.json
prov.temperature = temp;  prov.top_p = top_p;   // app.rs:180-185  ← OVERWRITES with session values
prov.frequency_penalty = freq; prov.presence_penalty = pres;
prov.requests_per_hour = rph; prov.handoff_percent = handoff;
```
The session's saved sampling params overwrite the active `ApiProvider` in RAM.
Note `fill_from_config()` is called *first* (line 179), which reloads
`temperature`/`top_p`/etc. from the provider's `models_config` (providers.json),
and then lines 180-185 immediately overwrite those with the session snapshot.
So **the session wins over providers.json in RAM** — but providers.json on disk
is not touched here.

**Save direction (RAM → disk) — `AutocodeApp::save` (`app.rs:465-493`):**
```rust
let provider_params = state.active_provider().map(|p| (p.temperature, p.top_p, ...));
if let Some(sess) = self.state.active_session_mut() {
    sess.provider_label = prov_label;
    sess.model = model;
    sess.temperature = temp;  sess.top_p = top_p;   // app.rs:487-492
    sess.frequency_penalty = freq; sess.presence_penalty = pres;
    sess.requests_per_hour = rph; sess.handoff_percent = handoff;
}
```
The active provider's *current* RAM params are copied back into the active
session struct, then `save_sessions()` writes `session.json`. So whatever the
provider's params are at save time become the session's params on disk.

**The conflict.** Because the *active provider* is a single shared object keyed
by `state.active_provider`, switching sessions does NOT switch the provider
object — it mutates the same `ApiProvider` in place. Consequence:

- **Bug A — cross-session provider param leakage.** Open session S1 (temperature
  0.2). The provider's `temperature` is set to 0.2. Switch to session S2
  (`load_new_session`, `session.rs:139-147`): it sets `state.active_provider =
  new_sess.provider_label` and calls `prov.fill_from_config()` — but it does
  **NOT** copy S2's `temperature`/`top_p`/etc. into the provider (compare to
  `restore_active_session` at startup which does, `app.rs:180-185`). So S2 runs
  with S1's sampling params until the next save overwrites S2's `session.json`
  with S1's values. **S1's params clobber S2 on disk.**
- **Bug B — settings edits leak into the wrong session.** A user edits
  temperature in Settings for the active provider (`providers.rs:601-602`:
  `if is_active { p.temperature = mc.temperature; }`). This mutates the shared
  `ApiProvider`. On the next auto-save (10s), `app.rs:487` copies that
  temperature into *whatever session is active*, writing it to that session's
  `session.json`. If the user then switches sessions, the just-edited value is
  now baked into the old session's disk file even though the edit was meant as a
  provider-level (providers.json) change. The provider-level change is also
  persisted to providers.json (`app_state.rs:503`), so the value exists in both
  places — but the session.json copy is stale-relative and will be re-imposed on
  that session the next time it's restored (`app.rs:180-185`), silently undoing a
  later providers.json edit for that session.
- **Bug C — `active_provider` persisted to app.ron survives restart and
  overrides disk.** `active_provider` is serialized to app.ron. On restart,
  `AppState::load` (`app_state.rs:385-393`) only checks that the label exists in
  `providers`; it does NOT validate it against the active session's
  `provider_label`. If the user changed providers in another session before
  quitting, app.ron's `active_provider` is the last session's provider, and
  `restore_active_session` (`app.rs:164`) then overwrites it with the session's
  `provider_label` — so for the *active* session this self-corrects. But if
  `active_session_id` is orphaned at startup, `restore_active_session` returns
  early (`app.rs:120`) and the stale `active_provider` from app.ron remains,
  pointing at a provider that may have nothing to do with any session the user
  next opens.

### 3.2 Per-session UI flags — global working copies persisted to app.ron

**The duplication.** Seven boolean UI flags exist on BOTH the `Session` struct
(source of truth: `session.json`) AND the global `AppState` (working copy,
persisted to app.ron):

| AppState field | Session field |
|---|---|
| `show_todo` | `sess.show_todo` |
| `todo_user_dismissed` | `sess.todo_user_dismissed` |
| `handoff_enabled` | `sess.handoff_enabled` |
| `show_explorer` | `sess.show_explorer` |
| `settings_open` | `sess.settings_open` |
| `show_reasoning_inline` | `sess.show_reasoning_inline` |
| `show_project_tasks` | `sess.show_project_tasks` |

**The sync flows:**

- **Startup restore** (`app.rs:144-150`): disk → global. `state.show_todo = sess.show_todo;` etc. ✅ correct (disk wins).
- **Session switch** (`load_new_session`, `session.rs:151-158`): disk → global. ✅ correct.
- **Save old session** (`save_old_session`, `session.rs:33-42`): global → old session struct → disk. ✅ correct *for the old session*.
- **eframe auto-save** (`app.rs:458-484`): global → active session struct → `save_sessions()` → disk. ✅ correct *for the active session*.
- **app.ron write** (`app.rs:497` → `AppState::save` → `storage.set("app_state", self)`): the global working copies are serialized to app.ron. ❌ This is the leak.

**The conflict — stale app.ron values survive restart and clobber the next session.**

On startup, `AppState::load` (`app_state.rs:332-408`) deserializes app.ron
FIRST. The seven global flags now hold whatever the *last active session* had at
the last save. Then disk projects/sessions are discovered. Then:

- If `active_session_id` is valid, `restore_active_session` (`app.rs:107`) runs
  and copies disk → global (lines 144-150), overwriting the stale app.ron
  values. ✅ self-corrects for the active session.
- **If `active_session_id` is orphaned/None at startup**, `AppState::load`
  clears only THREE of the seven flags (`app_state.rs:401-405`):
  ```rust
  if !active_ok {
      state.show_todo = false;
      state.todo_user_dismissed = false;
      state.settings_open = false;
  }
  ```
  It does **NOT** clear `handoff_enabled`, `show_explorer`,
  `show_reasoning_inline`, `show_project_tasks`. These four retain the stale
  app.ron values from the previous last-active session.

  Now the user opens a *different* session from the dropdown
  (`pickers.rs:87`: `state.active_session_id = Some(sid.clone())`). The next
  frame, `load_new_session` (`session.rs:151-158`) copies that session's disk
  values into the global flags — so the stale values are overwritten. ✅
  self-corrects on switch.

  **BUT:** between startup and the first session switch, the stale global flags
  are live in the UI. More importantly, `prune_disk_state` runs every 30s in the
  main loop (`app.rs:282-287`) and on every save. If `sessions.is_empty()`
  becomes true during a prune, `prune_disk_state` sets `handoff_enabled = false`
  (`app_state.rs:482`) but does NOT touch `show_explorer`,
  `show_reasoning_inline`, `show_project_tasks`. And if the active session is
  merely orphaned (not empty), `prune_disk_state` (`app_state.rs:483-490`) clears
  `active_session_id` but leaves ALL seven global flags untouched. So the stale
  app.ron UI flags persist indefinitely until a session is explicitly loaded.

- **The actual disk overwrite.** The real damage happens on the next eframe
  auto-save (≤10s after any change). `AutocodeApp::save` (`app.rs:475-494`)
  copies the global working-copy flags into the *active* session struct and
  writes `session.json`. If the active session was restored from disk and the
  user hasn't touched the flags, the global == disk, so no harm. But if the
  global flags came from a stale app.ron (orphaned-session startup path above)
  and a session is then activated WITHOUT going through `load_new_session`
  (e.g. via `new_session_for_project` → `ensure_session`, which does NOT copy
  session flags into global — it creates a fresh session with defaults), the
  stale global flags get written into the new session's `session.json` on the
  next save. **A brand-new session inherits the previous session's
  `show_explorer` / `show_reasoning_inline` / `show_project_tasks` /
  `handoff_enabled` from app.ron.**

  Concretely: `new_session_for_project` (`app_state.rs:617-642`) creates a
  `Session::new` (defaults: `show_explorer=true`, `handoff_enabled=true`,
  `show_reasoning_inline=false`, `show_project_tasks=false`) and sets
  `active_session_id`. It does NOT sync the global flags to the new session's
  values. The global flags still hold whatever was there. Then `AutocodeApp::save`
  (`app.rs:478-484`) writes those global flags back into the new session struct,
  overwriting its defaults. So the new session's `session.json` ends up with the
  stale global values, not its own defaults.

### 3.3 thinking_mode / reasoning_effort — half-duplicated state

`thinking_mode` and `reasoning_effort` exist on BOTH `ApiProvider`
(`provider.rs:230-234`) and `Session` (`session.rs:66-69`). However, unlike the
sampling params, they are **NOT** copied between provider and session in
`restore_active_session` or `AutocodeApp::save`. Instead:

- At request time, `start_completion` (`completion/mod.rs:137-175`) reads
  `sess.thinking_mode` and `sess.reasoning_effort` directly from the session,
  falling back to the provider only if the session's `reasoning_effort` is
  empty. So the **session is the effective source of truth at runtime**.
- The provider's `thinking_mode`/`reasoning_effort` are set only via
  `fill_from_config`/`fill_from_manifest`/`reset_defaults` (`provider.rs:414-493`)
  and are never propagated to the session on restore.
- The UI toggle (`input.rs:222`) and effort picker (`input.rs:293`,
  `pickers.rs:127,188`) write directly to `sess.thinking_mode` /
  `sess.reasoning_effort` and set `session_meta_dirty` (for the pickers) — but
  the thinking toggle in `input.rs:222` does **NOT** set `session_meta_dirty`.
  So toggling thinking mode mutates the session in RAM but the change is only
  persisted to `session.json` on the next `save_sessions` (auto-save / exit),
  not immediately. If the app crashes before the 10s auto-save, the thinking
  toggle is lost.

**Conflict:** the provider's `thinking_mode`/`reasoning_effort` in providers.json
and the session's in session.json can drift apart, and there is no sync path
between them. The session wins at request time, so a user editing the provider's
thinking settings in Settings (`providers.rs` — note: there is no UI editing
`p.thinking_mode` or `p.reasoning_effort` directly; they only change via model
selection) will see no effect for an existing session.

---

## 4. Bidirectional sync hazards (summary)

### 4.1 Provider params (temperature, top_p, freq/pres penalty, requests_per_hour, handoff_percent)

- **Live on:** `ApiProvider` (providers.json) AND `Session` (session.json).
- **Restore (startup):** session → provider in RAM (`app.rs:180-185`). Disk
  providers.json untouched.
- **Restore (session switch):** provider reloaded from providers.json via
  `fill_from_config` (`session.rs:145`), but session's params are **NOT** copied
  into the provider. ❌ **Asymmetric with startup restore.** This is Bug A in §3.1.
- **Save:** provider RAM → session struct → session.json (`app.rs:486-492`).
- **Settings edit:** writes to provider RAM + models_config; persisted to
  providers.json on settings close (`window.rs:171`) and on every auto-save
  (`app_state.rs:503`). Also leaks into the active session's session.json on the
  next auto-save (Bug B in §3.1).

**Net hazard:** switching away from a session and back can silently change its
sampling params, because the shared provider object carries the previous
session's values into the save of the next session. The startup path and the
switch path handle the session→provider copy inconsistently.

### 4.2 Per-session UI flags (the seven booleans)

- **Live on:** `Session` (session.json) AND `AppState` (app.ron).
- **Restore (startup, active session valid):** session → global (`app.rs:144-150`). ✅
- **Restore (startup, active session orphaned):** only 3 of 7 cleared
  (`app_state.rs:401-405`). ❌ 4 stale values survive.
- **Restore (session switch):** session → global (`session.rs:151-158`). ✅
- **New session:** global NOT synced to new session defaults
  (`new_session_for_project` doesn't touch global flags). ❌ stale global leaks
  into new session on next save.
- **Save:** global → active session → session.json (`app.rs:478-484`,
  `save_sessions`). Also global → app.ron (`app.rs:497`).

**Net hazard:** app.ron acts as a stale cache of "the last active session's UI
state" that can leak into newly created sessions and into the UI between
startup and first session switch.

---

## 5. Prioritized list of bugs / inconsistencies

### P0 — Data loss / silent overwrite of disk source of truth

1. **Session switch does not restore sampling params from disk into the
   provider** (`session.rs:139-147` vs `app.rs:180-185`). When switching to a
   session, `load_new_session` reloads the provider via `fill_from_config` but
   does NOT copy the session's `temperature`/`top_p`/`frequency_penalty`/
   `presence_penalty`/`requests_per_hour`/`handoff_percent` into the provider.
   The provider retains the *previous* session's values. The next auto-save
   writes those values into the new session's `session.json`, **overwriting the
   disk source of truth**. This is the asymmetric counterpart of the startup
   restore path and is the most direct "RAM overwrites disk" bug.

2. **New sessions inherit stale global UI flags from app.ron.**
   `new_session_for_project` (`app_state.rs:617-642`) creates a session with
   defaults but does not sync `state.show_explorer`, `state.handoff_enabled`,
   `state.show_reasoning_inline`, `state.show_project_tasks` to the new
   session's values. The next `AutocodeApp::save` (`app.rs:478-484`) writes the
   stale global flags into the new session's `session.json`, clobbering its
   defaults.

### P1 — Stale state survives restart

3. **Orphaned-session startup leaves 4 of 7 UI flags stale.** `AppState::load`
   (`app_state.rs:401-405`) clears only `show_todo`, `todo_user_dismissed`,
   `settings_open` when `active_session_id` is orphaned. `handoff_enabled`,
   `show_explorer`, `show_reasoning_inline`, `show_project_tasks` retain stale
   app.ron values until a session is explicitly loaded. These are live in the
   UI and will be written into the next session's `session.json`.

4. **`active_provider` in app.ron can point at an unrelated provider after
   restart.** When `active_session_id` is orphaned at startup,
   `restore_active_session` returns early (`app.rs:120`) and the stale
   `active_provider` from app.ron is kept (only validated for existence,
   `app_state.rs:385`). It may have no relationship to the session the user next
   opens.

### P2 — Inconsistent / missing persistence

5. **Thinking-mode toggle does not set `session_meta_dirty`.** `input.rs:222`
   mutates `sess.thinking_mode` but does not flag the session meta dirty, so the
   change is only persisted on the next 10s auto-save / exit. A crash loses the
   toggle. (The reasoning-effort picker at `input.rs:293` has the same omission;
   the toolbar pickers at `pickers.rs:130,192` do set it.)

6. **`thinking_mode` / `reasoning_effort` are duplicated on `ApiProvider` and
   `Session` with no sync path.** The session wins at request time
   (`completion/mod.rs:137-175`); the provider's copies are set only by model
   selection and never propagated to existing sessions. Editing provider-level
   thinking settings has no effect on existing sessions.

7. **`on_exit` does not save per-session UI flags or sampling params.**
   `on_exit` (`app.rs:504-530`) copies only `provider_label` and `model` into
   the active session, then calls `save_sessions()`. It does NOT copy the seven
   UI flags or the sampling params (contrast with `save` at `app.rs:478-493`).
   So if the user changes a UI flag and quits immediately, the change is lost
   unless the 10s auto-save already ran. (The auto-save usually covers this, but
   it's an inconsistency between the two save paths.)

### P3 — Minor / cosmetic

8. **`prune_disk_state` orphaned-session branch is inconsistent with the
   empty-sessions branch.** When `active_session_id` is orphaned but sessions
   remain (`app_state.rs:483-490`), it clears `active_session_id` but leaves all
   seven UI flags. When `sessions.is_empty()` (`app_state.rs:478-482`), it clears
   `active_session_id`, `show_todo`, `todo_user_dismissed`, `handoff_enabled`
   — but still not `show_explorer`, `show_reasoning_inline`, `show_project_tasks`.
   The two branches should clear the same set of session-level flags.

9. **`explorer_width` is global but `show_explorer` is per-session.** The
   explorer panel width (`explorer_width`, app.ron) is shared across all
   sessions, while its visibility (`show_explorer`) is per-session. Resizing the
   explorer in one session affects all sessions. Likely intentional but worth
   noting as a UX inconsistency.

---

## 6. Root cause

The fundamental design flaw is that **per-session state is stored as global
working-copy fields on `AppState` and serialized to app.ron**, instead of being
read from the active `Session` on demand. app.ron then becomes a stale cache of
"whichever session was active at last save," and every code path that activates
a session must remember to copy disk → global (and every save must copy global →
session → disk). The multiple copy sites are inconsistent:

- Startup restore copies sampling params session→provider (`app.rs:180-185`),
  but session switch does not (`session.rs:139-147`).
- Startup restore clears 3 of 7 UI flags on orphan (`app_state.rs:401-405`);
  `prune_disk_state` clears a different 4 of 7 on empty (`app_state.rs:478-482`);
  new-session creation clears none.
- `on_exit` copies only provider label/model; `save` copies everything.

The fix direction (not implemented here — audit only): make the `Session` struct
the single source of truth for all per-session state, read per-session fields
directly from `active_session()` at use sites, and stop persisting the seven UI
flags and the provider sampling params to app.ron. app.ron should hold only
genuinely global state (timeouts, system prompt, explorer width, active ids).
