use egui::{Align, Frame, Layout, Margin};

use std::collections::HashMap;

use crate::helpers;
use crate::theme::Palette;
use autocode_ai::chat::ChatRuntime;
use autocode_core::{helpers as core_helpers, state::AppState};

use super::buttons;
use super::meters;
use super::pickers;

pub fn show(ui: &mut egui::Ui, state: &mut AppState, runtimes: &mut HashMap<String, ChatRuntime>) {
    Frame::NONE
        .fill(Palette::BG_BASE)
        .inner_margin(Margin {
            left: 12,
            right: 8,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // -- Project picker ------------------------------------
                pickers::show_project_picker(ui, state);

                // -- Session picker for the active project ----------------
                pickers::show_session_picker(ui, state);

                helpers::toolbar_separator(ui);

                // -- Provider picker ------------------------------------
                pickers::show_provider_picker(ui, state);

                // -- Model picker --------------------------------------
                pickers::show_model_picker(ui, state);

                helpers::toolbar_separator(ui);

                // -- Context budget meter ------------------------------
                let frac = core_helpers::budget_fraction(state).clamp(0.0, 1.0);
                meters::show_token_meter(ui, state, frac);

                // -- Network status indicator -------------------------
                let active_sid = state.active_session_id.clone();
                if let Some(runtime) = active_sid.as_ref().and_then(|sid| runtimes.get_mut(sid)) {
                    meters::show_network_status(ui, &mut runtime.net_status);
                } else {
                    let mut net = autocode_ai::chat::NetworkStatus::default();
                    meters::show_network_status(ui, &mut net);
                }

                // -- Right-side actions --------------------------------
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Settings (lights up when settings window is open).
                    if buttons::lit_btn(ui, "Settings", state.settings_open).clicked() {
                        let just_closed = ui.ctx().data_mut(|d| {
                            d.remove_temp::<bool>(egui::Id::new("settings_closed_this_frame"))
                                .unwrap_or(false)
                        });
                        if !just_closed {
                            state.settings_open = !state.settings_open;
                        }
                    }

                    // Explorer toggle (lights up when explorer is open).
                    if buttons::lit_btn(ui, "Files", state.show_explorer).clicked() {
                        state.show_explorer = !state.show_explorer;
                    }

                    // Handoff toggle (lights up when enabled).
                    buttons::show_handoff_toggle(ui, state);

                    // Reasoning visibility toggle.
                    if buttons::lit_btn(ui, "Reasoning", state.show_reasoning_inline)
                        .on_hover_text("Show/hide AI reasoning in chat")
                        .clicked()
                    {
                        state.show_reasoning_inline = !state.show_reasoning_inline;
                    }
                });
            });
        });
}
