use crate::helpers;
use crate::theme::{Palette, ROUND_SM};
use autocode_core::state::AppState;
use autocode_core::storage::provider_file;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke, Vec2};

use super::state::{SettingsState, Tab};
use super::{
    about::show_about, projects::show_projects, prompt::show_prompt, providers::show_providers,
    session::show_session_settings, timeouts::show_timeouts,
};

pub fn show_window(ctx: &egui::Context, state: &mut AppState, settings: &mut SettingsState) {
    if !state.settings_open {
        // Clean up stale debounce flag from a prior outside-click close.
        // egui insert_temp is persistent across frames, so we must clear it
        // here to avoid the first Settings button click being silently swallowed.
        helpers::take_temp_bool(ctx, helpers::data::SETTINGS_CLOSED_THIS_FRAME);
        return;
    }
    let mut open = true;

    let mut request_close = false;

    // Force zero rounding and no shadow — same style as the file viewer.
    ctx.global_style_mut(|s| {
        s.visuals.window_corner_radius = CornerRadius::ZERO;
        s.visuals.window_shadow = egui::Shadow::NONE;
        s.spacing.window_margin = egui::Margin::ZERO;
    });

    let window_resp = egui::Window::new("Settings")
        .id(settings.window_id)
        .title_bar(false)
        .open(&mut open)
        .resizable(true)
        .default_size([700.0, 720.0])
        .min_size([480.0, 340.0])
        .max_size([750.0, f32::INFINITY])
        .frame(
            Frame::NONE
                .fill(Palette::BG_BASE)
                .corner_radius(CornerRadius::ZERO)
                .stroke(Stroke::new(1.0, Palette::BORDER))
                .inner_margin(Margin::same(0)),
        )
        .show(ctx, |ui| {
            // Header row — same style as file viewer navbar.
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
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("[*]").size(14.0).color(Palette::ACCENT));
                        ui.label(
                            RichText::new("Settings")
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
                                .clicked()
                            {
                                request_close = true;
                            }

                            if settings.tab == Tab::Timeouts
                                && ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Reset")
                                                .size(11.0)
                                                .color(Palette::TEXT_MUTED),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .min_size(Vec2::new(50.0, 20.0)),
                                    )
                                    .on_hover_text("Reset all timeouts to defaults")
                                    .clicked()
                            {
                                state.stream_idle_timeout_secs =
                                    autocode_core::helpers::default_stream_idle_timeout();
                                state.request_timeout_secs =
                                    autocode_core::helpers::default_request_timeout();
                                state.tool_timeout_secs =
                                    autocode_core::helpers::default_tool_timeout();
                                state.shell_timeout_secs =
                                    autocode_core::helpers::default_shell_timeout();
                                state.shell_timeout_max_secs =
                                    autocode_core::helpers::default_shell_timeout_max();
                                state.max_retries = autocode_core::helpers::default_max_retries();
                                state.max_retry_wait_secs =
                                    autocode_core::helpers::default_max_retry_wait();
                            }
                        });
                    });
                });

            // Tab bar — horizontal, full width, same surface bg.
            Frame::NONE
                .fill(Palette::BG_SURFACE)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin {
                    left: 12,
                    right: 12,
                    top: 0,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        tab_btn(ui, &mut settings.tab, Tab::Providers, "Providers");
                        tab_btn(ui, &mut settings.tab, Tab::Projects, "Projects");
                        tab_btn(ui, &mut settings.tab, Tab::Prompt, "Prompt");
                        tab_btn(ui, &mut settings.tab, Tab::Session, "Session");
                        tab_btn(ui, &mut settings.tab, Tab::Timeouts, "Timeouts");
                        tab_btn(ui, &mut settings.tab, Tab::About, "About");
                    });
                });

            ui.add_space(4.0);

            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    Frame::NONE.inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.set_max_width(680.0);
                        match settings.tab {
                            Tab::Providers => show_providers(ui, state, settings),
                            Tab::Projects => show_projects(ui, state),
                            Tab::Prompt => show_prompt(ui, state),
                            Tab::Session => show_session_settings(ui, state),
                            Tab::Timeouts => show_timeouts(ui, state),
                            Tab::About => show_about(ui, state),
                        }
                    });
                });
        });

    // Close on click anywhere outside the window.
    if !request_close
        && let Some(resp) = &window_resp
        && ctx.input(|i| i.pointer.any_pressed())
        && let Some(p) = ctx.input(|i| i.pointer.interact_pos())
        && !resp.response.rect.contains(p)
    {
        request_close = true;
    }

    if request_close {
        state.settings_open = false;
        helpers::set_temp_bool(ctx, helpers::data::SETTINGS_CLOSED_THIS_FRAME, true);
        let _ = provider_file::save_providers_file(&state.providers);
    }
    if !state.settings_open {
        // Notify the chat input that a popup just closed so it can reclaim focus.
        helpers::set_temp_bool(ctx, helpers::data::POPUP_JUST_CLOSED, true);
    }
}

fn tab_btn(ui: &mut egui::Ui, current: &mut Tab, target: Tab, label: &str) {
    let selected = std::mem::discriminant(current) == std::mem::discriminant(&target);
    let (bg_fill, border_color) = if selected {
        (Palette::BG_ACTIVE, Palette::ACCENT_DIM)
    } else {
        (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT)
    };
    let text = RichText::new(label).size(12.0).color(if selected {
        Palette::ACCENT
    } else {
        Palette::TEXT_SECONDARY
    });
    let resp = Frame::NONE
        .fill(bg_fill)
        .corner_radius(ROUND_SM)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(Margin {
            left: 10,
            right: 10,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.label(text);
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    // Hover overlay — same subtle tint as file explorer items.
    if resp.hovered() && !selected {
        ui.painter().rect_filled(
            resp.rect,
            ROUND_SM,
            Color32::from_rgba_premultiplied(33, 39, 50, 40),
        );
    }

    if resp.clicked() {
        *current = target;
    }
}
