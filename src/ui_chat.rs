// ui_chat.rs -- Main chat interface.
// Session tab bar + message bubbles + markdown renderer with syntax
// highlighting + collapsible tool cards with diff views.

use std::cell::RefCell;

use egui::{
    CollapsingHeader, Color32, FontId, Frame, Key, Margin, RichText, ScrollArea, Stroke, TextEdit,
    TextFormat, Vec2,
};

use crate::{
    chat::{self, ChatRuntime},
    state::{AppState, ChatMessage, DesignSettings, Role, ToolMeta},
    theme::{Palette, ROUND_LG, ROUND_MD, ROUND_SM},
    ui_helpers,
};

thread_local! {
    static CURRENT_DESIGN: RefCell<Option<DesignSettings>> = const { RefCell::new(None) };
}

pub fn set_design(d: &DesignSettings) {
    CURRENT_DESIGN.set(Some(d.clone()));
}

pub fn design() -> DesignSettings {
    CURRENT_DESIGN.with(|c| c.borrow().clone().unwrap_or_default())
}

// -- Live theme colors from design settings ------------------------------------

fn rgb(c: [f32; 3]) -> Color32 {
    Color32::from_rgb(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    )
}

pub struct ThemeColors {
    pub accent: Color32,
    pub accent_dim: Color32,
    pub bg_surface: Color32,
    pub bg_base: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_code: Color32,
    pub border: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub user_badge: Color32,
    pub assist_badge: Color32,
    pub tool_badge: Color32,
    pub system_badge: Color32,
    pub user_bubble_fill: Color32,
    pub user_bubble_stroke: Color32,
    pub tool_bubble_fill: Color32,
    pub tool_bubble_stroke: Color32,
    pub assist_bubble_fill: Color32,
    pub assist_bubble_stroke: Color32,
    pub system_pill_fill: Color32,
    pub system_pill_stroke: Color32,
    pub error_notice_fill: Color32,
    pub error_notice_stroke: Color32,
    pub terminal_bg: Color32,
    pub terminal_text: Color32,
    pub terminal_border: Color32,
    pub terminal_label: Color32,
    pub live_terminal_bg: Color32,
    pub live_terminal_border: Color32,
    pub code_frame_bg: Color32,
    pub diff_frame_bg: Color32,
    pub diff_del_text: Color32,
    pub diff_add_text: Color32,
    pub diff_num: Color32,
    pub reason_bg: Color32,
    pub reason_border: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::from_design(&DesignSettings::default())
    }
}

impl ThemeColors {
    pub fn from_design(d: &DesignSettings) -> Self {
        Self {
            accent: rgb(d.accent_color),
            accent_dim: Palette::ACCENT_DIM,
            bg_surface: Palette::BG_SURFACE,
            bg_base: Palette::BG_BASE,
            text_primary: rgb(d.text_primary),
            text_secondary: rgb(d.text_secondary),
            text_muted: rgb(d.muted_color),
            text_code: rgb(d.code_text),
            border: Palette::BORDER,
            success: rgb(d.success_color),
            warning: rgb(d.warning_color),
            error: rgb(d.error_color),
            user_badge: rgb(d.user_badge),
            assist_badge: rgb(d.assist_badge),
            tool_badge: rgb(d.tool_badge),
            system_badge: rgb(d.system_badge),
            user_bubble_fill: rgb(d.user_bubble_fill),
            user_bubble_stroke: rgb(d.user_bubble_stroke),
            tool_bubble_fill: rgb(d.tool_bubble_fill),
            tool_bubble_stroke: rgb(d.tool_bubble_stroke),
            assist_bubble_fill: rgb(d.assist_bubble_fill),
            assist_bubble_stroke: rgb(d.assist_bubble_stroke),
            system_pill_fill: rgb(d.system_pill_fill),
            system_pill_stroke: rgb(d.system_pill_stroke),
            error_notice_fill: rgb(d.error_notice_fill),
            error_notice_stroke: rgb(d.error_notice_stroke),
            terminal_bg: rgb(d.terminal_bg),
            terminal_text: rgb(d.terminal_text),
            terminal_border: rgb(d.terminal_border),
            terminal_label: rgb(d.terminal_label_color),
            live_terminal_bg: rgb(d.live_terminal_bg),
            live_terminal_border: rgb(d.live_terminal_border),
            code_frame_bg: rgb(d.code_frame_bg),
            diff_frame_bg: rgb(d.diff_frame_bg),
            diff_del_text: rgb(d.diff_del_text),
            diff_add_text: rgb(d.diff_add_text),
            diff_num: rgb(d.diff_num_color),
            reason_bg: rgb(d.reason_bg),
            reason_border: rgb(d.reason_border),
        }
    }
}

fn theme() -> ThemeColors {
    ThemeColors::from_design(&design())
}

// -- Panel state ---------------------------------------------------------------

pub struct ChatPanelState {
    pub input: String,
    pub scroll_to_bottom: bool,
    pub needs_focus: bool,
    focus_attempts: u8,
    prev_session_id: Option<String>,
    scroll_offsets: std::collections::HashMap<String, f32>,
    scroll_area_id: Option<egui::Id>,
}

impl Default for ChatPanelState {
    fn default() -> Self {
        Self {
            input: String::new(),
            scroll_to_bottom: true,
            needs_focus: true,
            focus_attempts: 0,
            prev_session_id: None,
            scroll_offsets: std::collections::HashMap::new(),
            scroll_area_id: None,
        }
    }
}

// -- Entry point ---------------------------------------------------------------

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    panel_state: &mut ChatPanelState,
) {
    show_session_tabs(ui, state, runtime, panel_state);
    ui.separator();

    // On session switch, save/restore scroll offset using cached scroll area ID
    // (captured from the previous frame's ScrollArea).
    if panel_state.prev_session_id != state.active_session_id {
        if let Some(sa_id) = panel_state.scroll_area_id {
            // Save offset for the session we're leaving.
            if let Some(ref prev) = panel_state.prev_session_id {
                let sid = ui.ctx().data_mut(|d| {
                    d.get_persisted::<egui::scroll_area::State>(sa_id)
                        .unwrap_or_default()
                });
                panel_state
                    .scroll_offsets
                    .insert(prev.clone(), sid.offset.y);
            }
            // Restore offset for the session we're entering.
            if let Some(ref next) = state.active_session_id {
                if let Some(saved_y) = panel_state.scroll_offsets.get(next) {
                    let mut sid = ui.ctx().data_mut(|d| {
                        d.get_persisted::<egui::scroll_area::State>(sa_id)
                            .unwrap_or_default()
                    });
                    sid.offset.y = *saved_y;
                    ui.ctx().data_mut(|d| d.insert_persisted(sa_id, sid));
                } else {
                    panel_state.scroll_to_bottom = true;
                }
            }
        } else {
            // First frame — no cached ID yet, scroll to bottom.
            panel_state.scroll_to_bottom = true;
        }
        panel_state.prev_session_id = state.active_session_id.clone();
    }

    let is_live_session = state.active_session_id.is_some()
        && runtime.active_session_id.is_some()
        && state.active_session_id == runtime.active_session_id;

    let streaming = is_live_session
        && (runtime.is_busy()
            || !runtime.pending_response.is_empty()
            || !runtime.reasoning_buf.is_empty()
            || !runtime.live_shell_buf.is_empty());
    if streaming {
        panel_state.scroll_to_bottom = true;
    }

    let chat_w = ui.available_width();
    // Reserve space for the separator + input row (frame margins 6+6 + textarea 60 + gutter ≈ 92).
    let input_row_h = 92.0;
    let scroll_h = (ui.available_height() - input_row_h).max(40.0);

    let scroll_resp = ScrollArea::vertical()
        .id_salt("chat_scroll")
        .max_height(scroll_h)
        .stick_to_bottom(panel_state.scroll_to_bottom)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.set_min_width(chat_w);
            ui.add_space(6.0);
            // Indent chat content so bubbles, names, and timestamps share the
            // same 6 px left margin as the session tab row above.
            let bubble_indent = Margin {
                left: 6,
                right: 6,
                top: 0,
                bottom: 0,
            };
            Frame::NONE.inner_margin(bubble_indent).show(ui, |ui| {
                ui.set_min_width(chat_w - 12.0);
                if let Some(sess) = state.active_session() {
                    if sess.messages.is_empty() {
                        empty_state(ui);
                    } else {
                        for (i, msg) in sess.messages.iter().enumerate() {
                            if msg.role == Role::System {
                                show_system_pill(ui, msg);
                            } else if msg.role == Role::Error {
                                continue; // errors render at the bottom
                            } else if msg.role == Role::Assistant
                                && msg.content.trim().is_empty()
                                && msg.tool_calls.is_some()
                            {
                            } else {
                                show_bubble(ui, msg, i, chat_w, false);
                            }
                            ui.add_space(8.0);
                        }
                    }
                } else {
                    empty_state(ui);
                }

                if is_live_session {
                    let has_streaming =
                        !runtime.pending_response.is_empty() || !runtime.live_shell_buf.is_empty();

                    if runtime.is_busy() && !has_streaming {
                        show_waiting_bubble(ui, &runtime.status, chat_w);
                        ui.add_space(8.0);
                    } else {
                        if !runtime.reasoning_buf.is_empty() {
                            show_reasoning_bubble(ui, &runtime.reasoning_buf, chat_w, true);
                        }
                        if !runtime.pending_response.is_empty() {
                            show_streaming_bubble(ui, &runtime.pending_response, chat_w);
                        } else if !runtime.live_shell_buf.is_empty() {
                            show_live_shell_bubble(ui, &runtime.live_shell_buf, chat_w);
                        }
                    }
                }

                // Render error notices at the very bottom.
                if let Some(sess) = state.active_session() {
                    for msg in sess.messages.iter().rev() {
                        if msg.role == Role::Error {
                            show_error_notice(ui, msg);
                            ui.add_space(8.0);
                        }
                    }
                }
            }); // end bubble_indent frame
        });

    panel_state.scroll_area_id = Some(scroll_resp.id);

    // Manual keyboard scrolling: arrow keys / PgUp / PgDn scroll the chat
    // even when the scroll area doesn't have explicit keyboard focus.
    let scroll_id = scroll_resp.id;
    if !ui.ctx().text_edit_focused() && !ui.ctx().memory(|mem| mem.has_focus(scroll_id)) {
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
            let mut state: egui::scroll_area::State = ui
                .ctx()
                .data_mut(|d| d.get_persisted(scroll_id).unwrap_or_default());
            state.offset.y = (state.offset.y + delta).max(0.0);
            let max_offset =
                (scroll_resp.content_size.y - scroll_resp.inner_rect.height()).max(0.0);
            state.offset.y = state.offset.y.min(max_offset);
            ui.ctx().data_mut(|d| d.insert_persisted(scroll_id, state));
        }
    }

    if !streaming {
        panel_state.scroll_to_bottom = false;
    }

    ui.separator();
    show_input_row(ui, state, runtime, panel_state);
}

// -- Session tabs --------------------------------------------------------------

fn show_session_tabs(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtime: &mut crate::chat::ChatRuntime,
    panel_state: &mut ChatPanelState,
) {
    ui.add_space(6.0); // top padding for session tabs
    ScrollArea::horizontal()
        .id_salt("session_tabs")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.x = 2.0;

                let sessions: Vec<(String, String)> = state
                    .sessions
                    .iter()
                    .map(|s| (s.id.clone(), s.label.clone()))
                    .collect();

                // Prune stale scroll offsets before rendering tabs
                {
                    let valid_ids: std::collections::HashSet<String> =
                        state.sessions.iter().map(|s| s.id.clone()).collect();
                    panel_state
                        .scroll_offsets
                        .retain(|id, _| valid_ids.contains(id));
                }

                for (id, label) in sessions {
                    let active = state.active_session_id.as_deref() == Some(&id);
                    Frame::NONE
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(
                            1.0,
                            if active {
                                Palette::TAB_ACCENT
                            } else {
                                Color32::TRANSPARENT
                            },
                        ))
                        .corner_radius(ROUND_SM)
                        .inner_margin(Margin {
                            left: 10,
                            right: 10,
                            top: 4,
                            bottom: 4,
                        })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let tab_resp = ui.add(
                                    egui::Button::new(RichText::new(&label).size(11.5).color(
                                        if active {
                                            Palette::TAB_ACCENT
                                        } else {
                                            theme().text_muted
                                        },
                                    ))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE),
                                );
                                if tab_resp.clicked() {
                                    state.active_session_id = Some(id.clone());
                                }
                                if active {
                                    let close = ui.add(
                                        egui::Button::new(
                                            RichText::new("x").size(9.0).color(theme().text_muted),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .min_size(Vec2::new(14.0, 14.0)),
                                    );
                                    if close.on_hover_text("Close session").clicked() {
                                        crate::chat::abort_for_session(runtime, &id);
                                        panel_state.scroll_offsets.remove(&id);
                                        crate::session::delete_session(state, &id);
                                    }
                                }
                            });
                        });
                }
            });
        });
}

// -- Empty state ---------------------------------------------------------------

fn empty_state(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new("No messages yet -- type a task below and press Send (Enter).")
                .color(theme().text_muted)
                .size(13.0),
        );
    });
}

// -- Message bubbles -----------------------------------------------------------

fn show_bubble(ui: &mut egui::Ui, msg: &ChatMessage, idx: usize, panel_w: f32, suppress_ts: bool) {
    // Every widget inside a bubble — including nested ScrollAreas — gets a
    // unique parent ID derived from this message's timestamp + index.
    // This prevents scroll state from leaking between messages.
    ui.push_id((msg.timestamp, idx), |ui| {
        let is_user = msg.role == Role::User;
        let is_tool = msg.role == Role::Tool;
        let max_bubble_w = (panel_w * 0.72).max(240.0);

        if is_user {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_max_width(max_bubble_w);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(
                            RichText::new("You")
                                .size(10.0)
                                .color(theme().user_badge)
                                .strong(),
                        );
                        if !suppress_ts {
                            ui.label(
                                RichText::new(ui_helpers::format_time(msg.timestamp))
                                    .size(9.0)
                                    .color(theme().text_muted),
                            );
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Copy").size(9.0).color(theme().text_muted),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                            )
                            .on_hover_text("Copy message")
                            .clicked()
                        {
                            ui.ctx().copy_text(msg.content.clone());
                        }
                    });
                    Frame::NONE
                        .fill(theme().user_bubble_fill)
                        .corner_radius(ROUND_MD)
                        .stroke(Stroke::new(1.0, theme().user_bubble_stroke))
                        .inner_margin(Margin {
                            left: 12,
                            right: 12,
                            top: 8,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            render_markdown(ui, &msg.content);
                        });
                });
            });
        } else {
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_max_width(max_bubble_w);
                let (badge_color, badge_label) = match msg.role {
                    Role::Assistant => (theme().assist_badge, "AutoCode"),
                    Role::Tool => (theme().tool_badge, "Tool"),
                    _ => (theme().system_badge, "System"),
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        RichText::new(badge_label)
                            .size(10.0)
                            .color(badge_color)
                            .strong(),
                    );
                    if !suppress_ts {
                        ui.label(
                            RichText::new(ui_helpers::format_time(msg.timestamp))
                                .size(9.0)
                                .color(theme().text_muted),
                        );
                    }
                    if msg.token_count > 0 {
                        ui.label(
                            RichText::new(format!("{} tokens", msg.token_count))
                                .size(9.0)
                                .color(theme().text_muted),
                        );
                    }
                    if let Some(meta) = &msg.tool_meta
                        && let Some(dur) = meta.duration_ms
                    {
                        ui.label(
                            RichText::new(format!("{}ms", dur))
                                .size(9.0)
                                .color(theme().text_muted),
                        );
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Copy").size(9.0).color(theme().text_muted),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        )
                        .on_hover_text("Copy message")
                        .clicked()
                    {
                        ui.ctx().copy_text(msg.content.clone());
                    }
                });

                let (bubble_fill, bubble_stroke) = if is_tool {
                    (
                        theme().tool_bubble_fill,
                        Stroke::new(1.0, theme().tool_bubble_stroke),
                    )
                } else if is_user {
                    (
                        theme().user_bubble_fill,
                        Stroke::new(1.0, theme().user_bubble_stroke),
                    )
                } else {
                    (
                        theme().assist_bubble_fill,
                        Stroke::new(1.0, theme().assist_bubble_stroke),
                    )
                };

                Frame::NONE
                    .fill(bubble_fill)
                    .corner_radius(ROUND_MD)
                    .stroke(bubble_stroke)
                    .inner_margin(Margin {
                        left: 12,
                        right: 12,
                        top: 8,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        if is_tool {
                            render_tool_result(ui, msg, idx);
                        } else {
                            if let Some(reasoning) = &msg.reasoning_content
                                && !reasoning.is_empty()
                            {
                                CollapsingHeader::new(
                                    RichText::new("Thinking")
                                        .size(11.0)
                                        .color(theme().accent)
                                        .strong(),
                                )
                                .id_salt(format!("reasoning_saved_{}_{}", idx, msg.timestamp))
                                .default_open(false)
                                .show(ui, |ui| {
                                    Frame::NONE
                                        .fill(theme().reason_bg)
                                        .corner_radius(ROUND_SM)
                                        .stroke(Stroke::new(1.0, theme().reason_border))
                                        .inner_margin(Margin::same(8))
                                        .show(ui, |ui| {
                                            render_markdown(ui, reasoning);
                                        });
                                });
                                ui.add_space(6.0);
                            }
                            render_markdown(ui, &msg.content);
                        }
                    });
            });
        }
    }); // end push_id
}

// -- Tool result rendering with collapsible cards + diff views -----------------

fn render_tool_result(ui: &mut egui::Ui, msg: &ChatMessage, idx: usize) {
    if let Some(meta) = &msg.tool_meta {
        render_structured_tool_result(ui, msg, idx, meta);
        return;
    }

    let content = &msg.content;
    let summary = ui_helpers::extract_tool_summary(content);
    if let Some(summary) = summary {
        let id_salt = format!("tool_{}_{}", idx, msg.timestamp);
        CollapsingHeader::new(&summary)
            .id_salt(id_salt)
            .default_open(false)
            .show(ui, |ui| {
                let body = ui_helpers::get_tool_body(msg);
                render_code_block(ui, "", &body);
            });
    } else {
        render_markdown(ui, content);
    }
}

fn render_structured_tool_result(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    _idx: usize,
    meta: &ToolMeta,
) {
    match meta.tool_name.as_str() {
        "read_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!("[File] read {} — {} lines, {} bytes", path, lines, bytes);
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            ui.push_id(format!("code_{}", msg.timestamp), |ui| {
                let body = ui_helpers::get_tool_body(msg);
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                render_code_block(ui, name, &body);
            });
        }
        "read_entire_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!(
                "[File] read_entire_file {} — {} lines, {} bytes",
                path, lines, bytes
            );
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            ui.push_id(format!("code_{}", msg.timestamp), |ui| {
                let body = ui_helpers::get_tool_body(msg);
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                render_code_block(ui, name, &body);
            });
        }
        "read_files" => {
            let body = ui_helpers::get_tool_body(msg);
            let file_list = meta.file_path.as_deref().unwrap_or("");
            let file_count = if file_list.is_empty() {
                // Parse file paths from body for backwards compatibility
                // (older messages predate build_tool_meta storing file_path).
                body.lines().filter(|l| l.starts_with("path:")).count()
            } else {
                file_list.split(", ").filter(|s| !s.is_empty()).count()
            };
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!(
                "[File] read_files — {} files, {} lines, {} bytes",
                file_count, lines, bytes
            );
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            ui.push_id(format!("code_{}", msg.timestamp), |ui| {
                render_code_block(ui, "files", &body);
            });
        }
        "write_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let bytes = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!("[File] Written {} bytes to {}", bytes, path))
                    .size(12.0)
                    .color(theme().success)
                    .italics(),
            );
            if meta.is_error {
                let body = ui_helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            }
        }
        "patch_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Patch failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = ui_helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Patched {}", path))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                let old_text = meta.old_text.as_deref().unwrap_or("");
                let new_text = meta.new_text.as_deref().unwrap_or("");
                ui.push_id(format!("patch_{}", msg.timestamp), |ui| {
                    render_unified_diff(ui, old_text, new_text);
                });
            }
        }
        "run_shell" => {
            let exit_code = meta.exit_code.unwrap_or(-1);
            let header = if exit_code == 0 {
                "[OK] Shell exited 0".to_string()
            } else {
                format!("[FAIL] Shell exited {} (error)", exit_code)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if exit_code == 0 {
                        theme().success
                    } else {
                        theme().error
                    })
                    .strong(),
            );
            let body = ui_helpers::get_tool_body(msg);
            let display = strip_exit_code_trailer(&body);
            ui.push_id(format!("shell_{}", msg.timestamp), |ui| {
                render_shell_terminal(ui, display);
            });
        }
        "list_dir" => {
            let path = meta.file_path.as_deref().unwrap_or("directory");
            let count = meta.line_count.unwrap_or(0);
            ui.label(
                RichText::new(format!("[File] List directory — {} entries", count))
                    .size(12.0)
                    .color(theme().accent)
                    .strong(),
            );
            let body = ui_helpers::get_tool_body(msg);
            ui.push_id(format!("list_{}", msg.timestamp), |ui| {
                render_code_block(ui, path, &body);
            });
        }
        "delete_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Delete failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = ui_helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Deleted: {}", path))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
            }
        }
        "rename_file" => {
            let from = meta.file_path.as_deref().unwrap_or("file");
            let to = meta.old_text.as_deref().unwrap_or("?");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Rename failed: {} -> {}", from, to))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = ui_helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Renamed {} -> {}", from, to))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
            }
        }
        "create_dir" => {
            let path = meta.file_path.as_deref().unwrap_or("directory");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Create dir failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = ui_helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Created directory: {}", path))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
            }
        }
        "grep" => {
            let matches = meta.line_count.unwrap_or(0);
            let pattern = meta.old_text.as_deref().unwrap_or("");
            let path = meta.file_path.as_deref().unwrap_or("");
            let header = if matches > 0 {
                format!("[grep] \"{}\" in {} — {} match(es)", pattern, path, matches)
            } else {
                format!("[grep] \"{}\" in {} — No matches found", pattern, path)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if matches > 0 {
                        theme().accent
                    } else {
                        theme().system_badge
                    })
                    .strong(),
            );
            if matches > 0 {
                let body = ui_helpers::get_tool_body(msg);
                ui.push_id(format!("grep_{}", msg.timestamp), |ui| {
                    render_code_block(ui, "grep", &body);
                });
            }
        }
        "web_search" | "fetch_url" => {
            if meta.tool_name == "web_search" {
                ui.label(
                    RichText::new("[web] Web search")
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                render_markdown(ui, &msg.content);
            } else {
                ui.label(
                    RichText::new("[web] Fetched URL")
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                ui.push_id(format!("fetch_{}", msg.timestamp), |ui| {
                    ui.set_max_height(f32::INFINITY);
                    ScrollArea::vertical()
                        .max_height(400.0)
                        .min_scrolled_height(0.0)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            render_markdown(ui, &msg.content);
                        });
                });
            }
        }
        "todo_list" => {
            let total = meta.line_count.unwrap_or(0);
            let done = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!(
                    "[todo] Task list updated -- {}/{} complete",
                    done, total
                ))
                .size(12.0)
                .color(theme().accent)
                .italics(),
            );
        }
        "handoff" => {
            let reason = meta.old_text.as_deref().unwrap_or("no reason given");
            ui.label(
                RichText::new(format!("[handoff] {}", reason))
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
        }
        "glob" => {
            let matches = meta.line_count.unwrap_or(0);
            let pattern = meta.file_path.as_deref().unwrap_or("");
            let path = meta.old_text.as_deref().unwrap_or("");
            let header = if matches > 0 {
                format!(
                    "[glob] \"{}\" in {} — {} file(s) matched",
                    pattern, path, matches
                )
            } else {
                format!("[glob] \"{}\" in {} — No files matched", pattern, path)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if matches > 0 {
                        theme().accent
                    } else {
                        theme().system_badge
                    })
                    .strong(),
            );
            if matches > 0 {
                let body = ui_helpers::get_tool_body(msg);
                ui.push_id(format!("glob_{}", msg.timestamp), |ui| {
                    render_code_block(ui, "glob", &body);
                });
            }
        }
        _ => {
            render_markdown(ui, &msg.content);
        }
    }
}

/// Render a unified diff between old and new text with line numbers and
/// coloured backgrounds for deletions / additions.
///
/// Uses an LCS-based diff algorithm to produce multiple separate hunks
/// with surrounding context lines, separated by ` [...] ` when non-adjacent.
fn render_unified_diff(ui: &mut egui::Ui, old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    const CONTEXT: usize = 3;

    let line_data = if old_lines.len() < 2000 && new_lines.len() < 2000 {
        lcs_diff_lines(&old_lines, &new_lines)
    } else {
        simple_diff_lines(&old_lines, &new_lines)
    };

    // Find runs of changed lines (prefix != ' ')
    let mut change_runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, dl) in line_data.iter().enumerate() {
        if dl.prefix != ' ' {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start.take() {
            change_runs.push((start, i));
        }
    }
    if let Some(start) = run_start {
        change_runs.push((start, line_data.len()));
    }

    // Expand each run by CONTEXT lines; merge overlapping hunks.
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (start, end) in &change_runs {
        let hs = start.saturating_sub(CONTEXT);
        let he = (*end + CONTEXT).min(line_data.len());
        if let Some((_ps, pe)) = hunks.last_mut()
            && hs <= *pe
        {
            *pe = he.max(*pe);
            continue;
        }
        hunks.push((hs, he));
    }

    // Build final diff_lines: flatten hunks with section separators.
    let mut diff_lines: Vec<DiffLine> = Vec::new();
    for (hi, (start, end)) in hunks.iter().enumerate() {
        if hi > 0 {
            diff_lines.push(DiffLine {
                prefix: ' ',
                text: " [...] ",
                old_lineno: 0,
                new_lineno: 0,
            });
        }
        for dl in &line_data[*start..*end] {
            diff_lines.push(DiffLine {
                prefix: dl.prefix,
                text: dl.text,
                old_lineno: dl.old_lineno,
                new_lineno: dl.new_lineno,
            });
        }
    }

    if diff_lines.is_empty() {
        diff_lines.push(DiffLine {
            prefix: ' ',
            text: "(no differences)",
            old_lineno: 0,
            new_lineno: 0,
        });
    }

    // --- Build layout job with line numbers and colored backgrounds ---
    let max_line_num = diff_lines
        .iter()
        .map(|dl| {
            if dl.prefix == '-' {
                dl.old_lineno
            } else {
                dl.new_lineno
            }
        })
        .max()
        .unwrap_or(0);
    let num_width = max_line_num.to_string().len().max(2);
    let mono = FontId::monospace(12.0);

    let ctx_color = theme().text_secondary;
    let del_color = theme().diff_del_text;
    let add_color = theme().diff_add_text;
    let num_color = theme().diff_num;

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().diff_frame_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                // -- label bar --
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("diff")
                            .size(9.5)
                            .color(theme().text_muted)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy diff to clipboard")
                            .clicked()
                        {
                            let mut buf = String::new();
                            for dl in &diff_lines {
                                let trimmed = dl.text.trim_end();
                                buf.push_str(&format!("{}{}\n", dl.prefix, trimmed));
                            }
                            ui.ctx().copy_text(buf);
                        }
                    });
                });

                // -- scrollable diff content --
                let mut job = egui::text::LayoutJob {
                    wrap: egui::text::TextWrapping {
                        max_rows: usize::MAX,
                        max_width: ui.available_width(),
                        break_anywhere: true,
                        overflow_character: Some('\u{23CE}'),
                    },
                    ..Default::default()
                };

                for dl in &diff_lines {
                    let line_num = if dl.prefix == '-' {
                        dl.old_lineno
                    } else {
                        dl.new_lineno
                    };
                    let fg = match dl.prefix {
                        '-' => del_color,
                        '+' => add_color,
                        _ => ctx_color,
                    };
                    let trimmed = dl.text.trim_end();

                    // Line number column
                    job.append(
                        &format!("{:>width$} ", line_num, width = num_width),
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: num_color,
                            ..Default::default()
                        },
                    );
                    // Pipe separator
                    job.append(
                        "|",
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: num_color,
                            ..Default::default()
                        },
                    );
                    // Prefix symbol — coloured only, no background
                    job.append(
                        &format!("{} ", dl.prefix),
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: fg,
                            ..Default::default()
                        },
                    );
                    // Content — coloured text, no background
                    job.append(
                        trimmed,
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: fg,
                            ..Default::default()
                        },
                    );
                    // Newline
                    job.append(
                        "\n",
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: Color32::TRANSPARENT,
                            ..Default::default()
                        },
                    );
                }

                ScrollArea::vertical()
                    .id_salt("diff_scroll")
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(job);
                    });
            });
    }); // end scope
    ui.add_space(4.0);
}

// -- Diff helpers ---------------------------------------------------------------

struct DiffLine<'a> {
    prefix: char,
    text: &'a str,
    /// 1-based line number in the old file (0 for additions)
    old_lineno: usize,
    /// 1-based line number in the new file (0 for deletions)
    new_lineno: usize,
}

/// LCS-based diff (O(n*m) time/space). Falls back to simple diff for large files.
fn lcs_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let n = old.len();
    let m = new.len();
    let mut table = vec![0u16; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    for i in 0..n {
        for j in 0..m {
            table[idx(i + 1, j + 1)] = if old[i] == new[j] {
                table[idx(i, j)] + 1
            } else {
                table[idx(i, j + 1)].max(table[idx(i + 1, j)])
            };
        }
    }

    let mut result = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: j,
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[idx(i, j - 1)] >= table[idx(i - 1, j)]) {
            result.push(DiffLine {
                prefix: '+',
                text: new[j - 1],
                old_lineno: 0,
                new_lineno: j,
            });
            j -= 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: 0,
            });
            i -= 1;
        }
    }
    result.reverse();
    result
}

/// Simple line-by-line diff for very large files (>2000 lines).
/// Walks both files greedily, emitting matching lines as context
/// and unmatched lines as deletions / insertions.
fn simple_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let mut result = Vec::new();
    let (mut o, mut n) = (0, 0);
    while o < old.len() || n < new.len() {
        if o < old.len() && n < new.len() && old[o] == new[n] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        } else if o >= old.len() {
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            n += 1;
        } else if n >= new.len() {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            o += 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        }
    }
    result
}

// -- Waiting and streaming bubbles ---------------------------------------------

fn show_waiting_bubble(ui: &mut egui::Ui, status: &str, panel_w: f32) {
    let max_w = (panel_w * 0.72).max(240.0);
    ui.add_space(8.0);
    ui.vertical(|ui| {
        ui.set_max_width(max_w);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(
                RichText::new("AutoCode")
                    .size(10.0)
                    .color(theme().assist_badge)
                    .strong(),
            );
            ui.label(RichText::new(status).size(10.0).color(theme().warning));
        });
        Frame::NONE
            .fill(theme().bg_surface)
            .corner_radius(ROUND_MD)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.spinner();
                    ui.label(
                        RichText::new("Working...")
                            .size(12.0)
                            .color(theme().text_muted),
                    );
                });
            });
    });
}

fn show_reasoning_bubble(ui: &mut egui::Ui, text: &str, panel_w: f32, live: bool) {
    let max_w = (panel_w * 0.72).max(240.0);
    ui.add_space(4.0);
    ui.vertical(|ui| {
        ui.set_max_width(max_w);
        let label = if live { "Thinking..." } else { "Thinking" };
        ui.push_id("reasoning_live", |ui| {
            CollapsingHeader::new(
                RichText::new(label)
                    .size(11.0)
                    .color(theme().accent)
                    .strong(),
            )
            .default_open(live)
            .show(ui, |ui| {
                ui.set_max_height(f32::INFINITY);
                ScrollArea::vertical()
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        Frame::NONE
                            .fill(theme().code_frame_bg)
                            .corner_radius(ROUND_SM)
                            .stroke(Stroke::new(1.0, theme().border))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                if live {
                                    render_markdown_streaming(ui, text);
                                } else {
                                    render_markdown(ui, text);
                                }
                            });
                    });
            });
        });
    });
}

fn show_streaming_bubble(ui: &mut egui::Ui, text: &str, panel_w: f32) {
    let max_w = (panel_w * 0.72).max(240.0);
    ui.add_space(8.0);
    ui.vertical(|ui| {
        ui.set_max_width(max_w);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(
                RichText::new("AutoCode")
                    .size(10.0)
                    .color(theme().assist_badge)
                    .strong(),
            );
            ui.label(
                RichText::new("generating...")
                    .size(10.0)
                    .color(theme().text_muted),
            );
        });
        Frame::NONE
            .fill(theme().bg_surface)
            .corner_radius(ROUND_MD)
            .stroke(Stroke::new(1.0, theme().accent_dim))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                render_markdown_streaming(ui, text);
                ui.label(RichText::new("|").color(theme().accent).size(13.0));
            });
    });
}

fn show_live_shell_bubble(ui: &mut egui::Ui, text: &str, panel_w: f32) {
    let max_w = (panel_w * 0.72).max(240.0);
    ui.add_space(8.0);
    ui.vertical(|ui| {
        ui.set_max_width(max_w);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(
                RichText::new("Tool")
                    .size(10.0)
                    .color(theme().tool_badge)
                    .strong(),
            );
            ui.label(
                RichText::new("running shell...")
                    .size(10.0)
                    .color(theme().warning),
            );
            ui.spinner();
        });
        Frame::NONE
            .fill(theme().live_terminal_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().live_terminal_border))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                let mut job = egui::text::LayoutJob {
                    wrap: egui::text::TextWrapping {
                        max_rows: usize::MAX,
                        max_width: ui.available_width(),
                        break_anywhere: true,
                        overflow_character: Some('\u{23CE}'),
                    },
                    ..Default::default()
                };
                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(11.5),
                        color: theme().text_code,
                        ..Default::default()
                    },
                );
                ui.set_max_height(f32::INFINITY);
                ScrollArea::vertical()
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(job);
                    });
            });
    });
}

fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str) {
    render_code_block_impl(ui, lang, code, false)
}

const CODE_DISPLAY_MAX_LINES: usize = 5000;

fn render_code_block_impl(ui: &mut egui::Ui, lang: &str, code: &str, _streaming: bool) {
    let lines: Vec<&str> = code.lines().collect();
    let truncated_count = lines.len().saturating_sub(CODE_DISPLAY_MAX_LINES);
    let display_lines = if truncated_count > 0 {
        &lines[..CODE_DISPLAY_MAX_LINES]
    } else {
        &lines[..]
    };
    let display_text = display_lines.join("\n");

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().code_frame_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let lang_display = if lang.is_empty() { "code" } else { lang };
                    ui.label(
                        RichText::new(format!("{} | {} lines", lang_display, display_lines.len()))
                            .size(9.5)
                            .color(theme().text_muted)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy to clipboard")
                            .clicked()
                        {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                let mut code_job = egui::text::LayoutJob {
                    wrap: egui::text::TextWrapping {
                        max_rows: usize::MAX,
                        max_width: ui.available_width(),
                        break_anywhere: true,
                        overflow_character: Some('\u{23CE}'),
                    },
                    ..Default::default()
                };
                code_job.append(
                    &display_text,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(12.0),
                        color: theme().text_code,
                        ..Default::default()
                    },
                );
                let viewport_width = ui.available_width();
                ScrollArea::vertical()
                    .id_salt("code_scroll")
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        ui.set_width(viewport_width);
                        ui.label(code_job);
                    });
                if truncated_count > 0 {
                    ui.label(
                        RichText::new(format!(
                            "... {} lines truncated (use Copy for full content)",
                            truncated_count
                        ))
                        .size(10.0)
                        .color(theme().text_muted),
                    );
                }
            });
    }); // end scope
    ui.add_space(4.0);
}

fn render_inline(ui: &mut egui::Ui, line: &str) {
    // Headings
    if let Some(rest) = line.strip_prefix("### ") {
        ui.label(
            RichText::new(ui_helpers::parse_inline_formatting(rest))
                .size(13.5)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("## ") {
        ui.label(
            RichText::new(ui_helpers::parse_inline_formatting(rest))
                .size(14.5)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("# ") {
        ui.label(
            RichText::new(ui_helpers::parse_inline_formatting(rest))
                .size(16.0)
                .strong()
                .color(theme().accent),
        );
        return;
    }

    // Blockquote
    if let Some(rest) = line.strip_prefix("> ") {
        Frame::NONE
            .fill(Color32::from_rgba_premultiplied(80, 80, 120, 20))
            .corner_radius(ROUND_SM)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(ui_helpers::parse_inline_formatting(rest))
                        .size(13.0)
                        .color(theme().text_secondary),
                );
            });
        return;
    }

    // List items
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: ui.available_width(),
                break_anywhere: true,
                ..Default::default()
            },
            ..Default::default()
        };
        job.append(
            "* ",
            0.0,
            TextFormat {
                font_id: FontId::proportional(13.0),
                color: theme().accent,
                ..Default::default()
            },
        );
        ui_helpers::append_rich_inline_to_job(&mut job, rest.trim());
        ui.add_space(6.0);
        ui.label(job);
        return;
    }
    if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit())
        && let Some(rest) = rest.strip_prefix(". ")
    {
        let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: ui.available_width(),
                break_anywhere: true,
                ..Default::default()
            },
            ..Default::default()
        };
        job.append(
            &format!("{}. ", num),
            0.0,
            TextFormat {
                font_id: FontId::proportional(13.0),
                color: theme().accent,
                ..Default::default()
            },
        );
        ui_helpers::append_rich_inline_to_job(&mut job, rest.trim());
        ui.add_space(6.0);
        ui.label(job);
        return;
    }

    // Table row (basic: pipe-separated values)
    if line.contains('|') && line.trim().starts_with('|') {
        let cells: Vec<&str> = line.split('|').filter(|c| !c.trim().is_empty()).collect();
        if !cells.is_empty() {
            // Skip separator rows like |---|---|
            if cells.iter().all(|c| c.trim().trim_matches('-').is_empty()) {
                ui.add_space(1.0);
                return;
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("|").size(12.0).color(theme().text_muted));
                    }
                    ui.label(
                        RichText::new(cell.trim())
                            .size(12.0)
                            .color(theme().text_primary),
                    );
                }
            });
            return;
        }
    }

    if line.is_empty() {
        ui.add_space(3.0);
        return;
    }

    render_rich_inline(ui, line);
}

/// Render inline text with bold, italic, and inline code support.
fn render_rich_inline(ui: &mut egui::Ui, text: &str) {
    let mut job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: ui.available_width(),
            break_anywhere: true,
            ..Default::default()
        },
        ..Default::default()
    };
    ui_helpers::append_rich_inline_to_job(&mut job, text);
    ui.label(job);
}

// -- Input row -----------------------------------------------------------------

fn show_input_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    panel_state: &mut ChatPanelState,
) {
    Frame::NONE
        .fill(theme().bg_base)
        .inner_margin(Margin {
            left: 10,
            right: 8,
            top: 6,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.push_id("input_row", |ui| {
                ui.horizontal(|ui| {
                    let live = state.active_session_id.is_some()
                        && runtime.active_session_id.is_some()
                        && state.active_session_id == runtime.active_session_id;
                    let busy = live && runtime.is_busy();
                    // Buttons: Send(72) + gap(6) + TH(36) + gap(6) + Effort(44) + gap(6) + [=](28) = 198
                    // Plus item_spacing before button group in this horizontal (default ~6) = 204
                    // Small buffer for padding = 206
                    let input_w = (ui.available_width() - 206.0).max(120.0);
                    let send_enabled = !panel_state.input.trim().is_empty() && !busy;

                    let te = TextEdit::multiline(&mut panel_state.input)
                        .id(egui::Id::new("chat_input"))
                        .hint_text("Describe a task... Shift+Enter for newline")
                        .desired_width(input_w)
                        .font(egui::TextStyle::Body)
                        .text_color(theme().text_primary);

                    let resp = ui.add_sized(egui::vec2(input_w, 60.0), te);

                    // Enter sends, Shift+Enter inserts a newline.
                    // Ctrl+Enter is a no-op (not a send shortcut).
                    let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter))
                        && !ui.input(|i| i.modifiers.shift)
                        && !ui.input(|i| i.modifiers.ctrl);
                    let send_shortcut = enter_pressed && send_enabled && !busy;

                    // Focus management:
                    // 1) User-initiated clicks ALWAYS get focus (even with popups open).
                    // 2) Programmatic reclaim only on startup or after a popup closes.
                    let ctx = ui.ctx().clone();
                    let input_id = egui::Id::new("chat_input");

                    // Deferred focus: wait a few frames so the widget tree settles.
                    if resp.clicked() {
                        ctx.memory_mut(|mem| mem.request_focus(input_id));
                        panel_state.needs_focus = false;
                        panel_state.focus_attempts = 0;
                    }
                    if panel_state.needs_focus && !ctx.text_edit_focused() {
                        if panel_state.focus_attempts >= 10 {
                            ctx.memory_mut(|mem| mem.request_focus(input_id));
                            panel_state.needs_focus = false;
                            panel_state.focus_attempts = 0;
                        } else {
                            panel_state.focus_attempts += 1;
                            ctx.request_repaint();
                        }
                    }

                    // Thinking mode toggle + reasoning effort between input and action buttons.
                    let provider_key = state.active_provider.clone();
                    let (thinking, effort, thinking_supported, provider_kind, model) = state
                        .active_provider()
                        .map(|p| {
                            (
                                p.thinking_mode,
                                p.reasoning_effort.clone(),
                                p.thinking_api.supports_thinking(),
                                p.kind.clone(),
                                p.model.clone(),
                            )
                        })
                        .unwrap_or((
                            false,
                            "high".into(),
                            false,
                            // No active provider ? dead path, buttons stay greyed.
                            // Fallback kind is irrelevant here.
                            crate::state::ProviderKind::OpenRouter,
                            String::new(),
                        ));

                    // Changed from ui.vertical to ui.horizontal with center alignment
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Send / Stop button
                        if busy {
                            let stop_btn = egui::Button::new(
                                RichText::new("Stop").size(12.5).color(Color32::WHITE),
                            )
                            .fill(theme().error)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(72.0, 36.0));

                            if ui.add(stop_btn).clicked() {
                                runtime.drain();
                                runtime.status = "Stopped.".into();
                            }
                        } else {
                            let send_btn = egui::Button::new(
                                RichText::new("Send").size(12.5).color(if send_enabled {
                                    Color32::WHITE
                                } else {
                                    theme().text_muted
                                }),
                            )
                            .fill(if send_enabled {
                                theme().accent
                            } else {
                                theme().bg_surface
                            })
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(72.0, 36.0));

                            if ui.add_enabled(send_enabled, send_btn).clicked() || send_shortcut {
                                if send_shortcut && panel_state.input.ends_with('\n') {
                                    panel_state.input.pop();
                                }
                                let text = std::mem::take(&mut panel_state.input);
                                chat::send_message(state, runtime, text);
                                panel_state.scroll_to_bottom = true;
                            }
                        }

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
                                .min_size(Vec2::new(36.0, 36.0)),
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
                            && let Some(p) = state.providers.get_mut(&provider_key)
                        {
                            p.thinking_mode = !p.thinking_mode;
                        }

                        // Reasoning effort selector (always visible, greyed if unsupported/off)
                        let effort_enabled = thinking_supported && thinking;
                        let effort_label = {
                            let mut c = effort.clone();
                            if !c.is_empty() {
                                let (first, rest) = c.split_at(1);
                                c = format!("{}{}", first.to_uppercase(), rest);
                            }
                            c
                        };

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
                                .min_size(Vec2::new(44.0, 36.0)),
                            )
                            .on_hover_text("Reasoning effort");

                        let provider_key_popup = provider_key.clone();
                        let popup_id = egui::Popup::default_response_id(&effort_resp);
                        let available_efforts =
                            crate::state::reasoning_efforts_for_provider(&provider_kind, &model);
                        egui::Popup::menu(&effort_resp).show(|ui| {
                            ui.set_min_width(80.0);
                            ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                            for label in &available_efforts {
                                let display = {
                                    let mut c = label.clone();
                                    if !c.is_empty() {
                                        let (first, rest) = c.split_at(1);
                                        c = format!("{}{}", first.to_uppercase(), rest);
                                    }
                                    c
                                };
                                let selected = effort == *label;
                                if ui.selectable_label(selected, &display).clicked() {
                                    if let Some(p) = state.providers.get_mut(&provider_key_popup) {
                                        p.reasoning_effort = label.clone();
                                    }
                                    egui::Popup::close_id(ui.ctx(), popup_id);
                                }
                            }
                        });

                        let todo_icon = "[=]";
                        let todo_color = if state.show_todo {
                            theme().accent
                        } else {
                            theme().text_muted
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(todo_icon).size(12.0).color(todo_color),
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
                                // Increased height slightly to aesthetically match the 36.0px high Send button
                                .min_size(Vec2::new(28.0, 36.0)),
                            )
                            .on_hover_text("Toggle task list panel")
                            .clicked()
                        {
                            state.show_todo = !state.show_todo;
                        }
                    });
                });
            });
        });
}

fn show_error_notice(ui: &mut egui::Ui, msg: &ChatMessage) {
    ui.add_space(8.0);
    Frame::NONE
        .fill(theme().error_notice_fill)
        .corner_radius(ROUND_MD)
        .stroke(Stroke::new(1.5, theme().error_notice_stroke))
        .inner_margin(Margin {
            left: 12,
            right: 12,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("\u{26a0} {}", msg.content))
                    .size(12.5)
                    .color(theme().error)
                    .strong(),
            );
        });
}

fn show_system_pill(ui: &mut egui::Ui, msg: &ChatMessage) {
    ui.add_space(24.0);
    Frame::NONE
        .fill(theme().system_pill_fill)
        .corner_radius(ROUND_LG)
        .stroke(Stroke::new(1.0, theme().system_pill_stroke))
        .inner_margin(Margin {
            left: 12,
            right: 12,
            top: 3,
            bottom: 3,
        })
        .show(ui, |ui| {
            let preview: String = msg
                .content
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(80)
                .collect();
            ui.label(
                RichText::new(format!("System: {}", preview))
                    .size(11.0)
                    .color(Palette::PURPLE),
            );
        });
}

// -- Markdown-lite renderer with bold, italic, inline code, tables -------------

fn render_markdown(ui: &mut egui::Ui, text: &str) {
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    for line in text.lines() {
        if !in_code && line.starts_with("```") {
            in_code = true;
            code_lang = line.trim_start_matches('`').trim().to_string();
            code_buf.clear();
            continue;
        }
        if in_code {
            if line.trim() == "```" {
                render_code_block(ui, &code_lang, &code_buf);
                in_code = false;
                code_buf.clear();
            } else {
                code_buf.push_str(line);
                code_buf.push('\n');
            }
            continue;
        }
        render_inline(ui, line);
    }

    if in_code && !code_buf.is_empty() {
        render_code_block(ui, &code_lang, &code_buf);
    }
}

fn render_markdown_streaming(ui: &mut egui::Ui, text: &str) {
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    for line in text.lines() {
        if !in_code && line.starts_with("```") {
            in_code = true;
            code_lang = line.trim_start_matches('`').trim().to_string();
            code_buf.clear();
            continue;
        }
        if in_code {
            if line.trim() == "```" {
                render_code_block_streaming(ui, &code_lang, &code_buf);
                in_code = false;
                code_buf.clear();
            } else {
                code_buf.push_str(line);
                code_buf.push('\n');
            }
            continue;
        }
        render_inline(ui, line);
    }

    if in_code && !code_buf.is_empty() {
        render_code_block_streaming(ui, &code_lang, &code_buf);
    }
}

fn strip_exit_code_trailer(body: &str) -> &str {
    if let Some(pos) = body.rfind("\n\nExit code: ") {
        &body[..pos]
    } else if let Some(pos) = body.rfind("\nExit code: ") {
        &body[..pos]
    } else {
        body
    }
}

fn render_code_block_streaming(ui: &mut egui::Ui, lang: &str, code: &str) {
    render_code_block_impl(ui, lang, code, true)
}

fn render_shell_terminal(ui: &mut egui::Ui, code: &str) {
    if code.trim().is_empty() {
        return;
    }
    let lines: Vec<&str> = code.lines().collect();
    let display_text = lines.join("\n");

    let label = lines
        .first()
        .and_then(|line| line.strip_prefix("$ "))
        .unwrap_or("terminal");

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().terminal_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().terminal_border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} | {} lines", label, lines.len()))
                            .size(9.5)
                            .color(theme().terminal_label)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy full output")
                            .clicked()
                        {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                let mut job = egui::text::LayoutJob {
                    wrap: egui::text::TextWrapping {
                        max_rows: usize::MAX,
                        max_width: ui.available_width(),
                        break_anywhere: true,
                        overflow_character: Some('\u{23CE}'),
                    },
                    ..Default::default()
                };
                job.append(
                    &display_text,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(12.0),
                        color: theme().terminal_text,
                        ..Default::default()
                    },
                );
                let viewport_width = ui.available_width();
                ScrollArea::vertical()
                    .id_salt("terminal_scroll")
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        ui.set_width(viewport_width);
                        ui.label(job);
                    });
            });
    }); // end scope
    ui.add_space(4.0);
}
