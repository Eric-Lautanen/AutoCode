// processes/mod.rs -- Floating background-process window.
//
// Mirrors the task-window pattern (same frame, palette, header row, close
// semantics) but lists the session's background processes: status, command,
// elapsed time, a per-process Kill button, and an expandable output tail.
// All process data lives in the AI-side manager; this module only renders it
// and forwards kill requests.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Vec2};

use autocode_ai::chat::processes::{self, BackgroundProcess, ProcessStatus};
use autocode_core::state::AppState;

use crate::helpers;
use crate::theme::Palette;

const HEADER_ICON: &str = "[>]";
const DEFAULT_Y: f32 = 580.0;
const WINDOW_W: f32 = 340.0;

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_processes {
        return;
    }
    // Mark the popup open so the chat input defers focus, like the task windows.
    helpers::set_temp_bool(ctx, helpers::data::PROCESSES_OPEN, true);

    let session_id = state.active_session_id.clone();
    // Summaries only: the output buffer is fetched lazily for expanded rows.
    let procs: Vec<BackgroundProcess> = processes::summaries(session_id.as_deref());

    let mut open = true;
    let mut close_requested = false;
    let mut clear_clicked = false;
    let mut kill_all_clicked = false;
    let content_rect = ctx.content_rect();
    let default_x = (content_rect.right() - WINDOW_W - 50.0).max(50.0);
    let default_y = content_rect.top() + DEFAULT_Y;
    let running = procs
        .iter()
        .filter(|p| p.status == ProcessStatus::Running)
        .count();

    egui::Window::new("Processes")
        .id(egui::Id::new("ac::processes_window"))
        .title_bar(false)
        .open(&mut open)
        .resizable(true)
        .default_size([WINDOW_W, 0.0])
        .min_size([WINDOW_W, 120.0])
        .max_size([WINDOW_W, f32::INFINITY])
        .default_pos([default_x, default_y])
        .frame(
            Frame::NONE
                .fill(Palette::BG_BASE)
                .corner_radius(CornerRadius::ZERO)
                .stroke(Stroke::new(1.0, Palette::BORDER))
                .inner_margin(Margin::same(0)),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.allocate_space(egui::vec2(WINDOW_W, 0.0));
            let full_w = ui.available_width();

            Frame::NONE
                .fill(Palette::BG_SURFACE)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin {
                    left: 12,
                    right: 8,
                    top: 10,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.set_min_width(full_w);
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(HEADER_ICON).size(14.0).color(Palette::ACCENT));
                        let title = if running > 0 {
                            format!("Processes ({})", running)
                        } else {
                            "Processes".to_string()
                        };
                        ui.label(
                            RichText::new(title)
                                .size(13.0)
                                .strong()
                                .color(Palette::TEXT_PRIMARY),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("X").size(11.0).color(Palette::TEXT_MUTED),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE)
                                    .min_size(Vec2::new(20.0, 20.0)),
                                )
                                .on_hover_text("Close")
                                .clicked()
                            {
                                close_requested = true;
                            }
                            ui.add_space(4.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Clear").size(11.0).color(Palette::WARNING),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE)
                                    .min_size(Vec2::new(36.0, 20.0)),
                                )
                                .on_hover_text("Remove finished processes")
                                .clicked()
                            {
                                clear_clicked = true;
                            }
                            if running > 0 {
                                ui.add_space(4.0);
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Kill All")
                                                .size(11.0)
                                                .color(Palette::ERROR),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .min_size(Vec2::new(48.0, 20.0)),
                                    )
                                    .on_hover_text("Terminate every running process")
                                    .clicked()
                                {
                                    kill_all_clicked = true;
                                }
                            }
                        });
                    });
                });

            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(500.0)
                .show(ui, |ui| {
                    ui.set_min_width(full_w);
                    if procs.is_empty() {
                        empty_state(ui);
                    } else {
                        let item_w = full_w - 16.0;
                        for p in &procs {
                            render_process(ui, p, item_w);
                            ui.add_space(3.0);
                        }
                    }
                });

            ui.add_space(4.0);
        });

    if kill_all_clicked {
        for p in procs.iter().filter(|p| p.status == ProcessStatus::Running) {
            processes::request_kill(&p.id);
        }
        ctx.request_repaint();
    }

    if clear_clicked && let Some(sid) = state.active_session_id.clone() {
        processes::clear_finished(&sid);
        ctx.request_repaint();
    }

    if !open || close_requested {
        state.show_processes = false;
        state.process_user_dismissed = true;
        helpers::set_temp_bool(ctx, helpers::data::PROCESSES_OPEN, false);
        helpers::set_temp_bool(ctx, helpers::data::POPUP_JUST_CLOSED, true);
    }
}

fn render_process(ui: &mut egui::Ui, p: &BackgroundProcess, item_w: f32) {
    let (icon, color, bg_fill, border_color) = match &p.status {
        ProcessStatus::Running => (
            ">",
            Palette::ACCENT,
            Color32::from_rgba_premultiplied(30, 50, 80, 30),
            Color32::from_rgba_premultiplied(50, 80, 130, 70),
        ),
        ProcessStatus::Completed { exit_code: 0 } => (
            "[x]",
            Palette::SUCCESS,
            Color32::from_rgba_premultiplied(30, 70, 40, 30),
            Color32::from_rgba_premultiplied(50, 100, 60, 60),
        ),
        ProcessStatus::Completed { .. } => (
            "!",
            Palette::WARNING,
            Color32::from_rgba_premultiplied(70, 55, 20, 30),
            Color32::from_rgba_premultiplied(100, 80, 30, 60),
        ),
        ProcessStatus::Failed(_) => (
            "!",
            Palette::ERROR,
            Palette::ERROR_BG,
            Color32::from_rgba_premultiplied(120, 60, 60, 70),
        ),
        ProcessStatus::Killed => (
            "X",
            Palette::TEXT_MUTED,
            Color32::from_rgba_premultiplied(40, 40, 40, 20),
            Color32::from_rgba_premultiplied(60, 60, 60, 40),
        ),
    };

    let expanded_id = egui::Id::new(("ac::process_output", &p.id));

    Frame::NONE
        .fill(bg_fill)
        .corner_radius(CornerRadius::same(4))
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(Margin {
            left: 10,
            right: 10,
            top: 7,
            bottom: 7,
        })
        .show(ui, |ui| {
            ui.set_min_width(item_w);
            ui.set_max_width(item_w);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(12.0).color(color));
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&p.label)
                            .size(12.0)
                            .color(Palette::TEXT_PRIMARY),
                    )
                    .truncate(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if p.status == ProcessStatus::Running
                        && ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Kill").size(10.5).color(Palette::ERROR),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(30.0, 18.0)),
                            )
                            .on_hover_text("Terminate this process")
                            .clicked()
                    {
                        processes::request_kill(&p.id);
                        ui.ctx().request_repaint();
                    }
                    let elapsed = p.elapsed_secs();
                    ui.label(
                        RichText::new(format_elapsed(elapsed))
                            .size(10.0)
                            .color(Palette::TEXT_MUTED),
                    );
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("$ {}", p.command))
                            .size(10.5)
                            .color(Palette::TEXT_SECONDARY),
                    )
                    .truncate(),
                );
            });
            ui.horizontal(|ui| {
                let mut expanded = ui.data(|d| d.get_temp::<bool>(expanded_id).unwrap_or(false));
                let label = if expanded { "Hide" } else { "Output" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).size(10.0).color(Palette::ACCENT))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(42.0, 16.0)),
                    )
                    .clicked()
                {
                    expanded = !expanded;
                    ui.data_mut(|d| d.insert_temp(expanded_id, expanded));
                }
                ui.label(
                    RichText::new(p.status.label())
                        .size(10.0)
                        .color(Palette::TEXT_MUTED),
                );
                if let Some(pid) = p.pid {
                    ui.label(
                        RichText::new(format!("pid {}", pid))
                            .size(10.0)
                            .color(Palette::TEXT_MUTED),
                    );
                }
            });
            if ui.data(|d| d.get_temp::<bool>(expanded_id).unwrap_or(false)) {
                ui.add_space(3.0);
                let output = processes::get(&p.id).map(|f| f.output).unwrap_or_default();
                let body = if output.trim().is_empty() {
                    "(no output yet)".to_string()
                } else {
                    output.trim_end().to_string()
                };
                Frame::NONE
                    .fill(Palette::BG_BASE)
                    .corner_radius(CornerRadius::same(3))
                    .inner_margin(Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.set_max_width(item_w - 12.0);
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .id_salt(expanded_id)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(body)
                                        .size(10.0)
                                        .monospace()
                                        .color(Palette::TEXT_CODE),
                                );
                            });
                    });
            }
        });
}

fn format_elapsed(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

fn empty_state(ui: &mut egui::Ui) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(HEADER_ICON)
                .size(24.0)
                .color(Palette::TEXT_MUTED),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("No background processes")
                .size(12.5)
                .color(Palette::TEXT_MUTED),
        );
        ui.add_space(3.0);
        ui.label(
            RichText::new("Processes started by the AI")
                .size(10.5)
                .color(Palette::TEXT_MUTED),
        );
        ui.label(
            RichText::new("keep running here while it works")
                .size(10.5)
                .color(Palette::TEXT_MUTED),
        );
    });
    ui.add_space(24.0);
}
