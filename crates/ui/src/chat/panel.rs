// panel.rs -- Main chat panel entry point.

use std::collections::HashMap;

use egui::{Frame, Key, Margin, RichText, ScrollArea};

use autocode_ai::chat::{ChatRuntime, QueuedKind};
use autocode_core::state::{AppState, Role};

use super::input::show_input_row;
use super::live::show_live_turn;
use super::messages::{MessageAction, TranscriptCtx, empty_state, render_message};
use super::session::{
    handle_purge_on_missing, load_new_session, restore_scroll_offset, save_old_session,
};
use super::state::ChatPanelState;
use super::tabs::show_session_tabs;
use super::theme::{FONT_LABEL, SPACE_M, theme};
use crate::helpers;

/// Gap between the floating composer overlays and the divider line above the
/// input row. The overlays are bottom-anchored to that line, so their distance
/// from the input is this constant plus the divider's own half-height — never a
/// hardcoded estimate of the input row's height.
const COMPOSER_OVERLAY_GAP: f32 = 8.0;

/// Single-line preview of a queued message: collapse whitespace and cap the
/// length, so a long follow-up can't stretch the floating popup off-screen.
fn queued_preview(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = collapsed.chars().take(80).collect();
    if collapsed.chars().count() > 80 {
        out.push('…');
    }
    out
}

/// What the user asked to do with one queued message from the floating popup.
enum QueuedAction {
    /// Send it right away, interrupting the running turn.
    Inject(usize),
    /// Discard it.
    Cancel(usize),
    /// Pull it back into the input box so it can be edited and re-queued.
    Edit(usize),
}

/// Clickable, single-line preview of a queued message.
///
/// Rendered as a plain label rather than a button so the popup's action
/// buttons stay the only chrome; a subtle outline appears on hover to signal
/// that the preview itself is clickable.
/// The `+2 files` badge shown beside a queued message that is carrying
/// attachments, or `None` when it carries none.
fn queued_file_badge(attachments: usize) -> Option<String> {
    match attachments {
        0 => None,
        1 => Some("+1 file".to_string()),
        n => Some(format!("+{n} files")),
    }
}

fn queued_preview_widget(
    ui: &mut egui::Ui,
    text: &str,
    attachments: usize,
    kind: QueuedKind,
) -> bool {
    // A process notice is machine-generated: it cannot usefully be pulled back
    // into the input box for editing, so only a real user follow-up is
    // click-to-edit. Both kinds stay injectable/cancellable from the row buttons.
    let editable = kind == QueuedKind::User;
    if kind == QueuedKind::Process {
        ui.label(RichText::new("process").size(11.0).color(theme().accent))
            .on_hover_text("Background-process completion, delivered as a 'process' turn");
    }
    let resp = ui.add(
        egui::Label::new(
            RichText::new(queued_preview(text))
                .size(FONT_LABEL)
                .color(theme().text_primary),
        )
        .sense(if editable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        }),
    );
    // A queued message carries its attachments with it, but they are staged off
    // to the side until it is delivered — without this the files riding along
    // are invisible, and a queue entry looks like plain text.
    if let Some(badge) = queued_file_badge(attachments) {
        ui.label(RichText::new(badge).size(11.0).color(theme().text_muted))
            .on_hover_text("This queued message carries attached files");
    }
    if editable && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_stroke(
            resp.rect.expand(2.0),
            3.0,
            egui::Stroke::new(1.0, theme().accent),
            egui::StrokeKind::Inside,
        );
    }
    if editable {
        resp.on_hover_text("Click to edit this queued message")
            .clicked()
    } else {
        resp.on_hover_text("Waiting for the turn to finish; 'Inject now' sends it immediately");
        false
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    show_session_tabs(ui, state, runtimes, panel_state);
    ui.separator();

    let chat_salt = state.active_session_id.as_deref().unwrap_or("").to_owned();
    ui.push_id((panel_state.chat_panel_id, chat_salt), |ui| {
        // On session switch: persist old session, evict from RAM only if no live runtime.
        if panel_state.prev_session_id != state.active_session_id {
            save_old_session(state, runtimes, panel_state);
            let purge_on_missing = load_new_session(state, panel_state);
            handle_purge_on_missing(purge_on_missing, state, panel_state);
            panel_state.prev_message_count = panel_state.display_buffer.len();
            panel_state.wants_older_messages = false;
            panel_state.oldest_disk_id = 0;
            restore_scroll_offset(ui, state, panel_state);
            panel_state.prev_session_id = state.active_session_id.clone();
            // Drop reveal pacing for sessions that no longer exist.
            let valid_ids: std::collections::HashSet<String> =
                state.sessions.iter().map(|s| s.id.clone()).collect();
            panel_state.prune_live_reveals(&valid_ids);
        }

        // Handle "Load full history" — load all messages from disk.
        if panel_state.wants_older_messages {
            panel_state.wants_older_messages = false;
            if let (Some(proj), Some(sess)) = (state.active_project(), state.active_session()) {
                let mut all = autocode_core::storage::load_all_messages(proj, sess);
                all.retain(|m| m.role != Role::Error);
                // Deduplicate by ID — the persistence thread may have appended
                // stale messages after a replay truncation rewrote the files.
                {
                    let mut seen = std::collections::HashSet::new();
                    all.retain(|m| seen.insert(m.id));
                }
                if !all.is_empty() {
                    let max_disk = all.iter().map(|m| m.id).max().unwrap_or(0);
                    for msg in &sess.messages {
                        if msg.id > max_disk && msg.role != Role::Error {
                            all.push(msg.clone());
                        }
                    }
                    panel_state.display_buffer = all;
                    panel_state.prev_message_count = sess.messages.len();
                }
            }
        }

        // Track oldest message ID for scroll-back eviction.
        panel_state.loaded_min_id = panel_state
            .display_buffer
            .iter()
            .map(|m| m.id)
            .min()
            .unwrap_or(0);

        // Phase 2: new messages arrived — append to display_buffer.
        if let Some(sess) = state.active_session() {
            let current_count = sess.messages.len();
            if current_count > panel_state.prev_message_count {
                for msg in &sess.messages[panel_state.prev_message_count..current_count] {
                    panel_state.display_buffer.push(msg.clone());
                }
                panel_state.prev_message_count = current_count;
                if !panel_state.user_scrolled_up {
                    panel_state.scroll_to_bottom = true;
                }
            } else if current_count < panel_state.prev_message_count {
                // Messages were removed (trimmed or errors cleared by send_message).
                // Rebuild the display buffer so stale entries (e.g. cleared errors)
                // don't accumulate.
                panel_state.display_buffer = sess.messages.to_vec();
                panel_state.prev_message_count = current_count;
                if !panel_state.user_scrolled_up {
                    panel_state.scroll_to_bottom = true;
                }
            }
        }

        // --- scoped active-runtime block ----------------------------------------
        // `chat_w` is measured OUTSIDE the scroll area: the one trustworthy width
        // (the scroll content ui can be stretched far past the screen). Hoisted to
        // the function scope because `show_input_row` below also needs it.
        let chat_w = ui.available_width();
        let active_sid_str = state.active_session_id.clone().unwrap_or_default();
        {
            let active_sid = state.active_session_id.clone();
            let runtime = active_sid.as_ref().and_then(|sid| runtimes.get_mut(sid));
            let is_live_session = active_sid.is_some() && runtime.is_some();
            let streaming =
                is_live_session && runtime.as_deref().is_some_and(ChatRuntime::has_visible_stream);
            if streaming {
                panel_state.scroll_to_bottom = true;
            }

            // Reserve what the input row will occupy plus the separator above it
            // (6 px line + 5 px item spacing either side = 16 px, plus a little
            // headroom) so the scroll area can never overlap the row. Derived
            // from the row's own metrics instead of a magic number that drifts
            // whenever the control height changes.
            let input_row_h = super::input::input_row_height(ui) + 20.0;
            let scroll_h = (ui.available_height() - input_row_h).max(40.0);

            let scroll_resp = ScrollArea::both()
                .id_salt(panel_state.chat_scroll_id)
                .max_height(scroll_h)
                .stick_to_bottom(true)
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let inner_max_w = (chat_w - 30.0).max(200.0);
                    ui.set_min_width(inner_max_w);
                    ui.set_max_width(inner_max_w);
                    ui.add_space(6.0);
                    let bubble_indent = Margin {
                        left: 6,
                        right: 6,
                        top: 0,
                        bottom: 0,
                    };
                    Frame::NONE.inner_margin(bubble_indent).show(ui, |ui| {
                        ui.set_min_width(inner_max_w - 12.0);
                        ui.set_max_width(inner_max_w - 12.0);
                        // Exact content width every card must fit in. The
                        // scroll area is horizontally unbounded, so wrap
                        // decisions can't use ui metrics — and chat_w itself
                        // is 42px wider than what fits (scroll reservation +
                        // frame indent), which pushed full-width cards past
                        // the right edge with no right padding.
                        let content_w = inner_max_w - 12.0;
                        if !panel_state.display_buffer.is_empty() {
                            if panel_state.oldest_disk_id > 0
                                && panel_state.loaded_min_id > panel_state.oldest_disk_id
                            {
                                if ui.button("Load full history...").clicked() {
                                    panel_state.wants_older_messages = true;
                                }
                                ui.add_space(8.0);
                            }
                            ui.push_id(
                                (
                                    panel_state.chat_messages_id,
                                    active_sid.as_deref().unwrap_or(""),
                                ),
                                |ui| {
                                    // Staged-attachment dir for bubble thumbnails.
                                    let att_dir: Option<std::path::PathBuf> = state
                                        .active_session()
                                        .and_then(|sess| {
                                            sess.project_id.as_ref().and_then(|pid| {
                                                state.projects.iter().find(|p| &p.id == pid).map(
                                                    |proj| {
                                                        autocode_core::storage::session_messages_dir(proj, sess)
                                                    },
                                                )
                                            })
                                        });
                                    let ctx = TranscriptCtx {
                                        width: content_w,
                                        show_reasoning: state.show_reasoning_inline,
                                        att_dir,
                                        interactive: true,
                                        state,
                                    };
                                    for msg in panel_state.display_buffer.iter() {
                                        let action = render_message(
                                            ui,
                                            msg,
                                            &ctx,
                                            &mut panel_state.attachment_textures,
                                            &mut panel_state.diff_cache,
                                        );
                                        match action {
                                            MessageAction::Replay(msg_id) => {
                                                helpers::set_temp(
                                                    ui.ctx(),
                                                    helpers::data::REPLAY_ACTION,
                                                    Some((active_sid_str.clone(), msg_id)),
                                                );
                                            }
                                            MessageAction::OpenAgent(agent_sid) => {
                                                panel_state.agent_windows.insert(agent_sid);
                                            }
                                            MessageAction::None => {}
                                        }
                                        ui.add_space(SPACE_M);
                                    }
                                },
                            ); // end push_id("chat_messages", ...)
                        } else {
                            empty_state(ui, state);
                        }

                        if is_live_session {
                            let r = match runtime.as_ref() {
                                Some(r) => r,
                                None => return,
                            };
                            if r.retry_after.is_some() {
                                ui.add_space(SPACE_M);
                                ui.label(
                                    RichText::new(&r.status)
                                        .size(FONT_LABEL)
                                        .color(theme().text_muted),
                                );
                                ui.add_space(SPACE_M);
                            } else {
                                // Live reveal pacing is scoped to this surface.
                                let live = panel_state.live_reveal(&active_sid_str);
                                let rendered = show_live_turn(
                                    ui,
                                    r,
                                    live,
                                    state.show_reasoning_inline,
                                    content_w,
                                );
                                if !rendered && r.is_busy() {
                                    // Busy with nothing to stream yet (e.g. waiting
                                    // for the first delta) -- show the status line.
                                    ui.add_space(SPACE_M);
                                    ui.label(
                                        RichText::new(&r.status)
                                            .size(FONT_LABEL)
                                            .color(theme().text_muted),
                                    );
                                    ui.add_space(SPACE_M);
                                }
                            }
                            // Live sub-agent cards (D8): rendered while any
                            // spawned agent of this batch is outstanding.
                            if !r.pending_agents.is_empty() {
                                let handles: Vec<(String, u64)> = r
                                    .pending_agents
                                    .iter()
                                    .filter(|h| h.result.is_none())
                                    .map(|h| {
                                        (h.agent_session_id.clone(), h.started.elapsed().as_secs())
                                    })
                                    .collect();
                                crate::agents::show_agent_cards(
                                    ui,
                                    state,
                                    &handles,
                                    panel_state,
                                    content_w,
                                );
                            }
                        }
                    });
                }); // end ScrollArea

            panel_state.scroll_area_id = Some(scroll_resp.id);

            // Use scroll_resp.state directly instead of manual persistence round-trips.
            let max_y = (scroll_resp.content_size.y - scroll_resp.inner_rect.height()).max(0.0);
            // Follow behavior: only treat the user as scrolled up once they move
            // away from the bottom (~1px epsilon). Any upward input breaks follow.
            panel_state.user_scrolled_up = scroll_resp.state.offset.y < max_y - 1.0;
            // Force scroll to bottom when within 20px threshold so new content
            // (user, assistant, or tool) appears right away.
            if !panel_state.user_scrolled_up && scroll_resp.state.offset.y < max_y {
                // scroll_to_bottom will be handled by stick_to_bottom on next frame
                panel_state.scroll_to_bottom = true;
            }
            // Evict loaded history when back near bottom.
            if !panel_state.user_scrolled_up {
                let window = state.ui_display_window;
                let overshoot = panel_state.display_buffer.len().saturating_sub(window);
                if overshoot > 0 {
                    let tail = panel_state.display_buffer.split_off(overshoot);
                    panel_state.display_buffer = tail;
                }
            }

            if !ui.ctx().text_edit_focused()
                && !ui.ctx().memory(|mem| mem.has_focus(scroll_resp.id))
            {
                let delta = ui.ctx().input(|i| {
                    if i.key_pressed(Key::ArrowDown) {
                        100.0f32
                    } else if i.key_pressed(Key::ArrowUp) {
                        -100.0
                    } else if i.key_pressed(Key::PageDown) {
                        400.0
                    } else if i.key_pressed(Key::PageUp) {
                        -400.0
                    } else {
                        0.0
                    }
                });
                if delta != 0.0 {
                    panel_state.scroll_to_bottom = false;
                    if delta < 0.0 {
                        panel_state.user_scrolled_up = true;
                    }
                    ui.scroll_with_delta(egui::vec2(0.0, delta));
                    // Re-check if we hit bottom after scrolling
                    let max_offset =
                        (scroll_resp.content_size.y - scroll_resp.inner_rect.height()).max(0.0);
                    if scroll_resp.state.offset.y >= max_offset - 1.0 {
                        panel_state.user_scrolled_up = false;
                    }
                }
            }
        } // end scoped block — runtime borrow is released here

        // Handle any pending replay action from a ↺ button click.
        // The action was stored by show_user_bubble during the message loop above.
        let replay =
            helpers::take_temp::<Option<(String, u64)>>(ui.ctx(), helpers::data::REPLAY_ACTION)
                .flatten();
        if let Some((sid, msg_id)) = replay
            && let Some(text) = autocode_ai::chat::replay_to_message(state, runtimes, &sid, msg_id)
        {
            // Rebuild the display buffer from the truncated session.
            if let Some(sess) = state.active_session() {
                panel_state.display_buffer = sess.messages.to_vec();
                panel_state.loaded_min_id = panel_state
                    .display_buffer
                    .first()
                    .map(|m| m.id)
                    .unwrap_or(0);
            }
            // Force Phase 2 to re-read on the next frame as a safety net.
            panel_state.prev_message_count = usize::MAX;
            panel_state.input = text;
            panel_state.scroll_to_bottom = true;
            panel_state.wants_input_focus = true;
            ui.ctx().request_repaint();
        }

        // Execute any agent-cancel requested from a card or agent window.
        if let Some(agent_sid) =
            helpers::take_temp::<Option<String>>(ui.ctx(), helpers::data::CANCEL_AGENT_ACTION)
                .flatten()
            && autocode_ai::chat::cancel_agent(state, runtimes, &agent_sid)
        {
            ui.ctx().request_repaint();
        }

        // Drag-and-drop attachments onto the chat panel (F3 D7).
        let (dropped_paths, hovering, pointer) = ui.ctx().input(|i| {
            let dropped: Vec<String> = i
                .raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            let hovering = !i.raw.hovered_files.is_empty();
            (dropped, hovering, i.pointer.latest_pos())
        });
        if hovering
            && let Some(pos) = pointer
        {
            // Hover highlight over the whole panel while files drag.
            ui.ctx().request_repaint();
            let screen = ui.clip_rect();
            let _ = pos;
            ui.painter().rect_filled(
                screen,
                0.0,
                egui::Color32::from_rgba_premultiplied(60, 90, 130, 40),
            );
            ui.painter().rect_stroke(
                screen,
                0.0,
                egui::Stroke::new(2.0, crate::theme::Palette::ACCENT),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                screen.center(),
                egui::Align2::CENTER_CENTER,
                "Drop files to attach",
                egui::FontId::proportional(18.0),
                crate::theme::Palette::TEXT_PRIMARY,
            );
        }
        if !dropped_paths.is_empty() && state.active_session_id.is_some() {
            for err in super::attachments::stage_paths(state, panel_state, &dropped_paths) {
                eprintln!("[attachments] {}", err);
            }
            ui.ctx().request_repaint();
        }

        // The divider between the transcript and the composer. It is drawn
        // *before* the overlay below so the overlay can anchor on the line
        // itself rather than on a guessed offset from the bottom of the panel
        // (which drifts with the control height, font size and DPI scale).
        // `Separator` paints its line through the centre of the band it
        // allocates, so that centre *is* the line. (Drawing it first is
        // otherwise a no-op: `Area` lives on its own layer, and the input row
        // is still drawn after this.)
        let composer_line_y = ui.separator().rect.center().y;
        panel_state.composer_divider_y = Some(composer_line_y);

        // Floating composer overlays: queued follow-up messages (each with
        // "Inject now" / "Cancel") and the pending attachment chips. Both
        // float above the input row, overlapping the chat scroll area, so the
        // input never gets pushed off-screen. The queue stacks above the
        // chips, which stay closest to the input. Queued messages represent a
        // turn the user typed while the AI was still working; clicking "Inject
        // now" interrupts the running turn and sends it immediately, while
        // clicking the message itself pulls it back into the input to edit.
        let composer_sid = state.active_session_id.clone();
        // (text, attachment count, kind) per queued message, in queue order.
        let queued: Vec<(String, usize, QueuedKind)> = composer_sid
            .as_ref()
            .and_then(|sid| runtimes.get(sid))
            .map(|r| {
                r.queued_messages
                    .iter()
                    .map(|q| (q.text.clone(), q.attachments.len(), q.kind))
                    .collect()
            })
            .unwrap_or_default();
        let mut queued_action: Option<QueuedAction> = None;

        if !queued.is_empty() || !panel_state.pending_attachments.is_empty() {
            // Only the left edge is taken from the (now post-separator) layout
            // rect; the vertical anchor comes from the divider above.
            let avail = ui.available_rect_before_wrap();
            let max_w = (chat_w - 40.0).max(220.0);
            egui::Area::new(panel_state.chat_panel_id.with("composer_overlay"))
                .fixed_pos(egui::pos2(
                    avail.left() + 10.0,
                    composer_line_y - COMPOSER_OVERLAY_GAP,
                ))
                .pivot(egui::Align2::LEFT_BOTTOM)
                .order(egui::Order::Foreground)
                .interactable(true)
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(max_w);
                    ui.vertical(|ui| {
                        if !queued.is_empty() {
                            egui::Frame::NONE
                                .fill(theme().bg_base)
                                .corner_radius(4)
                                .stroke(egui::Stroke::new(1.0, theme().border))
                                .inner_margin(egui::Margin::symmetric(8, 6))
                                .shadow(egui::Shadow {
                                    offset: [0, 2],
                                    blur: 8,
                                    spread: 0,
                                    color: egui::Color32::from_black_alpha(60),
                                })
                                .show(ui, |ui| {
                                    for (i, (text, attachments, kind)) in
                                        queued.iter().enumerate()
                                    {
                                        ui.horizontal(|ui| {
                                            if queued_preview_widget(
                                                ui,
                                                text,
                                                *attachments,
                                                *kind,
                                            ) {
                                                queued_action = Some(QueuedAction::Edit(i));
                                            }
                                            if ui.small_button("Inject now").clicked() {
                                                queued_action = Some(QueuedAction::Inject(i));
                                            }
                                            if ui.small_button("Cancel").clicked() {
                                                queued_action = Some(QueuedAction::Cancel(i));
                                            }
                                        });
                                    }
                                });
                        }
                        if !panel_state.pending_attachments.is_empty() {
                            egui::Frame::NONE
                                .fill(theme().bg_base)
                                .corner_radius(4)
                                .stroke(egui::Stroke::new(1.0, theme().border))
                                .inner_margin(egui::Margin::symmetric(8, 6))
                                .shadow(egui::Shadow {
                                    offset: [0, 2],
                                    blur: 8,
                                    spread: 0,
                                    color: egui::Color32::from_black_alpha(60),
                                })
                                .show(ui, |ui| {
                                    // Reuse the same chip renderer; it handles
                                    // horizontal wrapping and the X remove
                                    // buttons.
                                    super::attachments::show_pending_chips(
                                        ui,
                                        state,
                                        panel_state,
                                    );
                                });
                        }
                    });
                });
        }
        // Apply the queue action outside the overlay's borrow of `runtimes`.
        if let Some(action) = queued_action
            && let Some(sid) = composer_sid
        {
            match action {
                QueuedAction::Inject(idx) => {
                    autocode_ai::chat::inject_queued_message_now(state, runtimes, &sid, idx);
                }
                QueuedAction::Cancel(idx) => {
                    autocode_ai::chat::cancel_queued_message(runtimes, &sid, idx);
                }
                QueuedAction::Edit(idx) => {
                    // Pull the message out of the queue and back into the input
                    // box. Its attachments return to the pending chips (skipping
                    // any already staged) so the whole follow-up is editable.
                    if let Some(mut queued) =
                        autocode_ai::chat::take_queued_message(runtimes, &sid, idx)
                    {
                        let text = std::mem::take(&mut queued.text);
                        if panel_state.input.trim().is_empty() {
                            panel_state.input = text;
                        } else {
                            if !panel_state.input.ends_with('\n') {
                                panel_state.input.push('\n');
                            }
                            panel_state.input.push_str(&text);
                        }
                        for att in queued.attachments {
                            if !panel_state
                                .pending_attachments
                                .iter()
                                .any(|p| p.id == att.id)
                            {
                                panel_state.pending_attachments.push(att);
                            }
                        }
                        panel_state.wants_input_focus = true;
                    }
                }
            }
            ui.ctx().request_repaint();
        }

        show_input_row(ui, state, runtimes, panel_state, &active_sid_str, chat_w);
    }); // end push_id("chat_panel", ...)
}

#[cfg(test)]
mod tests {
    use super::*;
    use autocode_ai::chat::QueuedMessage;

    const SCREEN: egui::Vec2 = egui::vec2(1000.0, 700.0);

    /// One queued follow-up with no files — the common case.
    fn draw_with_a_queued_message() -> (egui::Rect, f32) {
        draw_with_queued_messages(1, 0)
    }

    fn one_attachment() -> autocode_core::state::Attachment {
        autocode_core::state::Attachment {
            id: "att-1".into(),
            kind: autocode_core::state::AttachmentKind::File,
            name: "notes.txt".into(),
            mime: "text/plain".into(),
            bytes: 128,
            rel_path: "attachments/notes.txt".into(),
        }
    }

    /// Lay the whole panel out with `queued` follow-ups pending, each carrying
    /// `attachments` files, and hand back the queue popup's on-screen rect plus
    /// the divider it should be hugging.
    fn draw_with_queued_messages(queued: usize, attachments: usize) -> (egui::Rect, f32) {
        let mut state = AppState::default();
        let sid = state.create_session_for_project(None);
        state.activate_session(sid.clone());

        let rt = ChatRuntime {
            active_session_id: Some(sid.clone()),
            queued_messages: (0..queued)
                .map(|i| QueuedMessage {
                    text: format!("queued follow-up {i}"),
                    attachments: (0..attachments).map(|_| one_attachment()).collect(),
                    kind: autocode_ai::chat::QueuedKind::User,
                })
                .collect(),
            ..Default::default()
        };
        let mut runtimes: HashMap<String, ChatRuntime> = HashMap::new();
        runtimes.insert(sid, rt);

        let mut panel_state = ChatPanelState::default();
        let ctx = egui::Context::default();
        // Two passes: an `Area`'s rect is only readable on the frame after the
        // one that laid it out.
        for _ in 0..2 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
                time: Some(0.1),
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                show(ui, &mut state, &mut runtimes, &mut panel_state);
            });
        }

        let overlay = ctx
            .memory(|m| m.area_rect(panel_state.chat_panel_id.with("composer_overlay")))
            .expect("the queue popup was laid out");
        let divider = panel_state
            .composer_divider_y
            .expect("the composer divider was measured");
        (overlay, divider)
    }

    /// The popup must be anchored to the divider line itself. Sizing it from a
    /// fixed offset from the bottom of the panel (as it used to be) leaves it
    /// floating in the middle of the transcript whenever the input row renders
    /// taller than the guess.
    #[test]
    fn the_queue_popup_hugs_the_divider_above_the_input_row() {
        let (overlay, divider) = draw_with_a_queued_message();

        assert!(
            (overlay.bottom() - (divider - COMPOSER_OVERLAY_GAP)).abs() < 1.0,
            "popup bottom {} vs divider {} - gap {COMPOSER_OVERLAY_GAP}",
            overlay.bottom(),
            divider
        );
        // The panel also draws a divider under the session tabs; the composer's
        // is the lower one, so the popup belongs near the bottom of the window.
        assert!(divider > SCREEN.y * 0.5, "divider at {divider}");
        assert!(overlay.left() < SCREEN.x * 0.5, "popup hugs the left edge");
    }

    /// The badge is an extra widget in the row, so the anchoring has to survive
    /// it, and a queue that carries files must stay anchored where it was.
    #[test]
    fn the_popup_still_hugs_the_divider_with_files_and_several_messages() {
        let (overlay, divider) = draw_with_queued_messages(1, 2);
        assert!(
            (overlay.bottom() - (divider - COMPOSER_OVERLAY_GAP)).abs() < 1.0,
            "popup bottom {} vs divider {divider}",
            overlay.bottom()
        );

        // Several queued messages stack *upwards* from the same anchor, so the
        // bottom edge is the invariant and the box grows into the transcript.
        let (stacked, divider) = draw_with_queued_messages(4, 0);
        assert!((stacked.bottom() - (divider - COMPOSER_OVERLAY_GAP)).abs() < 1.0);
        assert!(
            stacked.height() > overlay.height(),
            "a deeper queue is taller: {} vs {}",
            stacked.height(),
            overlay.height()
        );
    }

    #[test]
    fn the_attachment_badge_pluralizes() {
        assert_eq!(queued_file_badge(0), None);
        assert_eq!(queued_file_badge(1).as_deref(), Some("+1 file"));
        assert_eq!(queued_file_badge(2).as_deref(), Some("+2 files"));
    }
}
