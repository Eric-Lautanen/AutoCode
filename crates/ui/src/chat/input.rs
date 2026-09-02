// input.rs -- Chat input row with send/stop buttons, thinking toggle, effort selector.

use std::collections::HashMap;

use egui::{Color32, Frame, Key, Margin, Rect, RichText, ScrollArea, Stroke, TextEdit, Vec2};

use autocode_ai::chat::{self, ChatRuntime};
use autocode_core::state::AppState;

use super::state::ChatPanelState;
use super::theme::theme;

/// Number of text rows the input box shows before it starts scrolling.
const INPUT_ROWS: usize = 2;

/// Total vertical margin a `TextEdit` adds around its text. Its default frame
/// is `Margin::symmetric(4, 2)` — 2 px top + 2 px bottom.
///
/// A multiline `TextEdit` is sized as `desired_rows * row_height + this`, and
/// `min_size().y` is **ignored** by `TextEdit` (only `min_size().x` is used),
/// so `desired_rows` is the only knob for the height. The buttons therefore
/// derive their height from the same formula instead of a guessed constant.
///
/// No horizontal equivalent is needed: children are laid out in the *inner*
/// rect, so a multiline TextEdit wraps at `desired_width - 8` and always
/// renders at exactly `desired_width`, never wider.
const TEXT_EDIT_MARGIN_Y: f32 = 4.0;

/// Horizontal gap between every control in the row.
const BTN_GAP: f32 = 6.0;

// --- Button widths ----------------------------------------------------------
// Floors for each control's width. The *actual* width is measured from the
// rendered label at layout time (see `btn_w` below) because `crate::theme`
// sets `button_padding = (10, 4)` globally. Hardcoded guesses — e.g. 28 px for
// `[=]`, which really renders at ~39 px — under-reserved the right-hand side
// and pushed the last two buttons off the right edge of the window.
const ATTACH_W: f32 = 28.0;
const SEND_W: f32 = 72.0;
const THINK_W: f32 = 36.0;
const EFFORT_W: f32 = 44.0;
const TODO_W: f32 = 28.0;
const PROJ_TODO_W: f32 = 28.0;

const TODO_ICON: &str = "[=]";
const PROJ_TODO_ICON: &str = "[~]";

/// Vertical margins of the row's own frame (`top: 6` + `bottom: 6`).
const INPUT_ROW_MARGIN_Y: f32 = 12.0;

/// Height shared by the text field and every button in the row, derived from
/// the *input box's* own rendered height (`INPUT_ROWS` of body text plus the
/// TextEdit frame margin) rather than a hardcoded number. Because it uses the
/// live row height, the two can never drift apart when the font size or the
/// DPI scale changes.
fn control_height(ui: &egui::Ui) -> f32 {
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    let row_h = ui.fonts_mut(|f| f.row_height(&body_font));
    INPUT_ROWS as f32 * row_h + TEXT_EDIT_MARGIN_Y
}

/// Total height the input row occupies: the shared control height plus the row
/// frame's vertical margins. `panel.rs` reserves this much so the chat scroll
/// area can never overlap the row — and so a hardcoded number over there
/// can't drift out of sync with what the row actually renders at.
pub(crate) fn input_row_height(ui: &egui::Ui) -> f32 {
    control_height(ui) + INPUT_ROW_MARGIN_Y
}

/// Exact rendered width of a button carrying `label` at `size` points: the
/// laid-out glyph width plus `button_padding` on both sides, floored at `min`.
///
/// Measuring beats guessing here because the input field is sized from
/// whatever width is left over — any error in these numbers lands on the
/// buttons and shoves them past the window edge instead of being absorbed.
fn btn_w(ui: &mut egui::Ui, label: &str, size: f32, min: f32) -> f32 {
    let font_id = egui::FontId::new(size, egui::FontFamily::Proportional);
    let glyph_w = ui
        .fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font_id, Color32::WHITE))
        .size()
        .x;
    (glyph_w + 2.0 * ui.spacing().button_padding.x).max(min)
}

/// `"high" -> "High"`, for the reasoning-effort button label.
fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    if let Some(first) = s.chars().next() {
        out.extend(first.to_uppercase());
        out.push_str(&s[first.len_utf8()..]);
    }
    out
}

pub(crate) fn show_input_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
    _sid: &str,
    width: f32,
) {
    // `width` is measured at panel level, outside the scroll area, so it is
    // immune to the content-width stretching that pushes these buttons off
    // screen when laid out from ui metrics inside a stretched scope.
    // Subtract the frame's inner margins (left 10 + right 8). The right
    // margin used to be 24 — narrowing it to 8 closes the obvious gap
    // between the action buttons and the app's right edge.
    //
    // Clamp to the current viewport so the row never extends past the
    // window: a parent layout that gets stretched by wide content (e.g. a
    // long un-wrapped markdown bubble) would otherwise report a width
    // larger than the actual visible area and push the right-most buttons
    // off-screen.
    let viewport_w = ui.ctx().viewport_rect().width();
    let width = width.min(viewport_w);
    let row_w = (width - 18.0).max(200.0); // minus frame inner margins (10 + 8)

    // Shared by the input box and every button in the row.
    let control_h = control_height(ui);

    let framed = Frame::NONE
        .fill(theme().bg_base)
        .inner_margin(Margin {
            left: 10,
            right: 8,
            top: (INPUT_ROW_MARGIN_Y / 2.0) as i8,
            bottom: (INPUT_ROW_MARGIN_Y / 2.0) as i8,
        })
        .show(ui, |ui| {
            ui.set_max_width(row_w);
            ui.push_id(panel_state.input_scope_id, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = BTN_GAP;
                    let active_sid = state.active_session_id.clone();
                    let busy = active_sid.as_ref().is_some_and(|sid| {
                        runtimes
                            .get(sid)
                            .is_some_and(|r| r.is_busy() || r.retry_after.is_some())
                    });

                    // Resolved before the input is laid out: the reasoning-effort
                    // label feeds into the width budget below.
                    let (thinking, effort, thinking_supported, provider_kind, model) = 'rd: {
                        // Prefer per-session values so each session remembers its thinking state.
                        if let Some(sid) = state.active_session_id.as_ref()
                            && let Some(sess) = state.sessions.iter().find(|s| &s.id == sid)
                        {
                            let p = state.active_provider();
                            let supported = p
                                .map(|p| {
                                    p.thinking_api.supports_thinking()
                                        || p.thinking_overrides.iter().any(|(k, _)| k != "off")
                                })
                                .unwrap_or(false);
                            let kind = p.map(|p| p.kind.clone()).unwrap_or_else(|| {
                                autocode_core::state::ProviderKind::new(
                                    autocode_core::helpers::provider_ids()
                                        .first()
                                        .map(|s| s.as_str())
                                        .unwrap_or("openai-compatible"),
                                )
                            });
                            let model = p.map(|p| p.model.clone()).unwrap_or_default();
                            let effort = if sess.reasoning_effort.is_empty() {
                                p.map(|p| p.reasoning_effort.clone())
                                    .unwrap_or_else(|| "high".into())
                            } else {
                                sess.reasoning_effort.clone()
                            };
                            break 'rd (sess.thinking_mode, effort, supported, kind, model);
                        }
                        let p = state.active_provider();
                        (
                            p.as_ref().map(|p| p.thinking_mode).unwrap_or(false),
                            p.as_ref()
                                .map(|p| p.reasoning_effort.clone())
                                .unwrap_or_else(|| "high".into()),
                            p.as_ref()
                                .map(|p| {
                                    p.thinking_api.supports_thinking()
                                        || p.thinking_overrides.iter().any(|(k, _)| k != "off")
                                })
                                .unwrap_or(false),
                            p.map(|p| p.kind.clone()).unwrap_or_else(|| {
                                autocode_core::state::ProviderKind::new(
                                    autocode_core::helpers::provider_ids()
                                        .first()
                                        .map(|s| s.as_str())
                                        .unwrap_or("openai-compatible"),
                                )
                            }),
                            p.as_ref().map(|p| p.model.clone()).unwrap_or_default(),
                        )
                    };

                    let available_efforts = autocode_core::helpers::reasoning_efforts_for_provider(
                        &provider_kind,
                        &model,
                    );
                    let effort = if available_efforts.contains(&effort) {
                        effort
                    } else {
                        available_efforts.first().cloned().unwrap_or(effort)
                    };
                    let effort_label = capitalize(&effort);

                    // Measure every button instead of trusting the constants:
                    // the input field takes whatever width is left over, so any
                    // error here would land on the buttons rather than being
                    // absorbed. 7 controls in the row means 6 gaps.
                    let attach_w = btn_w(ui, "+", 12.0, ATTACH_W);
                    let send_w = btn_w(ui, if busy { "Stop" } else { "Send" }, 12.5, SEND_W);
                    let think_w = btn_w(ui, "TH", 12.5, THINK_W);
                    // Widest effort label wins, so the row doesn't shift when
                    // the user picks a different effort.
                    let effort_w = available_efforts
                        .iter()
                        .map(|e| btn_w(ui, &capitalize(e), 11.5, 0.0))
                        .fold(EFFORT_W, f32::max);
                    let todo_w = btn_w(ui, TODO_ICON, 12.0, TODO_W);
                    let proj_todo_w = btn_w(ui, PROJ_TODO_ICON, 12.0, PROJ_TODO_W);

                    // The input field absorbs whatever is left, so the row adds
                    // up to exactly `row_inner_w` and the buttons stay put.
                    let reserved_w = attach_w
                        + send_w
                        + think_w
                        + effort_w
                        + todo_w
                        + proj_todo_w
                        + BTN_GAP * 6.0;
                    // Measure the live rect rather than trusting `row_w`: this
                    // is the space the layout will actually hand out, and it is
                    // clamped to the viewport so the row can never run past the
                    // window's right edge.
                    let row_inner_w = ui.available_width().min(viewport_w - 18.0);
                    let input_w = (row_inner_w - reserved_w).max(100.0);
                    let send_enabled = !panel_state.input.trim().is_empty() && !busy;

                    // Attach button — leftmost in the input row, same
                    // frame as the thinking / task-list toggles.
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("+").size(12.0).color(theme().text_secondary),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, theme().border))
                            .min_size(Vec2::new(attach_w, control_h)),
                        )
                        .on_hover_text("Attach files")
                        .clicked()
                    {
                        crate::helpers::set_temp_bool(
                            ui.ctx(),
                            crate::helpers::data::OPEN_FILE_PICKER,
                            true,
                        );
                    }

                    // The ScrollArea is what makes the row's height independent
                    // of the prompt: a multiline TextEdit grows with its content
                    // and would otherwise drag the buttons down and eventually
                    // off the bottom of the window. `max_height` caps it;
                    // `min_scrolled_height` must be set too, because its default
                    // (64 px) would override a smaller `max_height` and blow the
                    // cap. The scroll bar stays hidden so the input's width
                    // doesn't jump by a bar's width once the text overflows.
                    let resp = ScrollArea::vertical()
                        .id_salt(("input_scroll", panel_state.input_id))
                        .max_height(control_h)
                        .min_scrolled_height(control_h)
                        .max_width(input_w)
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden,
                        )
                        .show(ui, |ui: &mut egui::Ui| {
                            ui.add(
                                TextEdit::multiline(&mut panel_state.input)
                                    .id_salt(panel_state.input_id)
                                    .hint_text("Describe a task... Shift+Enter for newline")
                                    .desired_width(input_w)
                                    .desired_rows(INPUT_ROWS)
                                    .font(egui::TextStyle::Body)
                                    .text_color(theme().text_primary),
                            )
                        })
                        .inner;

                    // Remember the actual widget Id so other code can request
                    // focus on it (the final Id depends on the push_id scope).
                    panel_state.actual_input_id = Some(resp.id);

                    // Enter sends, Shift+Enter inserts a newline.
                    // Ctrl+Enter is a no-op (not a send shortcut).
                    let enter_pressed = ui.input(|i| {
                        i.key_pressed(Key::Enter) && !i.modifiers.shift && !i.modifiers.ctrl
                    });
                    let send_shortcut = enter_pressed && send_enabled && !busy;

                    // Focus management: request focus when an external caller
                    // (e.g. replay action) sets the flag, or when the user
                    // clicks the input area.
                    if panel_state.wants_input_focus {
                        panel_state.wants_input_focus = false;
                        ui.ctx().memory_mut(|mem| {
                            mem.request_focus(resp.id);
                        });
                    }
                    if resp.clicked() {
                        ui.ctx().memory_mut(|mem| {
                            mem.request_focus(resp.id);
                        });
                    }

                    // All right-side controls (Send/Stop, TH, Effort, todo
                    // toggles) sit in the SAME outer layout as the attach
                    // button and the input area — that way every control
                    // gets the same allocated height and lines up vertically
                    // instead of the nested layout clipping them shorter.

                    // Send / Stop button
                    if busy {
                        let stop_btn = egui::Button::new(
                            RichText::new("Stop").size(12.5).color(Color32::WHITE),
                        )
                        .fill(theme().error)
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::new(send_w, control_h));

                        if ui.add(stop_btn).clicked()
                            && let Some(sid) = active_sid.clone()
                        {
                            // Cancel any running sub-agents first so their
                            // results land before the runtime drains.
                            chat::settle_agents_on_stop(state, runtimes, &sid);
                            if let Some(r) = runtimes.get_mut(&sid) {
                                r.stopped_by_user = true;
                                r.drain();
                                r.status = "Stopped.".into();
                            }
                        }
                    } else {
                        let send_btn = egui::Button::new(RichText::new("Send").size(12.5).color(
                            if send_enabled {
                                Color32::WHITE
                            } else {
                                theme().text_muted
                            },
                        ))
                        .fill(if send_enabled {
                            theme().accent
                        } else {
                            theme().bg_surface
                        })
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::new(send_w, control_h));

                        if ui.add_enabled(send_enabled, send_btn).clicked() || send_shortcut {
                            if send_shortcut && panel_state.input.ends_with('\n') {
                                panel_state.input.pop();
                            }
                            let text = std::mem::take(&mut panel_state.input);
                            let atts = std::mem::take(&mut panel_state.pending_attachments);
                            chat::send_message(state, runtimes, text, atts);
                            panel_state.scroll_to_bottom = true;
                            panel_state.user_scrolled_up = false;
                        }
                    }

                    // Pending project-meta sync (thinking default + effort) so the
                    // next new session inherits the user's last toggle. The actual
                    // disk write happens after the effort picker below; gather first
                    // because the session borrow ends before we can touch projects.
                    let mut project_meta_update: Option<(Option<String>, bool, String)> = None;

                    // Thinking toggle button (always visible, greyed if unsupported)
                    let th_enabled = thinking_supported;
                    if ui
                        .add_enabled(
                            th_enabled,
                            egui::Button::new(RichText::new("TH").size(12.5).color(
                                if th_enabled && thinking {
                                    theme().accent
                                } else {
                                    theme().text_muted
                                },
                            ))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(
                                1.0,
                                if th_enabled && thinking {
                                    theme().accent
                                } else {
                                    theme().border
                                },
                            ))
                            .min_size(Vec2::new(think_w, control_h)),
                        )
                        .on_hover_text(if th_enabled {
                            if thinking {
                                "Thinking: ON"
                            } else {
                                "Thinking: OFF"
                            }
                        } else {
                            "Thinking not supported by this API"
                        })
                        .clicked()
                        && let Some(sid) = state.active_session_id.as_ref()
                        && let Some(sess) = state.sessions.iter_mut().find(|s| &s.id == sid)
                    {
                        sess.thinking_mode = !sess.thinking_mode;
                        if sess.thinking_mode && !available_efforts.contains(&sess.reasoning_effort)
                        {
                            sess.reasoning_effort = available_efforts
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "high".into());
                        }
                        state.session_meta_dirty = true;
                        project_meta_update = Some((
                            sess.project_id.clone(),
                            sess.thinking_mode,
                            sess.reasoning_effort.clone(),
                        ));
                    }

                    // Reasoning effort selector (always visible, greyed if unsupported/off)
                    let effort_enabled = thinking_supported && thinking;

                    let effort_resp = ui
                        .add_enabled(
                            effort_enabled,
                            egui::Button::new(RichText::new(&effort_label).size(11.5).color(
                                if effort_enabled {
                                    theme().text_primary
                                } else {
                                    theme().text_muted
                                },
                            ))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, theme().border))
                            .min_size(Vec2::new(effort_w, control_h)),
                        )
                        .on_hover_text("Reasoning effort");

                    let popup_id = egui::Popup::default_response_id(&effort_resp);
                    egui::Popup::menu(&effort_resp).show(|ui| {
                        ui.set_min_width(80.0);
                        ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                        for label in &available_efforts {
                            ui.push_id(("effort", label), |ui| {
                                let display = capitalize(label);
                                let selected = effort == *label;
                                if ui.selectable_label(selected, &display).clicked() {
                                    if let Some(sid) = state.active_session_id.as_ref()
                                        && let Some(sess) =
                                            state.sessions.iter_mut().find(|s| &s.id == sid)
                                    {
                                        sess.reasoning_effort = label.clone();
                                        state.session_meta_dirty = true;
                                        project_meta_update = Some((
                                            sess.project_id.clone(),
                                            true,
                                            sess.reasoning_effort.clone(),
                                        ));
                                    }
                                    egui::Popup::close_id(ui.ctx(), popup_id);
                                }
                            });
                        }
                    });

                    // Persist the user's thinking/effort choice to the project-level
                    // meta.json so new sessions in this project start with these on.
                    if let Some((pid, th, effort)) = project_meta_update
                        && let Some(pid) = pid
                        && let Some(proj) = state.projects.iter().find(|p| p.id == pid)
                    {
                        autocode_core::storage::sync_project_thinking_defaults(proj, th, &effort);
                    }

                    let todo_color = if state.show_todo {
                        theme().accent
                    } else {
                        theme().text_muted
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(TODO_ICON).size(12.0).color(todo_color),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(
                                1.0,
                                if state.show_todo {
                                    theme().accent
                                } else {
                                    theme().border
                                },
                            ))
                            .min_size(Vec2::new(todo_w, control_h)),
                        )
                        .on_hover_text("Toggle task list panel")
                        .clicked()
                    {
                        state.show_todo = !state.show_todo;
                    }

                    let project_todo_color = if state.show_project_tasks {
                        theme().accent
                    } else {
                        theme().text_muted
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(PROJ_TODO_ICON)
                                    .size(12.0)
                                    .color(project_todo_color),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(
                                1.0,
                                if state.show_project_tasks {
                                    theme().accent
                                } else {
                                    theme().border
                                },
                            ))
                            .min_size(Vec2::new(proj_todo_w, control_h)),
                        )
                        .on_hover_text("Toggle project tasks panel")
                        .clicked()
                    {
                        state.show_project_tasks = !state.show_project_tasks;
                    }
                });
            });
        });
    // Extend the dark strip to the physical window edge: panel/scroll layout
    // can leave a sliver of the lighter panel background at the right.
    let frame_rect = framed.response.rect;
    let right_band = Rect::from_min_max(
        egui::pos2(frame_rect.right(), frame_rect.top()),
        egui::pos2(ui.ctx().viewport_rect().right(), frame_rect.bottom()),
    );
    if right_band.width() > 0.0 {
        ui.painter().rect_filled(right_band, 0.0, theme().bg_base);
    }
}
