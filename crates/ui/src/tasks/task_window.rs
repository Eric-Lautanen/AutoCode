use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Vec2};

use crate::helpers;
use crate::theme::Palette;
use autocode_core::state::{TodoItem, TodoList, TodoStatus};

/// Configuration that distinguishes a session-scoped todo window
/// from a project-scoped task window.
pub struct TodoWindowConfig<'a> {
    /// Window title shown in the egui Window (not rendered — used as ID).
    pub window_title: &'a str,
    /// Icon shown in the header row (e.g. "[=]" or "[~]").
    pub header_icon: &'a str,
    /// Default Y position for the floating window.
    pub default_y: f32,
    /// egui temp-data key for "is this popup open?".
    pub open_id: &'a str,
    /// Hardcoded list title (always "Session tasks" or "Project tasks").
    pub list_title: &'a str,
    /// Hover text for the "Clear" button.
    pub clear_hover: &'a str,
    /// Lines shown in the empty-state placeholder.
    pub empty_icon: &'a str,
    pub empty_title: &'a str,
    pub empty_line1: &'a str,
    pub empty_line2: &'a str,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Render a floating todo/task-list window.
///
/// Returns `true` when the user clicks the "Clear" button so the caller
/// can perform its own persistence side-effects.
pub fn show_todo_window(
    ctx: &egui::Context,
    config: &TodoWindowConfig<'_>,
    list: &TodoList,
    is_open: bool,
) -> TodoWindowOutput {
    if !is_open {
        return TodoWindowOutput::default();
    }

    // Store open state in ctx.data so the chat input focus logic
    // can check if a popup window is currently open.
    helpers::set_temp_bool(ctx, config.open_id, true);

    let mut open = true;
    let mut clear_clicked = false;
    let close_requested = std::cell::Cell::new(false);
    let content_rect = ctx.content_rect();
    let default_x = (content_rect.right() - 300.0 - 50.0).max(50.0);
    let default_y = content_rect.top() + config.default_y;

    egui::Window::new(config.window_title)
        .title_bar(false)
        .open(&mut open)
        .resizable(true)
        .default_size([300.0, 0.0])
        .min_size([300.0, 150.0])
        .max_size([300.0, f32::INFINITY])
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
            ui.allocate_space(egui::vec2(300.0, 0.0));
            let full_w = ui.available_width();

            // Header row.
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
                        ui.label(
                            RichText::new(config.header_icon)
                                .size(14.0)
                                .color(Palette::ACCENT),
                        );
                        let title = autocode_core::helpers::truncate_str(config.list_title, 25);
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
                                close_requested.set(true);
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
                                .on_hover_text(config.clear_hover)
                                .clicked()
                            {
                                clear_clicked = true;
                            }
                        });
                    });
                });

            // Progress section.
            if !list.items.is_empty() {
                ui.add_space(4.0);
                let (done, total) = list.progress();
                let frac = if total > 0 {
                    done as f32 / total as f32
                } else {
                    0.0
                };

                Frame::NONE
                    .fill(Palette::BG_BASE)
                    .inner_margin(Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}/{} complete", done, total))
                                    .size(10.0)
                                    .color(Palette::TEXT_MUTED),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!("{:.0}%", frac * 100.0))
                                            .size(10.0)
                                            .color(if frac >= 1.0 {
                                                Palette::SUCCESS
                                            } else if frac > 0.0 {
                                                Palette::ACCENT
                                            } else {
                                                Palette::TEXT_MUTED
                                            }),
                                    );
                                },
                            );
                        });
                        ui.add_space(3.0);
                        let bar_w = ui.available_width();
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(bar_w, 5.0), egui::Sense::hover());
                        let cr = CornerRadius::same(3);
                        ui.painter().rect_filled(rect, cr, Palette::BG_SURFACE);
                        if frac > 0.0 {
                            let fill_max_x = rect.min.x + rect.width() * frac;
                            let fill_rect = egui::Rect::from_min_max(
                                rect.min,
                                egui::pos2(fill_max_x, rect.max.y),
                            );
                            let fill_color = if frac >= 1.0 {
                                Palette::SUCCESS
                            } else {
                                Palette::ACCENT
                            };
                            ui.painter().rect_filled(fill_rect, cr, fill_color);
                        }
                    });
            }

            ui.add_space(4.0);

            let cur = helpers::find_current_task_index(&list.items);
            helpers::todo_scroll_area(ui, &list.items, full_w, cur, render_item, |ui| {
                empty_state(ui, config)
            });

            ui.add_space(4.0);
        });

    // Auto-close and clear when all items are completed.
    let all_done = !list.is_empty() && list.items.iter().all(|i| i.status == TodoStatus::Completed);

    let all_done_triggered = if all_done {
        clear_clicked = true;
        true
    } else {
        false
    };

    let close_clicked = !open || close_requested.get();
    if close_clicked {
        helpers::set_temp_bool(ctx, config.open_id, false);
        helpers::set_temp_bool(ctx, helpers::data::POPUP_JUST_CLOSED, true);
    }

    TodoWindowOutput {
        close_clicked,
        clear_clicked,
        all_done_triggered,
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TodoWindowOutput {
    /// User clicked the X button or the window was otherwise closed.
    pub close_clicked: bool,
    /// User clicked "Clear" (or all items completed).
    pub clear_clicked: bool,
    /// All items were completed, triggering auto-clear.
    pub all_done_triggered: bool,
}

// ---------------------------------------------------------------------------
// Shared item renderer & empty state
// ---------------------------------------------------------------------------

fn render_item(ui: &mut egui::Ui, item: &TodoItem, item_w: f32) {
    let (icon, icon_color, bg_fill, border_color) = match item.status {
        TodoStatus::Completed => (
            "[x]",
            Palette::SUCCESS,
            Color32::from_rgba_premultiplied(30, 70, 40, 30),
            Color32::from_rgba_premultiplied(50, 100, 60, 60),
        ),
        TodoStatus::InProgress => (
            ">",
            Palette::WARNING,
            Color32::from_rgba_premultiplied(70, 55, 20, 30),
            Color32::from_rgba_premultiplied(100, 80, 30, 60),
        ),
        TodoStatus::Cancelled => (
            "X",
            Palette::TEXT_MUTED,
            Color32::from_rgba_premultiplied(40, 40, 40, 20),
            Color32::from_rgba_premultiplied(60, 60, 60, 40),
        ),
        TodoStatus::Pending => (
            "o",
            Palette::TEXT_SECONDARY,
            Palette::BG_SURFACE,
            Palette::BORDER,
        ),
    };

    let priority_dot = match item.priority.as_str() {
        "high" => Some(Palette::ERROR),
        "medium" => Some(Palette::WARNING),
        "low" => Some(Palette::SUCCESS),
        _ => None,
    };

    let text_color = match item.status {
        TodoStatus::Completed => Palette::TEXT_MUTED,
        TodoStatus::Cancelled => Palette::TEXT_MUTED,
        _ => Palette::TEXT_PRIMARY,
    };

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
                if let Some(dot_color) = priority_dot {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(6.0, 6.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.0, dot_color);
                    ui.add_space(4.0);
                }
                ui.label(RichText::new(icon).size(12.0).color(icon_color));
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(RichText::new(&item.content).size(12.0).color(text_color))
                        .truncate(),
                );
            });
        });
}

fn empty_state(ui: &mut egui::Ui, config: &TodoWindowConfig<'_>) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(config.empty_icon)
                .size(24.0)
                .color(Palette::TEXT_MUTED),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(config.empty_title)
                .size(12.5)
                .color(Palette::TEXT_MUTED),
        );
        ui.add_space(3.0);
        ui.label(
            RichText::new(config.empty_line1)
                .size(10.5)
                .color(Palette::TEXT_MUTED),
        );
        ui.label(
            RichText::new(config.empty_line2)
                .size(10.5)
                .color(Palette::TEXT_MUTED),
        );
    });
    ui.add_space(24.0);
}
