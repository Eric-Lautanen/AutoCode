// ui_toolbar.rs -- Top toolbar.
// Project picker * Provider/model picker * Token budget meter * Action buttons.
// Design: single horizontal strip, uses egui Visuals for colours, minimal chrome.

use egui::{Align, Frame, Layout, Margin, RichText, Sense, Stroke, StrokeKind, Vec2};

use std::collections::HashMap;

use crate::helpers;
use autocode_ai::{chat::ChatRuntime, session};
use autocode_core::{
    helpers as core_helpers,
    state::{AppState, Project},
    theme::Palette,
};

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
                let proj_label = state
                    .active_project()
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "No Project".into());

                egui::ComboBox::from_id_salt("project_picker")
                    .selected_text(
                        RichText::new(&proj_label)
                            .size(12.0)
                            .color(Palette::TEXT_SECONDARY),
                    )
                    .show_ui(ui, |ui| {
                        let projects: Vec<Project> = state.projects.clone();
                        for p in &projects {
                            ui.push_id(("proj_sel", &p.id), |ui| {
                                let selected = state.active_project_id.as_deref() == Some(&p.id);
                                if ui.selectable_label(selected, &p.name).clicked() {
                                    autocode_core::session_storage::switch_to_project(state, &p.id);
                                    session::ensure_session(state);
                                }
                            });
                        }
                        ui.separator();
                        if ui.button("New Project...").clicked() {
                            let current_dir = std::env::current_dir()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| ".".to_string());
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(egui::Id::new("open_new_project"), true);
                                d.insert_temp(
                                    egui::Id::new("new_project_dialog_path"),
                                    current_dir,
                                );
                            });
                        }
                    });

                // -- Session picker for the active project ----------------
                if let Some(ref pid) = state.active_project_id {
                    let sessions_here: Vec<(String, String)> = state
                        .sessions
                        .iter()
                        .filter(|s| {
                            s.project_id.as_ref() == Some(pid)
                                && state
                                    .projects
                                    .iter()
                                    .find(|p| &p.id == pid)
                                    .map(|proj| {
                                        autocode_core::session_storage::session_exists(proj, s)
                                    })
                                    .unwrap_or(true)
                        })
                        .map(|s| (s.id.clone(), s.label.clone()))
                        .collect();
                    let active_label = state
                        .active_session()
                        .map(|s| s.label.clone())
                        .unwrap_or_else(|| "Select Session".into());
                    egui::ComboBox::from_id_salt("session_picker")
                        .selected_text(
                            RichText::new(&active_label)
                                .size(11.0)
                                .color(Palette::TEXT_MUTED),
                        )
                        .show_ui(ui, |ui| {
                            let has_active = state.active_session_id.is_some();
                            if ui
                                .selectable_label(!has_active, "Select Session")
                                .clicked()
                                && has_active
                            {
                                state.active_session_id = None;
                            }
                            for (sid, slabel) in &sessions_here {
                                ui.push_id(("sess_sel", sid), |ui| {
                                    let selected =
                                        state.active_session_id.as_deref() == Some(sid);
                                    if ui.selectable_label(selected, slabel).clicked()
                                        && !selected
                                    {
                                        if let Some(sess) =
                                            state.sessions.iter_mut().find(|s| s.id == *sid)
                                        {
                                            sess.closed = false;
                                        }
                                        state.active_session_id = Some(sid.clone());
                                    }
                                });
                            }
                        });
                    // New session button next to the session picker.
                    if lit_btn(ui, "+ Session", false).clicked() {
                        state.new_session_for_project(Some(pid.clone()));
                        session::ensure_session(state);
                    }
                }

                helpers::toolbar_separator(ui);

                // -- Provider picker ------------------------------------
                egui::ComboBox::from_id_salt("provider_picker")
                    .selected_text(
                        RichText::new(&state.active_provider)
                            .size(11.0)
                            .color(Palette::TEXT_MUTED),
                    )
                    .show_ui(ui, |ui| {
                        let any_enabled = state.providers.values().any(|p| p.enabled);
                        let keys: Vec<String> = if any_enabled {
                            state.providers.iter()
                                .filter(|(_, p)| p.enabled)
                                .map(|(k, _)| k.clone())
                                .collect()
                        } else {
                            state.providers.keys().cloned().collect()
                        };
                        for key in keys {
                            ui.push_id(("prov_sel", key.clone()), |ui| {
                                let sel = state.active_provider == key;
                                if ui.selectable_label(sel, &key).clicked() {
                                    let model = state
                                        .providers
                                        .get(&key)
                                        .map(|p| p.model.clone())
                                        .unwrap_or_default();
                                    state.active_provider = key.clone();
                                    if let Some(sess) = state.active_session_mut() {
                                        sess.provider_label = key;
                                        sess.model = model;
                                    }
                                }
                            });
                        }
                    });

                // -- Model picker --------------------------------------
                let current_model = state
                    .active_provider()
                    .map(|p| p.model.clone())
                    .unwrap_or_default();

                egui::ComboBox::from_id_salt("model_picker")
                    .selected_text(
                        RichText::new(&current_model)
                            .size(11.0)
                            .color(Palette::TEXT_MUTED),
                    )
                    .show_ui(ui, |ui| {
                        let manifest_models: Vec<String> = state
                            .active_provider()
                            .and_then(|p| autocode_core::state::provider_manifest(&p.kind))
                            .map(|m| {
                                let mut keys: Vec<String> = m.models.keys().cloned().collect();
                                keys.sort();
                                keys
                            })
                            .unwrap_or_default();

                        let mut all_models = manifest_models;
                        if !current_model.is_empty() && !all_models.contains(&current_model) {
                            all_models.insert(0, current_model.clone());
                        }

                        for model_id in &all_models {
                            ui.push_id(("model_sel", model_id.clone()), |ui| {
                                let sel = current_model == *model_id;
                                if ui.selectable_label(sel, model_id).clicked() {
                                    if let Some(prov) =
                                        state.providers.get_mut(&state.active_provider)
                                    {
                                        prov.model.clone_from(model_id);
                                        prov.fill_from_manifest();
                                    }
                                    if let Some(sess) = state.active_session_mut() {
                                        sess.model.clone_from(model_id);
                                    }
                                }
                            });
                        }
                    });

                helpers::toolbar_separator(ui);

                // -- Context budget meter ------------------------------
                let frac = core_helpers::budget_fraction(state).clamp(0.0, 1.0);
                show_token_meter(ui, state, frac);

                // -- Network status indicator -------------------------
                let active_sid = state.active_session_id.clone();
                if let Some(runtime) = active_sid.as_ref().and_then(|sid| runtimes.get_mut(sid)) {
                    show_network_status(ui, &mut runtime.net_status);
                } else {
                    let mut net = autocode_ai::chat::NetworkStatus::default();
                    show_network_status(ui, &mut net);
                }

                // -- Right-side actions --------------------------------
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Settings (lights up when settings window is open).
                    if lit_btn(ui, "Settings", state.settings_open).clicked() {
                        state.settings_open = !state.settings_open;
                    }

                    // Explorer toggle (lights up when explorer is open).
                    if lit_btn(ui, "Files", state.show_explorer).clicked() {
                        state.show_explorer = !state.show_explorer;
                    }

                    // Handoff toggle (lights up when enabled).
                    show_handoff_toggle(ui, state);
                });
            });
        });
}

fn show_token_meter(ui: &mut egui::Ui, state: &AppState, frac: f32) {
    let meter_w = 88.0;
    let meter_h = 6.0;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(meter_w, meter_h), Sense::hover());
    let painter = ui.painter();

    // Track.
    painter.rect_filled(rect, 3.0, Palette::BG_SURFACE);

    // Fill.
    let fill_color = if frac > 0.85 {
        Palette::ERROR
    } else if frac > 0.65 {
        Palette::WARNING
    } else {
        Palette::SUCCESS
    };
    let fill_rect =
        egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * frac, rect.height()));
    painter.rect_filled(fill_rect, 3.0, fill_color);

    // Outline.
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, Palette::BORDER),
        StrokeKind::Outside,
    );

    resp.on_hover_text(format!("{:.0}% context used", frac * 100.0));

    ui.add_space(4.0);
    ui.label(
        RichText::new(core_helpers::usage_display(state))
            .size(10.0)
            .color(Palette::TEXT_MUTED),
    );
}

fn show_network_status(ui: &mut egui::Ui, net: &mut autocode_ai::chat::NetworkStatus) {
    let (dot, dot_color) = net.blink_dot();
    let byte_str = net.format_bytes();

    let show = net.active || !byte_str.is_empty();

    helpers::toolbar_separator(ui);

    let dot_color = if show { dot_color } else { Palette::BG_BASE };
    let dot_text = RichText::new(dot.to_string())
        .size(10.0)
        .color(dot_color)
        .monospace();
    let dot_resp = ui.add_sized(
        egui::Vec2::new(14.0, 20.0),
        egui::Label::new(dot_text).sense(egui::Sense::hover()),
    );
    if dot_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if show {
        if net.active {
            let tip = match net.idle_secs {
                Some(s) => format!(
                    "Stream idle: {}s\nStall timeout at your Stream Idle setting (Settings)",
                    s
                ),
                None => "Waiting for response...".to_string(),
            };
            dot_resp.on_hover_text(tip);
        } else if net.stalled {
            dot_resp.on_hover_text("Connection stalled");
        }
    }

    if !byte_str.is_empty() {
        ui.label(RichText::new(byte_str).size(10.0).color(if net.stalled {
            Palette::ERROR
        } else {
            Palette::TEXT_MUTED
        }));
    } else {
        ui.label(RichText::new(" ").size(10.0));
    }
}

fn lit_btn(ui: &mut egui::Ui, label: &str, lit: bool) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(if lit {
            Palette::ACCENT
        } else {
            Palette::TEXT_MUTED
        }))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(
            1.0,
            if lit {
                Palette::ACCENT
            } else {
                Palette::BORDER
            },
        )),
    )
}

fn show_handoff_toggle(ui: &mut egui::Ui, state: &mut AppState) {
    let enabled = state.handoff_enabled;
    let resp = lit_btn(ui, "Handoff", enabled);
    if resp.clicked() {
        state.handoff_enabled = !enabled;
    }
    resp.on_hover_text(if enabled {
        "Handoff enabled — agent can call `handoff` to start a fresh session"
    } else {
        "Handoff disabled — context will fill until manual intervention"
    });
}
