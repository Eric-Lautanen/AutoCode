# AutoCode — Project File Structure

## Root

```
.gitignore         |  47
AGENTS.md           |   9
Cargo.lock          |  —
Cargo.toml          |  24  Workspace manifest (5 crate members)
README.md           | 172
structure.md        |  —  (this file)
ultimate_egui.md    |2893  egui/eframe reference notes
```

## `.cargo/`

```
.cargo/config.toml  |  14
```

## `.github/workflows/`

```
.github/workflows/ci.yml     | 75
.github/workflows/release.yml | 76
```

## `assets/`

```
assets/icon.icns              |  —
assets/icon.ico               |  —
assets/providers.json         | 190  Bundled provider configs (seeded on first launch)
assets/screenshot.png         |  —
assets/linux/icon-16.png      |  —
assets/linux/icon-32.png      |  —
assets/linux/icon-48.png      |  —
assets/linux/icon-64.png      |  —
assets/linux/icon-128.png     |  —
assets/linux/icon-256.png     |  —
assets/linux/icon-512.png     |  —
```

## `crates/ai/` — AI Provider & Chat Orchestration (35 files, 11,371 lines)

```
crates/ai/Cargo.toml                          |  12
crates/ai/src/lib.rs                           |  12
crates/ai/src/chat/mod.rs                      |  27
crates/ai/src/chat/errors.rs                   | 230
crates/ai/src/chat/looping.rs                  | 311  LRU pruning: scoring, pair grouping, breadcrumbs
crates/ai/src/chat/runtime.rs                  | 270
crates/ai/src/chat/session.rs                  | 175
crates/ai/src/chat/session_ops.rs              | 460
crates/ai/src/chat/completion/mod.rs           | 614
crates/ai/src/chat/completion/preflight.rs     | 143
crates/ai/src/chat/completion/provider.rs      | 114
crates/ai/src/chat/polling/mod.rs              | 136
crates/ai/src/chat/polling/shell.rs            | 316
crates/ai/src/chat/polling/stream.rs           | 923
crates/ai/src/chat/polling/tools.rs            | 127
crates/ai/src/chat/tools/mod.rs                |   8
crates/ai/src/chat/tools/execute.rs            |1307  24-tool dispatcher + handlers
crates/ai/src/chat/tools/meta.rs               | 408
crates/ai/src/chat/tools/proof.rs              |1766  Yang-Mills proof checker (verifier discovery, exec, output parsing, structural checks, JSONL attempt log)
crates/ai/src/chat/tools/process.rs            |  38
crates/ai/src/helpers/mod.rs                   |  18
crates/ai/src/helpers/fuzzy.rs                 | 761
crates/ai/src/helpers/misc.rs                  |  88
crates/ai/src/helpers/strip_lines.rs           |  61
crates/ai/src/helpers/task_detect.rs           |  18
crates/ai/src/helpers/todo_parse.rs            |  46
crates/ai/src/helpers/tool_error.rs            |  14
crates/ai/src/provider/mod.rs                  |  19
crates/ai/src/provider/client.rs               | 435
crates/ai/src/provider/http.rs                 | 937
crates/ai/src/provider/rate_limit.rs           |  68
crates/ai/src/provider/thread_pool.rs          |  80
crates/ai/src/provider/tool_defs.rs            |  44
crates/ai/src/provider/types.rs                | 104
crates/ai/src/provider/web.rs                  |1281  HTTP/network layer: native_get/post, headless-Chrome (CDP) SPA renderer (used by the fetch_url/web_search handlers in chat/tools/execute.rs)
```

## `crates/autocode/` — Binary Entry Point (4 files, 27 code lines)

```
crates/autocode/Cargo.toml                     |  14
crates/autocode/build.rs                       |   4
crates/autocode/resources/app.rc               |   1   Windows resource (icon) manifest
crates/autocode/src/main.rs                    |   9
```

## `crates/core/` — Shared Types, State, Storage, Utils (36 files, 7,983 lines)

```
crates/core/Cargo.toml                          |   8
crates/core/src/lib.rs                          |  15
crates/core/src/helpers/mod.rs                   |  46
crates/core/src/helpers/id.rs                    |  40
crates/core/src/helpers/paths.rs                 | 282
crates/core/src/helpers/regex.rs                | 382
crates/core/src/helpers/sanitize.rs             |  56
crates/core/src/helpers/serde_defaults.rs       |  92
crates/core/src/helpers/tokens.rs               | 284
crates/core/src/helpers/utils.rs                | 404
crates/core/src/state/mod.rs                    |  23
crates/core/src/state/access_log.rs             |  73  FileAccessLog — turn-window working-set tracker (no eviction)
crates/core/src/state/app_state.rs             | 816
crates/core/src/state/chat.rs                   | 115
crates/core/src/state/manifest.rs               |  55
crates/core/src/state/project.rs                |  11
crates/core/src/state/provider.rs               | 495
crates/core/src/state/secret.rs                 |  66
crates/core/src/state/session.rs               | 307
crates/core/src/state/todo.rs                   | 102
crates/core/src/storage/mod.rs                  |  31
crates/core/src/storage/app_storage.rs           |  14
crates/core/src/storage/messages.rs             | 294
crates/core/src/storage/discovery.rs             | 242
crates/core/src/storage/persistence.rs          | 163
crates/core/src/storage/provider_file.rs        | 271
crates/core/src/storage/session_io.rs           | 457
crates/core/src/storage/session_meta.rs         | 114
crates/core/src/storage/shell_task.rs            |  88
crates/core/src/tokenizer/mod.rs                 |  19
crates/core/src/utils/mod.rs                     |  20
crates/core/src/utils/extract.rs                 | 477
crates/core/src/utils/fsutil.rs                  | 160
crates/core/src/utils/html.rs                    | 762  HTML cleaner / text extraction
crates/core/src/utils/sysinfo.rs                 | 878
crates/core/tests/stability.rs                   | 321
```

## `crates/fs/` — Filesystem, Shell, Git, Skills (18 files, 2,680 lines)

```
crates/fs/Cargo.toml                            |   7
crates/fs/src/lib.rs                            |  11
crates/fs/src/git.rs                            | 201
crates/fs/src/shell.rs                          | 211
crates/fs/src/skills.rs                         | 167
crates/fs/src/explorer/mod.rs                   |  18
crates/fs/src/explorer/comment.rs               | 321
crates/fs/src/explorer/fuzzy.rs                 | 831
crates/fs/src/explorer/gitignore.rs             |  79
crates/fs/src/explorer/glob.rs                   |  59
crates/fs/src/explorer/grep.rs                    | 234
crates/fs/src/explorer/listing.rs               | 168
crates/fs/src/explorer/read_file.rs             |  17
crates/fs/src/explorer/tree.rs                   |  97
crates/fs/src/helpers/mod.rs                     |  10
crates/fs/src/helpers/extract.rs                 | 139
crates/fs/src/helpers/glob_match.rs              |  74
crates/fs/src/helpers/levenshtein.rs             |  25
```

## `crates/ui/` — Desktop UI (egui/eframe) (46 files, 8,125 lines)

```
crates/ui/Cargo.toml                            |  15
crates/ui/src/lib.rs                            |  64
crates/ui/src/app.rs                            | 567
crates/ui/src/theme.rs                          | 151
crates/ui/src/chat/mod.rs                       |  19
crates/ui/src/chat/code_block.rs                | 186
crates/ui/src/chat/diff_view.rs                 | 250
crates/ui/src/chat/input.rs                     | 392
crates/ui/src/chat/markdown.rs                  | 205
crates/ui/src/chat/messages.rs                  | 118
crates/ui/src/chat/panel.rs                     | 338
crates/ui/src/chat/session.rs                   | 249
crates/ui/src/chat/state.rs                     |  77
crates/ui/src/chat/tabs.rs                      | 200
crates/ui/src/chat/theme.rs                     |  81
crates/ui/src/chat/tool_result.rs               | 535
crates/ui/src/explorer/mod.rs                   |  11
crates/ui/src/explorer/panel.rs                 | 139
crates/ui/src/explorer/state.rs                 |  57
crates/ui/src/explorer/tree.rs                  | 308
crates/ui/src/explorer/viewer.rs                | 534
crates/ui/src/helpers/mod.rs                     |  23
crates/ui/src/helpers/diff.rs                   | 113
crates/ui/src/helpers/formatting.rs             | 264
crates/ui/src/helpers/time.rs                   |  11
crates/ui/src/helpers/todo.rs                   |  12
crates/ui/src/helpers/tool_result.rs            | 157
crates/ui/src/helpers/ui_id.rs                   | 172
crates/ui/src/helpers/widgets.rs                |  51
crates/ui/src/settings/mod.rs                   |  11
crates/ui/src/settings/about.rs                 | 223
crates/ui/src/settings/projects.rs              | 209
crates/ui/src/settings/prompt.rs                | 140
crates/ui/src/settings/providers.rs             | 790
crates/ui/src/settings/session.rs               |  90
crates/ui/src/settings/state.rs                 |  47
crates/ui/src/settings/timeouts.rs              | 165
crates/ui/src/settings/window.rs                | 230
crates/ui/src/tasks/mod.rs                       |   5
crates/ui/src/tasks/task_list.rs                |  82
crates/ui/src/tasks/task_window.rs              | 347
crates/ui/src/toolbar/mod.rs                     |   6
crates/ui/src/toolbar/buttons.rs                |  45
crates/ui/src/toolbar/layout.rs                 |  97
crates/ui/src/toolbar/meters.rs                 | 141
crates/ui/src/toolbar/pickers.rs                | 198
```

## `skills/` — Agent Skill Library (77 files, 20,157 lines)

```
skills/accessibility.md                         | 145
skills/api_integration.md                       | 163
skills/authentication_and_authorization.md      | 159
skills/background_jobs_and_queues.md            | 163
skills/bash_scripting.md                        | 252
skills/browser_performance.md                   | 221
skills/caching_strategies.md                    | 201
skills/ci_cd_pipelines.md                       | 219
skills/cli_tool_design.md                       | 192
skills/code_generation.md                       | 153
skills/code_migration.md                        | 164
skills/code_refactoring.md                      | 120
skills/code_review_checklist.md                 |  97
skills/codebase_orientation.md                  |  98
skills/component_design.md                       | 205
skills/concurrency_patterns.md                  | 197
skills/css_architecture.md                      | 246
skills/css_layout.md                            | 254
skills/css_styling.md                           | 302
skills/data_migration.md                        | 207
skills/data_modeling.md                         | 191
skills/database_patterns.md                      | 156
skills/date_and_time_handling.md                | 109
skills/debugging_workflow.md                    | 126
skills/dependency_injection.md                  | 189
skills/dependency_management.md                 | 101
skills/design_tokens.md                         | 207
skills/docker_and_containers.md                 | 166
skills/documentation_writing.md                 | 157
skills/egui_guru.md                             | 662
skills/encryption_and_hashing.md                | 230
skills/environment_and_config.md                | 172
skills/error_handling_design.md                 | 199
skills/event_driven_architecture.md             | 206
skills/file_editing_strategy.md                 | 142
skills/file_format_handling.md                  | 269
skills/filesystem_operations.md                 | 186
skills/frontend_basics.md                       | 214
skills/git_workflows.md                         | 194
skills/go_patterns.md                           | 307
skills/html_structure.md                        | 277
skills/infrastructure_as_code.md                | 258
skills/javascript_dom.md                        | 330
skills/json_and_data_serialization.md           | 218
skills/language_specific_conventions.md         | 213
skills/library_and_package_design.md            | 241
skills/logging_and_observability.md             | 180
skills/long_running_task_management.md          | 193
skills/memory_management.md                      | 191
skills/monorepo_management.md                    | 169
skills/networking_fundamentals.md               | 183
skills/performance_profiling.md                 | 166
skills/plugin_and_extension_systems.md          | 197
skills/prompt_engineering.md                     | 161
skills/python_patterns.md                        | 315
skills/react_patterns.md                         | 214
skills/regex_patterns.md                         | 185
skills/responsive_images_and_media.md           | 206
skills/rest_api_design.md                        | 201
skills/rust_guru.md                             | 862
skills/search_and_filtering.md                   | 176
skills/security_basics.md                        | 177
skills/shell_usage.md                            | 135
skills/sql_advanced.md                           | 277
skills/state_machine_design.md                   | 197
skills/system_design.md                          | 135
skills/task_decomposition.md                     | 130
skills/testing_strategies.md                     | 154
skills/typescript_patterns.md                    | 202
skills/ui_design_fundamentals.md                 | 151
skills/ux_principles.md                          | 126
skills/web_animation.md                          | 287
skills/web_research.md                           | 129
skills/webscraping.md                            | 181
skills/websocket_and_realtime.md                 | 196
skills/writing_tests.md                          | 184
skills/yang_mills_mass_gap.md                    |  46
```

---

## Summary

| Area | Files | Lines | Role |
|------|-------|-------|------|
| `crates/ai/` | 35 | 11,371 | AI provider clients, chat orchestration, tool execution, HTTP/SSE, web scraping, LRU looping |
| `crates/autocode/` | 4 | 27 | Windows binary entry point, icon embedding |
| `crates/core/` | 36 | 7,983 | State types, persistence, helpers, tokenizer, sysinfo, HTML extraction, FileAccessLog |
| `crates/fs/` | 18 | 2,680 | File explorer, shell executor, git status, skill loader |
| `crates/ui/` | 46 | 8,125 | egui panels — chat, settings, explorer, toolbar, todo windows |
| **Crate subtotal** | **139** | **30,186** | |
| `skills/` | 77 | 20,157 | Skill markdown files bundled with the binary |
| `assets/` | 11 | — | Icons, screenshot, bundled providers.json |
| Root + config | 13 | — | Workspace manifest, CI/CD, .gitignore, documentation |
| **Grand total** | **240** | **~50,343** | |
