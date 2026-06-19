# AutoCode Roadmap

## High Priority

- [ ] **`ChatRuntime` leak** — Old runtimes never removed from `runtimes` HashMap → unbounded growth. Add a prune function that removes runtimes for sessions no longer in `AppState.sessions`.
- [ ] **Shell command injection** — `run_shell` passes raw strings to `cmd /C` / `sh -c` with no sanitization. Add a `sanitize_shell()` function that rejects `;`, `&&`, `||`, `|`, `>`, `<` when not explicitly needed.
- [ ] **Silent disk write failures** — Many `let _ =` patterns swallow I/O errors. Surface failures via the UI or logging instead of silently dropping them.
- [ ] **Panic swallowing** — `catch_unwind` in `ThreadPool::Worker` and `PersistenceThread` hides crashes. Report panics back to the user or at minimum log them visibly.

## Medium Priority

- [ ] **`catch_unwind` in PersistenceThread** — Panics in the background persistence thread are caught and discarded. Forward errors so the app can react (retry, notify user, etc.).
- [ ] **No retry for `flush_pending_writes`** — If the background thread dies, pending message writes are lost. Add a health-check or watchdog to detect and restart the thread.
- [ ] **Provider error surfacing** — `provider_error` string is set but not always displayed in the UI when a provider call fails. Ensure errors are consistently shown in the chat panel.

## Low Priority

- [ ] **Image texture cache** — Images accumulate unbounded while the file viewer is open. Evict old or unviewed textures after a TTL.
- [ ] **Thread pool queue unbounded** — The `mpsc` channel for shell execution has unbounded capacity. Switch to a bounded channel with backpressure or a max queue size.
- [ ] **Clone-heavy patterns** — Many `.clone()` calls on `ChatMessage`, `String`, and `Vec` add up with large message histories. Profile and add targeted borrow/shared-ptr optimizations.
- [ ] **Deep nesting in UI code** — Multiple levels of `Frame::NONE.show(ui, |ui| { ... })` in egui code make the UI logic harder to read. Consider extracting deeply nested blocks into named functions.