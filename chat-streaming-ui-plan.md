# Chat Streaming UI — Smoothness Plan (v2, audited)

Status: **Proposal / discussion document — no code changes made**
Scope: every provider-model output (text deltas, reasoning, tool-call args, tool execution, shell output, file writes, diffs) renders as one continuous, framed, non-jerky stream. Bottom-sticky scroll that holds while the user is at the bottom and breaks the instant they scroll or arrow up.

---

## 0. Locked decisions (answers to prior open questions)

| Question | Decision |
|---|---|
| Assistant bubble vs no frame | **Everything is framed** — reasoning, code blocks, diffs, assistant text, tool cards, shell. A single, subtle frame language; simple and clean. |
| What gets streamed | **Everything the model sends, as it arrives**: text, thinking, tool-call argument JSON, tool execution state, shell output, file-write previews, diffs. Nothing pops in. |
| Pacing aggressiveness | Smooth reveal with a generous per-frame budget (see §3.3); fast-forwards on completion. Not perceptibly slower than raw chunks. |
| Insert animation | On by default, very restrained (fade + tiny translate, 120–160 ms). No marquees, no bounces. |
| Sticky scroll | Hold sticky while the thumb is at the bottom. **Break the instant the user scrolls or arrows up.** Re-engage only when they scroll back to the bottom. |
| Persistence / architecture | Untouched. New state lives on `ChatRuntime` (transient, UI-facing) and `ChatPanelState` (UI). Nothing new is persisted. |

---

## 1. Current design review (audited)

### 1.1 The full pipeline

```
Provider SSE socket
   └─ http.rs parse_sse_stream_from_reader()
        ├─ ThinkTagFilter     splits <think>...</think> out of text deltas
        ├─ tool_acc           accumulates tool-call arg chunks per index (http.rs:487, 598-614)
        └─ emits ProviderEvent over mpsc
             ├─ Delta(String)          → runtime.pending_response   (stream.rs:32)
             ├─ Reasoning(String)      → runtime.reasoning_buf      (stream.rs:43)
             ├─ ToolCall(ToolCall)     → runtime.pending_tool_calls (stream.rs:50) * only after finish_reason=="tool_calls"
             ├─ Done{...} / Error(String)

UI frame loop (app.rs logic)
   └─ update_all → update_runtime → poll_stream (try_recv drain, ≤256 events/frame)
        ├─ Done → text path:  ChatMessage(Assistant, pending_response) → sess.messages (push_runtime)
        │        → tool path: bg-thread exec → ToolResult(s) → sess.messages (push_tool_results_to_state)
        └─ repaint → app.rs:304-334 (16 ms while any busy)

Chat panel (panel.rs) renders TWO divergent paths each frame:
   1. Committed history — display_buffer of sess.messages
        ├─ User      → right-aligned bubble      (messages.rs:30)
        ├─ Assistant → plain markdown, NO frame  (messages.rs:80)
        ├─ Tool      → collapsible card/diff/terminal (tool_result.rs)
        └─ Error     → red label
   2. Live preview (hand-rolled blob at bottom, non-frame)
        ├─ pending_response → separator + render_markdown + "|" caret (panel.rs:221-226)
        ├─ live_shell_buf   → render_shell_terminal                  (panel.rs:227-232)
        ├─ live_write_progress → "[File] Writing..." + code block    (panel.rs:233-242)
        └─ reasoning_buf    → "Thinking..." label                    (panel.rs:243-251)
```

### 1.2 Root causes of "bubbles but not smooth" (audited)

1. **Two divergent render paths.** While streaming, content is a raw `String` on the runtime rendered by a bespoke blob; on `Done` it becomes a `ChatMessage` rendered by a different path (no frame, different position). The swap is a visible jump. (panel.rs:221 vs messages.rs:80)

2. **Whole-buffer re-render every frame.** `render_markdown` re-parses and re-lays-out the *entire* accumulated response each frame (markdown.rs:11). Per-frame cost grows linearly with reply length → uneven frame pacing = the "jerky" feel. No galley/segment caching. Code blocks re-split every frame and pop in only at the closing ` ``` `; `render_code_block_impl`'s `_streaming` and `_inst` params are passed but **ignored** (code_block.rs:18). The `streaming` flag in `render_markdown` is effectively unused today (verified: it only reaches ignored code_block params), so repurposing it is safe.

3. **Tool calls are not streamed.** `http.rs` accumulates arg deltas and only emits `ToolCall` at `finish_reason=="tool_calls"` (http.rs:630-649). Nothing renders while the model writes function-call JSON; the card then appears suddenly.

4. **Tool execution has a blank gap.** Non-shell tools run on a background thread (`poll_tool_results`, tools.rs:7). Web/network tools take seconds with only a status line (tools.rs:66-69). No in-progress card / spinner / timer.

5. **Bursty arrival.** SSE chunks land as few large events; the whole chunk is dumped into the buffer in one frame. No display-side pacing.

6. **Snap scrolling with a dead-zone.** `stick_to_bottom(true)` + manual offset math with a **20 px threshold** (panel.rs:124, 261). A small upward scroll (<20 px) does not break stickiness — contradicts the "break the moment" requirement.

7. **No insert animation** — messages appear instantly. Live reasoning box (bottom) vs committed reasoning box inside the assistant message (messages.rs:88-101) is another teleport.

8. **Inconsistent framing.** User = bubble, assistant = none, tool = card, live = raw blob. Only 1 of 4 roles is framed.

---

## 2. Design goals

1. **One render path** — committed and in-flight rows use the same renderers; the live row is just the last row. Committing is a zero-jump state change.
2. **Everything streams, framed** — reasoning, code, diffs, tool calls, tool state, shell, file writes all appear as they happen, inside a consistent subtle frame.
3. **Steady pacing** — display-side reveal smooths network bursts to a steady flow; 60 fps cadence while a live turn is active.
4. **Predictable scroll** — hold bottom while at bottom; break immediately on scroll/arrow up; never yank the view while reading.
5. **Animated but calm** — restrained fade/translate inserts, blinking caret, spinner/timer on live tool cards. Nothing flashy.
6. **Zero breaking changes** — persistence, session model, provider commit behavior, and existing scrolling intent are preserved. All additions are additive (`ProviderEvent` variant, transient runtime/panel fields).

---

## 3. Proposed architecture

### 3.1 Unified message view with a live slot

Replace the `display_buffer`-only model with a view over committed rows + one live row:

```
MessageView {
  committed: Vec<MessageRow>,     // stable rows (replaces display_buffer), each with anim_start
  live: Option<LiveTurn>,         // in-flight assistant turn rendered as the last row
}
```

`LiveTurn` is derived each frame from the active runtime's transient buffers — **no new event plumbing for text/reasoning/shell/write** (they already exist on `ChatRuntime`):

```
enum LiveTurn {
  Thinking{ text, committed_prefix },        // reasoning streaming (framed box)
  Responding{ commit_buf, reveal_len },      // text deltas streaming (framed bubble)
  CallingTool{ index, name, args_so_far },   // tool-call JSON streaming (card)
  ExecutingTool{ name, args, elapsed },      // tool running on bg thread (card + spinner/timer)
  WritingFile{ path, content },              // write_file live preview (framed code block)
  ShellRunning{ buf, pid },                  // live shell output (framed terminal)
}
```

Key property: **the live row sits at a stable index in the same scroll content as committed rows**, rendered by the same helpers. When the turn completes it transitions in place: the committed row appears via the normal Phase-2 append (panel.rs:79-100), the live row clears, and a 120–160 ms fast-forward/insert anim covers the swap so there is no blank flicker.

### 3.2 New plumbing (strictly additive)

1. **Partial tool-call events.** In `http.rs`, inside the existing `delta["tool_calls"]` accumulation loop (http.rs:598), after updating `tool_acc[index]`, also emit:
   ```
   ProviderEvent::ToolCallDelta { index: usize, name: String, arguments: String }
   ```
   (id rarely streams; keep name+args-so-far only). The existing full `ToolCall` at `finish_reason=="tool_calls"` is unchanged and still drives execution — the delta is display-only.
2. `polling/stream.rs` forwards `ToolCallDelta` into a new transient field `runtime.live_tool_call: Option<(String, String)>` (name, args-so-far). Cleared in `drain()`, in `start_completion`, and when the tool batch is dispatched (stream.rs:342).
3. **Tool lifecycle for the UI.** No new channel — derive it. When `runtime.tool_rx` or `runtime.live_shell_rx` is `Some` and the last received `ToolCall` batch is known, the UI shows `ExecutingTool`. `elapsed` is computed from `request_start`-independent timestamps: add `runtime.tool_batch_start: Option<Instant>` set in stream.rs:617 (bg-thread dispatch) and shell.rs `start_next_live_shell`. Cleared in `drain()`. This gives a live spinner + `mm:ss` timer with zero new channels.

### 3.3 Smooth reveal (pacing + incremental layout)

1. **Reveal pacing.** `LiveTurn::Responding` keeps a **commit buffer** (full streamed text, what gets pushed to `sess.messages` on Done) and a **display slice** `commit[..reveal_len]`. Each frame advance `reveal_len` by a budget (default ~120 chars/frame; tunable) so even one giant network chunk renders as a smooth typewriter. On `Done`, fast-forward `reveal_len = commit.len()` over ~150 ms. Code blocks within the stream are revealed by **line**, not char (they type line-by-line, not char-by-char, keeping large blocks readable). Caller-settable on `ChatPanelState` so it's testable/configurable.
2. **Incremental layout (perf enabler).** Cache the laid-out `Galley`/`LayoutJob` for the *revealed prefix* keyed by `(msg_id, prefix_hash, round(available_width))`. Since reveal is append-only, only the new tail is laid out and appended each frame; unchanged prefixes are never re-measured. Bounded by the display window + live row. Invalidate when text scale/`pixels_per_point` or width changes. Worst case (miss) = today's full render, so it can never regress correctness.
3. **Repaint cadence.** While a live turn exists, `ctx.request_repaint_after(16 ms)` unconditionally (even in idle stream gaps), so pacing is even and independent of when network chunks arrive. (Today idle gaps fall back to 100 ms — app.rs:327-334.)

### 3.4 Consistent, clean framing (simple + clean)

- **Assistant text** — gains a subtle frame (matches the reasoning-box style: `reason_bg`/`reason_border`, `ROUND_MD`, 10 px margin). The live `Responding` row uses the *same* frame so commit is invisible.
- **Reasoning** — live thinking streams into a framed box at the same slot it will occupy when committed inside the assistant row (messages.rs:88). Honors `show_reasoning_inline`; when off, show only a muted "Thinking…" line (unchanged behavior).
- **Code blocks** — already framed (`code_frame_bg`, code_block.rs:38). Make streaming-aware: stable frame, header line-count animates, inner `ScrollArea` present from the start so the block doesn't grow-jump. If the model streams a ```` ```diff ```` block, color +/- lines live with the existing `diff_add_text`/`diff_del_text` colors (cheap per-line scan, no full diff algorithm).
- **Diffs (patch results)** — the unified-diff card (diff_view.rs) is already framed; it animates in on result commit (Phase 4). No change to the LCS logic.
- **Tool cards** — keep current layouts (tool_result.rs); live `CallingTool`/`ExecutingTool` cards reuse the card frame + a spinner (`runtime.blink_dot` pattern) + elapsed timer, then the committed result card replaces them in place.
- **Shell** — live terminal (`terminal_bg`, code_block.rs:111) and committed shell card use the same frame so the live→commit swap is invisible.
- **Write preview** — `WritingFile` renders as a framed code block with a path header (same shape as the committed `write_file` card).

### 3.5 Follow behavior spec (sticky, break-on-scroll-up)

Precise semantics, replacing the current 20 px dead-zone (panel.rs:259-267):

```
Per frame:
  max_y      = max(0, content_size.y - viewport_h)
  at_bottom  = scroll_offset.y >= max_y - 1.0          // ~1px epsilon, NOT 20px

  user_scrolled_up:
    set to true  ← on any upward scroll input this frame
                    (mouse wheel/touch drag up, ArrowUp, PageUp, drag-to-scroll-up)
    set to false ← when at_bottom (user returned thumb to bottom)

  follow (auto-scroll):
    follow is ON when the user is at the bottom AND has not broken stickiness.
    While follow ON: keep offset pinned to max_y (content growth below scrolls with it).
    While follow OFF (user_scrolled_up): do NOT touch offset — never yank the view;
      content may grow below/above without moving the viewport.
    Re-engage follow the instant at_bottom becomes true again.
```

Implementation notes (non-breaking):
- Keep `ScrollArea::stick_to_bottom(true)` as the base (it already disengages when the user scrolls away and re-anchors when they return to the bottom).
- Replace the `offset.y < max_y - 20.0` computation with `at_bottom = offset.y >= max_y - 1.0`, and compute `user_scrolled_up` from **explicit input** (keyboard/wheel/touch deltas this frame) OR `!at_bottom`, whichever is stricter — so a 1 px scroll up breaks it instantly.
- Preserve existing keyboard scroll (panel.rs:278-307) and session-switch scroll restore (session.rs `restore_scroll_offset`) unchanged.
- `scroll_to_bottom` stays for explicit events (send message, replay) which should force-snap to bottom.

---

## 4. Implementation plan (phases)

Ordered for early value, each phase compiles and is independently mergeable.

### Phase 1 — Tool-call & tool-lifecycle streaming (biggest visible win, fully additive)
1. `crates/ai/src/provider/types.rs`: add `ToolCallDelta { index, name, arguments }` variant to `ProviderEvent`.
2. `crates/ai/src/provider/http.rs`: emit `ToolCallDelta` in the `delta["tool_calls"]` loop (http.rs:598-614). Keep full `ToolCall` at finish (http.rs:630-649) unchanged.
3. `crates/ai/src/chat/runtime.rs`: add `live_tool_call: Option<(String,String)>` and `tool_batch_start: Option<Instant>`; update manual `Default` (runtime.rs:165), `drain()` (runtime.rs:221), and `is_busy()` (runtime.rs:213) is unchanged.
4. `crates/ai/src/chat/polling/stream.rs`: handle `ToolCallDelta` → `live_tool_call`; clear on batch dispatch (stream.rs:342). Set `tool_batch_start` when spawning bg tools (stream.rs:617) and at `start_next_live_shell`.
5. `crates/ai/src/chat/completion/mod.rs`: clear `live_tool_call`/`tool_batch_start` in `start_completion` (mod.rs:237-258) and in `handle_handoff` reset block (mod.rs:440-463).
6. `crates/ui/src/chat/live.rs` (new): derive `LiveTurn::CallingTool` (typing `name(...)` + monospace JSON preview of `args_so_far`, syntax-tinted via a cheap char scan, re-scanned only when the tail changes) and `LiveTurn::ExecutingTool` (spinner + timer). Render at the live slot as a tool card; on result commit the existing card replaces it.

**Verify:** no regressions to `poll_tool_results`/`commit_tool_results`; tool execution still starts only at finish.

### Phase 2 — Unified live row + framing (fixes the assistant jump)
1. Extract shared bubble/frame drawing into helpers used by both committed rows (`messages.rs`) and the live row.
2. `crates/ui/src/chat/live.rs`: adapter maps runtime buffers → `LiveTurn` each frame; render text via the same markdown/code-block helpers as committed rows, framed.
3. On `Done` (panel.rs Phase-2 append path already picks up the committed `ChatMessage`), the live row clears; a short fast-forward/insert anim covers the swap.
4. Reasoning: live thinking renders into the same framed slot it will occupy when committed; honors `show_reasoning_inline`.
5. Replace the hand-rolled live blob (panel.rs:207-252) with the unified live-row renderer.

**Verify:** committed history look changes only in that assistant messages gain a frame; scroll restore + replay still work.

### Phase 3 — Pacing + incremental layout + repaint cadence
1. `LiveTurn::Responding` gains `commit_buf` + `reveal_len`; pacing budget on `ChatPanelState`.
2. Markdown galley cache (keyed as in §3.3) used by live-row text and code-block line-count updates; bounded and evicted with the display window.
3. `code_block.rs`: honor `_streaming` — stable frame, animated line-count header, line-based reveal inside blocks; `diff` language → live +/- coloring.
4. `app.rs`: while any live turn is active, `request_repaint_after(16 ms)` even when `needs_repaint` is false.

**Verify:** 60 fps steady-state on a long reply; CPU flat because unchanged prefixes aren't re-laid-out.

### Phase 4 — Follow behavior + insert animation
1. Panel scroll: implement §3.5 (remove 20 px dead-zone; break on any upward input; re-engage at bottom; preserve keyboard + session restore).
2. Insert animation: per-row `anim_start`, 120–160 ms fade + 6–10 px translate for new committed rows and result cards.
3. Theme tokens for live-state colors/caret in `crates/ui/src/chat/theme.rs`.

**Verify:** sticky holds while streaming at bottom; a 1 px scroll up breaks it; scrolling back re-engages; reading is never yanked.

### Phase 5 — Edge cases & hardening
- Replay/truncation (panel.rs:310-333): clear `LiveTurn` + markdown cache; rebuild `display_buffer` as today.
- Session switch (session.rs `load_new_session`): clear live row + cache.
- Stop (input.rs:152): `drain()` clears live state; UI fast-forwards reveal and empties live row.
- Stall/idle timeout (stream.rs:109-137): live row shows a muted stalled indicator (reuse `NetworkStatus::stalled`).
- Truncation caps (`polling/mod.rs`:14-43): display slice stays consistent with commit buffer.
- Display-window eviction (session.rs:111): evicted rows drop their cached galleys (bounded RAM — invariant preserved).
- Background/other-tab runtimes: live row only reads the active runtime; switching tabs resets it.

---

## 5. Files touched (complete audit result)

| File | Change | Risk |
|---|---|---|
| `crates/ai/src/provider/types.rs` | +`ToolCallDelta` variant | none — additive enum variant |
| `crates/ai/src/provider/http.rs` | emit partial tool deltas while accumulating | none — commit events unchanged |
| `crates/ai/src/chat/runtime.rs` | +`live_tool_call`, `tool_batch_start`; `Default` + `drain` updates | compiler-enforced; not persisted |
| `crates/ai/src/chat/polling/stream.rs` | forward `ToolCallDelta`; clear on dispatch | contained; commit path identical |
| `crates/ai/src/chat/completion/mod.rs` | clear new transient fields on start/handoff | additive |
| `crates/ui/src/chat/state.rs` | +live row state, pacing budget, markdown cache, anim timestamps | manual `Default` update, compiler-enforced |
| `crates/ui/src/chat/live.rs` (new) | runtime → `LiveTurn`; pacing logic | new file |
| `crates/ui/src/chat/panel.rs` | unified live row; follow spec; insert anim | behavioral, isolated |
| `crates/ui/src/chat/messages.rs` | shared frame helper; frame assistant rows | visual only |
| `crates/ui/src/chat/markdown.rs` | incremental/cached layout path | opt-in; full render fallback |
| `crates/ui/src/chat/code_block.rs` | honor `_streaming`; live diff coloring | opt-in |
| `crates/ui/src/chat/tool_result.rs` | live tool cards (spinner/timer) | additive |
| `crates/ui/src/chat/theme.rs` | live-state color tokens | additive |
| `crates/ui/src/app.rs` | steady 16 ms repaint while live | repaint only |

**Untouched (verified):** `crates/core/*`, `crates/fs/*`, storage/persistence, `ChatMessage`/`Role`/`Session` schema, provider request building, tool execution commit logic, session save/load.

---

## 6. Breaking-change audit (no regressions)

| Concern | Verdict |
|---|---|
| `ProviderEvent` enum | Only consumer is `polling/stream.rs` (grep confirmed: all other sites are senders in http.rs/client.rs). Adding a variant is additive; the single match must add one arm. No `#[non_exhaustive]` issues in-crate. |
| `ChatRuntime` manual `Default` | Adding fields breaks `Default` — but that's a compile error, not a runtime break. `drain()` is updated to clear them. Runtime is never persisted. |
| `ChatPanelState` manual `Default` | Same — compiler-enforced; not persisted. |
| Persistence / disk format | Zero changes. New live state never touches `sess.messages`, `pending_writes`, or JSONL. "Disk is source of truth" invariant intact. |
| Tool execution flow | `ToolCall` at `finish_reason=="tool_calls"` still drives `pending_tool_calls` → bg exec → `push_tool_results_to_state`. `ToolCallDelta` is display-only. |
| Scrolling | `stick_to_bottom(true)` retained as base; only the 20 px dead-zone → ~1 px + explicit input break. `restore_scroll_offset` (session.rs) untouched. Keyboard handlers preserved. |
| `render_markdown` `streaming` flag | Verified currently unused (only forwarded to ignored code_block params). Safe to repurpose. |
| Repaint contract | `needs_repaint` semantics unchanged; only cadence tightens to 16 ms while live. |
| Stop / replay / session switch | All enumerated in Phase 5; they clear live state + cache exactly where they already reset display state. |
| Tests | `crates/core/tests/stability.rs` is storage/session only — unaffected. No test exhaustively matches `ProviderEvent`. |

---

## 7. Remaining minor decisions (no blockers)

1. **Pacing budget default** — start at ~120 chars/frame (smooth, near-native speed); expose as a settings value (`ChatPanelState`), so it's tunable without recompile.
2. **Live diff coloring** — apply only to fenced blocks tagged `diff` while streaming; the patch-result unified diff stays as-is (it renders only after the tool completes).
3. **Live tool preview depth** — compact "name(args…)" header + collapsible monospace JSON while typing; expands fully when the args finish. (Recommended for "simple and clean".)
4. **Insert animation scope** — animate new committed rows + result cards; never animate the scroll itself while the user is reading.

---

## 8. Suggested next step

Implement in phase order. **Phase 1** first (smallest, strictly additive, high-visibility: tool calls become visible as they stream). **Phase 2** then removes the biggest structural jump (unified live row + framing). Phases 3–5 add pacing, the follow spec, and polish. Each phase is independently mergeable and verified against the audit in §6.
