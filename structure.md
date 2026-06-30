# AutoCode — Project File Structure

## Root

```
.gitignore         |  40
AGENTS.md           |   7
Cargo.lock          |  —
Cargo.toml          |  24  Workspace manifest (5 crate members)
LRU.md              | 654
README.md           | 128
THINKING.md         |1011
audit.md            | 295
structure.md        | 150
ultimate_egui.md    |2588
```

## `.cargo/`

```
.cargo/config.toml  |  12
```

## `.github/workflows/`

```
.github/workflows/ci.yml     | 69
.github/workflows/release.yml | 71
```

## `assets/`

```
assets/icon.icns              |  —
assets/icon.ico               |  —
assets/providers.json         | 179
assets/screenshot.png         |  —
assets/linux/icon-16.png      |  —
assets/linux/icon-32.png      |  —
assets/linux/icon-48.png      |  —
assets/linux/icon-64.png      |  —
assets/linux/icon-128.png     |  —
assets/linux/icon-256.png     |  —
assets/linux/icon-512.png     |  —
```

## `crates/ai/` — AI Provider & Chat Orchestration (33 files, 7,074 lines)

```
crates/ai/Cargo.toml                          |  11
crates/ai/src/lib.rs                           |  11
crates/ai/src/chat/mod.rs                      |  24
crates/ai/src/chat/errors.rs                   | 220
crates/ai/src/chat/runtime.rs                  | 203
crates/ai/src/chat/session.rs                  | 166
crates/ai/src/chat/session_ops.rs              | 346
crates/ai/src/chat/completion/mod.rs           | 516
crates/ai/src/chat/completion/preflight.rs     | 132
crates/ai/src/chat/completion/provider.rs      | 107
crates/ai/src/chat/polling/mod.rs              | 116
crates/ai/src/chat/polling/shell.rs            | 288
crates/ai/src/chat/polling/stream.rs           | 677
crates/ai/src/chat/polling/tools.rs            | 117
crates/ai/src/chat/tools/mod.rs                |   6
crates/ai/src/chat/tools/execute.rs            | 842
crates/ai/src/chat/tools/meta.rs               | 324
crates/ai/src/chat/tools/process.rs            |  37
crates/ai/src/helpers/mod.rs                   |  17
crates/ai/src/helpers/fuzzy.rs                 | 686
crates/ai/src/helpers/misc.rs                  |  83
crates/ai/src/helpers/strip_lines.rs           |  60
crates/ai/src/helpers/task_detect.rs           |  18
crates/ai/src/helpers/todo_parse.rs            |  41
crates/ai/src/helpers/tool_error.rs            |  14
crates/ai/src/provider/mod.rs                  |  17
crates/ai/src/provider/client.rs               | 397
crates/ai/src/provider/http.rs                 | 868
crates/ai/src/provider/rate_limit.rs           |  61
crates/ai/src/provider/thread_pool.rs          |  67
crates/ai/src/provider/tool_defs.rs            |  43
crates/ai/src/provider/types.rs                |  95
crates/ai/src/provider/web.rs                  | 464
```

## `crates/autocode/` — Binary Entry Point (4 files, 23 lines)

```
crates/autocode/Cargo.toml                     |  11
crates/autocode/build.rs                       |   4
crates/autocode/resources/app.rc               |   1
crates/autocode/src/main.rs                    |   7
```

## `crates/core/` — Shared Types, State, Storage, Utils (36 files, 5,949 lines)

```
crates/core/Cargo.toml                          |   8
crates/core/src/lib.rs                          |  14
crates/core/src/debug.rs                        |  14
crates/core/src/helpers/mod.rs                  |  37
crates/core/src/helpers/id.rs                   |  35
crates/core/src/helpers/levenshtein.rs          |  35
crates/core/src/helpers/paths.rs                | 258
crates/core/src/helpers/regex.rs                | 366
crates/core/src/helpers/sanitize.rs             |  56
crates/core/src/helpers/serde_defaults.rs       |  81
crates/core/src/helpers/tokens.rs               | 258
crates/core/src/helpers/utils.rs               | 307
crates/core/src/state/mod.rs                    |  19
crates/core/src/state/app_state.rs             | 559
crates/core/src/state/chat.rs                   |  91
crates/core/src/state/manifest.rs               |  52
crates/core/src/state/project.rs                |  10
crates/core/src/state/provider.rs               | 411
crates/core/src/state/secret.rs                 |  55
crates/core/src/state/session.rs               | 255
crates/core/src/state/todo.rs                   | 115
crates/core/src/storage/mod.rs                  |  33
crates/core/src/storage/app_storage.rs           |  12
crates/core/src/storage/chunked_jsonl.rs         | 287
crates/core/src/storage/discovery.rs            | 209
crates/core/src/storage/persistence.rs          | 147
crates/core/src/storage/provider_file.rs        | 237
crates/core/src/storage/session_io.rs           | 361
crates/core/src/storage/session_meta.rs         | 104
crates/core/src/storage/shell_task.rs            |  82
crates/core/src/tokenizer/mod.rs                 |  15
crates/core/src/utils/mod.rs                     |  15
crates/core/src/utils/extract.rs                 | 298
crates/core/src/utils/fsutil.rs                  | 148
crates/core/src/utils/sysinfo.rs                 | 689
crates/core/tests/stability.rs                   | 276
```

## `crates/fs/` — Filesystem, Shell, Git, Skills (17 files, 1,719 lines)

```
crates/fs/Cargo.toml                            |   6
crates/fs/src/lib.rs                            |  10
crates/fs/src/git.rs                            | 175
crates/fs/src/shell.rs                          | 200
crates/fs/src/skills.rs                         | 153
crates/fs/src/explorer/mod.rs                   |  15
crates/fs/src/explorer/fuzzy.rs                 | 341
crates/fs/src/explorer/gitignore.rs             |  73
crates/fs/src/explorer/glob.rs                   |  54
crates/fs/src/explorer/grep.rs                    | 203
crates/fs/src/explorer/listing.rs               | 149
crates/fs/src/explorer/read_file.rs             |  14
crates/fs/src/explorer/tree.rs                   |  89
crates/fs/src/helpers/mod.rs                     |   8
crates/fs/src/helpers/extract.rs                 | 133
crates/fs/src/helpers/glob_match.rs              |  72
crates/fs/src/helpers/levenshtein.rs             |  24
```

## `crates/ui/` — Desktop UI (egui/eframe) (46 files, 7,310 lines)

```
crates/ui/Cargo.toml                            |  14
crates/ui/src/lib.rs                            |  58
crates/ui/src/app.rs                            | 494
crates/ui/src/theme.rs                          | 120
crates/ui/src/chat/mod.rs                       |  17
crates/ui/src/chat/code_block.rs                | 177
crates/ui/src/chat/diff_view.rs                 | 233
crates/ui/src/chat/input.rs                     | 331
crates/ui/src/chat/markdown.rs                  | 192
crates/ui/src/chat/messages.rs                  | 108
crates/ui/src/chat/panel.rs                     | 321
crates/ui/src/chat/session.rs                   | 217
crates/ui/src/chat/state.rs                     |  62
crates/ui/src/chat/tabs.rs                      | 199
crates/ui/src/chat/theme.rs                     |  76
crates/ui/src/chat/tool_result.rs               | 507
crates/ui/src/explorer/mod.rs                   |   8
crates/ui/src/explorer/panel.rs                 | 129
crates/ui/src/explorer/state.rs                 |  52
crates/ui/src/explorer/tree.rs                  | 292
crates/ui/src/explorer/viewer.rs                | 496
crates/ui/src/helpers/mod.rs                    |  21
crates/ui/src/helpers/diff.rs                   | 108
crates/ui/src/helpers/formatting.rs             | 250
crates/ui/src/helpers/time.rs                   |  10
crates/ui/src/helpers/todo.rs                   |  10
crates/ui/src/helpers/tool_result.rs            | 150
crates/ui/src/helpers/ui_id.rs                  | 143
crates/ui/src/helpers/widgets.rs                |  45
crates/ui/src/settings/mod.rs                   |  10
crates/ui/src/settings/about.rs                 | 187
crates/ui/src/settings/projects.rs              | 163
crates/ui/src/settings/prompt.rs                |  92
crates/ui/src/settings/providers.rs             | 732
crates/ui/src/settings/session.rs               |  83
crates/ui/src/settings/state.rs                 |  42
crates/ui/src/settings/timeouts.rs              | 147
crates/ui/src/settings/window.rs                | 216
crates/ui/src/tasks/mod.rs                      |   4
crates/ui/src/tasks/task_list.rs                |  87
crates/ui/src/tasks/task_window.rs              | 322
crates/ui/src/toolbar/mod.rs                    |   5
crates/ui/src/toolbar/buttons.rs                |  33
crates/ui/src/toolbar/layout.rs                 |  70
crates/ui/src/toolbar/meters.rs                 |  95
crates/ui/src/toolbar/pickers.rs                | 182
```

## `skills/` — Agent Skill Library (76 files)

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
skills/component_design.md                      | 205
skills/concurrency_patterns.md                  | 197
skills/css_architecture.md                      | 246
skills/css_layout.md                            | 254
skills/css_styling.md                           | 302
skills/data_migration.md                        | 207
skills/data_modeling.md                         | 191
skills/database_patterns.md                     | 156
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
skills/memory_management.md                     | 191
skills/monorepo_management.md                   | 169
skills/networking_fundamentals.md               | 183
skills/performance_profiling.md                 | 166
skills/plugin_and_extension_systems.md          | 197
skills/prompt_engineering.md                    | 161
skills/python_patterns.md                       | 315
skills/react_patterns.md                        | 214
skills/regex_patterns.md                        | 185
skills/responsive_images_and_media.md           | 206
skills/rest_api_design.md                       | 201
skills/rust_guru.md                             | 862
skills/search_and_filtering.md                  | 176
skills/security_basics.md                       | 177
skills/shell_usage.md                           | 135
skills/sql_advanced.md                          | 277
skills/state_machine_design.md                  | 197
skills/system_design.md                         | 135
skills/task_decomposition.md                    | 130
skills/testing_strategies.md                    | 154
skills/typescript_patterns.md                   | 202
skills/ui_design_fundamentals.md                | 151
skills/ux_principles.md                         | 126
skills/web_animation.md                         | 287
skills/web_research.md                          | 129
skills/webscraping.md                           | 181
skills/websocket_and_realtime.md                | 196
skills/writing_tests.md                         | 184
skills/yang_mills_mass_gap.md                   | 140
```

---

## Summary

| Area | Files | Lines | Role |
|------|-------|-------|------|
| `crates/ai/` | 33 | 7,074 | AI provider clients, chat orchestration, tool execution, HTTP/SSE, web scraping |
| `crates/autocode/` | 4 | 23 | Windows binary entry point, icon embedding |
| `crates/core/` | 36 | 5,949 | State types, persistence, helpers, tokenizer, sysinfo, HTML extraction |
| `crates/fs/` | 17 | 1,719 | File explorer, shell executor, git status, skill loader |
| `crates/ui/` | 46 | 7,310 | egui panels — chat, settings, explorer, toolbar, todo windows |
| **Crate subtotal** | **136** | **22,075** | |
| `skills/` | 77 | ~15,840 | Skill markdown files bundled with the binary |
| `assets/` | 11 | — | Icons, screenshot, bundled providers.json |
| Root + config | 13 | — | Workspace manifest, CI/CD, .gitignore, documentation |
| **Grand total** | **237** | **~37,940** | |
