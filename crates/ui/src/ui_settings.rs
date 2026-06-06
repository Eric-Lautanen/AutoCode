// ui_settings.rs -- Settings window.
// Tabs: Providers * Projects * Prompt * Timeouts * Design * About

use autocode_ai::provider;
use autocode_core::{
    state::{AppState, Project, Session},
    theme::{Palette, ROUND_MD, ROUND_SM},
};
use crate::helpers;
use egui::{
    CollapsingHeader, Color32, CornerRadius, Frame, Grid, Margin, RichText, ScrollArea, Stroke,
    TextEdit, Vec2,
};

// -- State ---------------------------------------------------------------------

#[derive(Default, PartialEq)]
enum Tab {
    #[default]
    Providers,
    Projects,
    Prompt,
    Session,
    Timeouts,
    Design,
    About,
}

#[derive(Default)]
pub struct SettingsState {
    tab: Tab,
    fetched_models: HashMap<String, Vec<String>>,
    fetch_status: HashMap<String, String>,
}

use std::collections::HashMap;

// -- Window --------------------------------------------------------------------

pub fn show_window(ctx: &egui::Context, state: &mut AppState, settings: &mut SettingsState) {
    if !state.settings_open {
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

    egui::Window::new("Settings")
        .title_bar(false)
        .open(&mut open)
        .resizable(true)
        .default_size([700.0, 720.0])
        .min_size([480.0, 340.0])
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
                                state.tool_timeout_secs = autocode_core::helpers::default_tool_timeout();
                                state.shell_timeout_secs = autocode_core::helpers::default_shell_timeout();
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
                        tab_btn(ui, &mut settings.tab, Tab::Design, "Design");
                        tab_btn(ui, &mut settings.tab, Tab::About, "About");
                    });
                });

            ui.add_space(4.0);

            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    Frame::NONE
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| match settings.tab {
                            Tab::Providers => show_providers(ui, state, settings),
                            Tab::Projects => show_projects(ui, state),
                            Tab::Prompt => show_prompt(ui, state),
                            Tab::Session => show_session_settings(ui, state),
                            Tab::Timeouts => show_timeouts(ui, state),
                            Tab::Design => show_design(ui, state),
                            Tab::About => show_about(ui, state),
                        });
                });
        });

    if request_close {
        state.settings_open = false;
    }
    if !state.settings_open {
        // Notify the chat input that a popup just closed so it can reclaim focus.
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("popup_just_closed"), true));
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

// -- Providers -----------------------------------------------------------------

fn show_providers(ui: &mut egui::Ui, state: &mut AppState, settings: &mut SettingsState) {
    helpers::section_heading(ui, "API Providers");

    ui.add_space(4.0);
    if ui.button("+ Add Provider").clicked() {
        let kind = autocode_core::state::ProviderKind::new("openai-compatible");
        let base = kind.label().to_string();
        let mut key = base.clone();
        let mut n = 2;
        while state.providers.contains_key(&key) {
            key = format!("{} {}", base, n);
            n += 1;
        }
        state
            .providers
            .insert(key, autocode_core::state::ApiProvider::new(kind));
    }
    ui.add_space(8.0);

    let mut keys: Vec<String> = state.providers.keys().cloned().collect();
    // Active provider always first.
    keys.sort_by(|a, b| {
        let a_active = state.active_provider == *a;
        let b_active = state.active_provider == *b;
        match (a_active, b_active) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.cmp(b),
        }
    });

    let mut to_remove: Vec<String> = Vec::new();
    let mut set_active_key: Option<(String, String)> = None;

    for key in keys {
        ui.push_id(("provider", &key), |ui| {
        let is_active = state.active_provider == key;
        let p = match state.providers.get_mut(&key) {
            Some(p) => p,
            None => return,
        };

        let border_color = if is_active {
            Palette::ACCENT
        } else {
            Palette::BORDER
        };
        let bg_color = if is_active {
            Palette::BG_ACTIVE
        } else {
            Palette::BG_SURFACE
        };
        Frame::NONE
            .fill(bg_color)
            .corner_radius(ROUND_MD)
            .stroke(egui::Stroke::new(1.0, border_color))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                // Provider header row.
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(p.kind.label())
                            .size(13.0)
                            .color(Palette::TEXT_PRIMARY)
                            .strong(),
                    );
                    if is_active {
                        ui.label(RichText::new("Active").size(10.0).color(Palette::SUCCESS));
                    } else if ui.small_button("Set Active").clicked() {
                        set_active_key = Some((key.clone(), p.model.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("Reset")
                            .on_hover_text("Reset all provider settings to defaults")
                            .clicked()
                        {
                            p.reset_defaults();
                        }
                        if !is_active
                            && ui
                                .small_button("Remove")
                                .on_hover_text("Remove this provider")
                                .clicked()
                        {
                            to_remove.push(key.clone());
                        }
                        let enabled_label = if p.enabled { "Enabled" } else { "Disabled" };
                        let enabled_color = if p.enabled {
                            Palette::SUCCESS
                        } else {
                            Palette::TEXT_MUTED
                        };
                        if ui
                            .button(RichText::new(enabled_label).size(11.0).color(enabled_color))
                            .clicked()
                        {
                            p.enabled = !p.enabled;
                        }
                    });
                });

                ui.add_space(8.0);

                // Fields in a 2-column grid for alignment.
                Grid::new(format!("provider_grid_{}", key))
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        // API Key.
                        ui.label(helpers::field_label("API Key"));
                        let mut key_buf = p.api_key.clone_inner();
                        if ui
                            .add(
                                TextEdit::singleline(&mut key_buf)
                                    .id(egui::Id::new(("provider_api_key", &key)))
                                    .password(true)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("sk-..."),
                            )
                            .changed()
                        {
                            p.api_key = autocode_core::state::SecretString::new(key_buf);
                        }
                        ui.end_row();

                        // Base URL.
                        ui.label(helpers::field_label("Base URL"));
                        let mut url = p.base_url.clone();
                        if ui
                            .add(
                                TextEdit::singleline(&mut url)
                                    .id(egui::Id::new(("provider_base_url", &key)))
                                    .desired_width(f32::INFINITY),
                            )
                            .changed()
                        {
                            p.base_url = url;
                        }
                        ui.end_row();

                        // Model.
                        ui.label(helpers::field_label("Model"));
                        ui.horizontal(|ui| {
                            let mut model = p.model.clone();
                            let input_w = ui.available_width() - 100.0;
                            if ui
                                .add(
                                    TextEdit::singleline(&mut model)
                                        .id(egui::Id::new(("provider_model", &key)))
                                        .desired_width(input_w),
                                )
                                .changed()
                            {
                                p.model = model;
                            }
                            let p_clone = p.clone();
                            let key_for_fetch = key.clone();
                            if ui
                                .button("Fetch")
                                .on_hover_text("Fetch available models for this provider")
                                .clicked()
                            {
                                let models = provider::fetch_models(&p_clone);
                                let status = format!("{} models", models.len());
                                settings
                                    .fetched_models
                                    .insert(key_for_fetch.clone(), models);
                                settings.fetch_status.insert(key_for_fetch, status);
                            }
                        });
                        ui.end_row();
                        // Fetched model dropdown — separate row so it never pushes
                        // the window wider.
                        if let Some(models) = settings.fetched_models.get(&key)
                            && !models.is_empty()
                        {
                            ui.label("");
                            ui.horizontal(|ui| {
                                let current_model = p.model.clone();
                                ui.set_max_width(260.0);
                                egui::ComboBox::from_id_salt(format!("model_list_{}", key))
                                    .selected_text(&current_model)
                                    .width(ui.available_width())
                                    .show_ui(ui, |ui| {
                                        for m in models.iter() {
                                            ui.push_id(("model_sel", m), |ui| {
                                                if ui.selectable_label(*m == current_model, m).clicked()
                                                {
                                                    p.model = m.clone();
                                                }
                                            });
                                        }
                                    });
                                if let Some(status) = settings.fetch_status.get(&key) {
                                    ui.label(
                                        RichText::new(status).size(10.0).color(Palette::TEXT_MUTED),
                                    );
                                }
                            });
                            ui.end_row();
                        }
                        ui.end_row();

                        // Context window (max tokens).
                        ui.label(helpers::field_label("Context Window"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut p.max_context_tokens)
                                    .speed(1000.0)
                                    .range(4_000..=2_000_000),
                            );
                            ui.label(
                                RichText::new("tokens")
                                    .size(10.5)
                                    .color(Palette::TEXT_MUTED),
                            );
                        });
                        ui.end_row();

                        // Thinking API style.
                        ui.label(helpers::field_label("Thinking API"));
                        {
                            let mut current = p.thinking_api.clone();
                            egui::ComboBox::from_id_salt(format!("thinking_api_{}", key))
                                .selected_text(current.label())
                                .show_ui(ui, |ui| {
                                for api in autocode_core::state::ThinkingApi::variants() {
                                    ui.push_id(("thinking_sel", api.label()), |ui| {
                                        if ui
                                            .selectable_label(current == *api, api.label())
                                            .clicked()
                                        {
                                            current = api.clone();
                                        }
                                    });
                                }
                                });
                            if current != p.thinking_api {
                                p.thinking_api = current;
                            }
                        }
                        ui.end_row();

                        // Handoff percentage.
                        ui.label(helpers::field_label("Handoff"));
                        ui.horizontal(|ui| {
                            let mut pct = p.handoff_percent as f32;
                            ui.add(
                                egui::Slider::new(&mut pct, 10.0..=95.0)
                                    .step_by(5.0)
                                    .show_value(false)
                                    .trailing_fill(true),
                            );
                            let new_pct = pct as u8;
                            p.handoff_percent = new_pct;
                            ui.label(
                                RichText::new(format!("{}%", new_pct))
                                    .size(11.0)
                                    .color(Palette::ACCENT)
                                    .strong(),
                            );
                            let threshold = (p.max_context_tokens as u64 * new_pct as u64) / 100;
                            let threshold_fmt = if threshold >= 1_000_000 {
                                format!("{:.1}M", threshold as f64 / 1_000_000.0)
                            } else if threshold >= 1_000 {
                                format!("{:.1}K", threshold as f64 / 1_000.0)
                            } else {
                                threshold.to_string()
                            };
                            ui.label(
                                RichText::new(format!("@{} tokens", threshold_fmt))
                                    .size(10.5)
                                    .color(Palette::TEXT_MUTED),
                            );
                        });
                        ui.end_row();

                        // Allow project escape (access files outside project root).
                        ui.label(helpers::field_label("Allow Outside Access"));
                        ui.horizontal(|ui| {
                            let mut val = p.allow_project_escape;
                            if ui
                                .checkbox(&mut val, "")
                                .on_hover_text(
                                    "When enabled, the AI can read/list/grep files \
                                     anywhere on disk, not just inside the project folder. \
                                     Write operations are still restricted to the project root \
                                     unless you also disable the write-path check.",
                                )
                                .changed()
                            {
                                p.allow_project_escape = val;
                            }
                            if val {
                                ui.label(
                                    RichText::new("Enabled")
                                        .size(11.0)
                                        .color(Palette::WARNING)
                                        .strong(),
                                );
                            } else {
                                ui.label(
                                    RichText::new("Restricted to project")
                                        .size(11.0)
                                        .color(Palette::TEXT_MUTED),
                                );
                            }
                        });
                        ui.end_row();

                        // Max output tokens.
                        ui.label(helpers::field_label("Max Output"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut p.max_output_tokens)
                                    .speed(1000.0)
                                    .range(256..=200_000),
                            );
                            ui.label(
                                RichText::new("tokens")
                                    .size(10.5)
                                    .color(Palette::TEXT_MUTED),
                            );
                        });
                        ui.end_row();

                        // Max output tokens when thinking is enabled.
                        ui.label(helpers::field_label("Max Output (Thinking)"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut p.max_output_tokens_thinking)
                                    .speed(1000.0)
                                    .range(256..=200_000),
                            );
                            ui.label(
                                RichText::new("tokens")
                                    .size(10.5)
                                    .color(Palette::TEXT_MUTED),
                            );
                        });
                        ui.end_row();
                    });
            });

        ui.add_space(8.0);
        }); // end push_id("provider", key)
    }

    for key in to_remove {
        state.providers.remove(&key);
        if state.active_provider == key {
            state.active_provider = state.providers.keys().next().cloned().unwrap_or_default();
        }
    }
    if let Some((label, model)) = set_active_key {
        state.active_provider = label.clone();
        if let Some(sess) = state.active_session_mut() {
            sess.provider_label = label;
            sess.model = model;
        }
    }
}

// -- Projects ------------------------------------------------------------------

fn show_projects(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "Projects");

    let projects: Vec<Project> = state.projects.clone();
    let mut to_remove: Option<String> = None;

    let mut rename_ops: Vec<(String, String)> = Vec::new();
    let mut delete_ops: Vec<String> = Vec::new();
    let mut delete_all_for_project: Option<String> = None;

    for p in &projects {
        ui.push_id(("project", &p.id), |ui| {
        let is_active = state.active_project_id.as_deref() == Some(&p.id);
        let border_color = if is_active {
            Palette::ACCENT_DIM
        } else {
            Palette::BORDER
        };
        let bg_color = if is_active {
            Palette::BG_ACTIVE
        } else {
            Palette::BG_SURFACE
        };

        Frame::NONE
            .fill(bg_color)
            .corner_radius(ROUND_SM)
            .stroke(egui::Stroke::new(1.0, border_color))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&p.name)
                                .size(12.5)
                                .color(Palette::TEXT_PRIMARY)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(&p.root_path)
                                .size(10.5)
                                .color(Palette::TEXT_MUTED)
                                .monospace(),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Remove").size(11.0).color(Palette::ERROR))
                            .clicked()
                        {
                            to_remove = Some(p.id.clone());
                        }
                        if !is_active && ui.button("Set Active").clicked() {
                            autocode_core::session_storage::switch_to_project(state, &p.id);
                        }
                        if is_active {
                            ui.label(RichText::new("Active").size(10.0).color(Palette::SUCCESS));
                        }
                    });
                });

                // Collapsible session list for this project.
                let proj_sessions: Vec<&Session> = state
                    .sessions
                    .iter()
                    .filter(|s| {
                        s.project_id.as_deref() == Some(&p.id)
                            && autocode_core::session_storage::session_exists(&p, s)
                    })
                    .collect();
                if !proj_sessions.is_empty() {
                    CollapsingHeader::new(format!("Sessions ({})", proj_sessions.len()))
                        .id_salt(format!("settings_proj_sessions_{}", p.id))
                        .show(ui, |ui| {
                            for sess in proj_sessions {
                                ui.push_id(("session", &sess.id), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&sess.id)
                                            .monospace()
                                            .size(11.0),
                                    );
                                    let mut label_buf = sess.label.clone();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut label_buf)
                                            .id(egui::Id::new(("session_label", &sess.id)))
                                            .desired_width(180.0),
                                    );
                                    if resp.lost_focus() && label_buf != sess.label {
                                        rename_ops.push((sess.id.clone(), label_buf));
                                    }
                                    if ui.button("Delete").clicked() {
                                        delete_ops.push(sess.id.clone());
                                    }
                                });
                                }); // end push_id("session", id)
                            }
                            ui.add_space(4.0);
                            if ui.button("Delete All Sessions").clicked() {
                                delete_all_for_project = Some(p.id.clone());
                            }
                        });
                }
            });

        ui.add_space(4.0);
        }); // end push_id("project", id)
    }

    // Apply collected ops.
    for (sid, new_label) in rename_ops {
        if let Some(s) = state.sessions.iter_mut().find(|s| s.id == sid) {
            s.label = new_label;
            if let Some(proj) = state
                .projects
                .iter()
                .find(|p| Some(&p.id) == s.project_id.as_ref())
            {
                let _ = autocode_core::session_storage::save_session(proj, s);
            }
        }
    }
    for sid in delete_ops {
        autocode_ai::session::delete_session(state, &sid);
    }
    if let Some(pid) = delete_all_for_project {
        let ids: Vec<String> = state
            .sessions
            .iter()
            .filter(|s| s.project_id.as_deref() == Some(&pid))
            .map(|s| s.id.clone())
            .collect();
        for sid in ids {
            autocode_ai::session::delete_session(state, &sid);
        }
    }
    if let Some(id) = to_remove {
        let sess_ids: Vec<String> = state
            .sessions
            .iter()
            .filter(|s| s.project_id.as_deref() == Some(&id))
            .map(|s| s.id.clone())
            .collect();
        let proj_dir = state
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(autocode_core::session_storage::project_sessions_dir);
        for sid in sess_ids {
            autocode_ai::session::delete_session(state, &sid);
        }
        if let Some(dir) = proj_dir {
            let _ = autocode_core::fsutil::remove_dir(&dir);
        }
        state.projects.retain(|p| p.id != id);
        if state.active_project_id.as_deref() == Some(&id) {
            state.active_project_id = state.projects.last().map(|p| p.id.clone());
            state.active_session_id = None;
        }
    }
}

// -- Session Settings ----------------------------------------------------------

fn show_session_settings(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "Session Settings");

    ui.label(
        RichText::new("Control how many messages are kept in memory and rendered.")
            .size(11.0)
            .color(Palette::TEXT_MUTED),
    );
    ui.add_space(10.0);

    Frame::NONE
        .fill(Palette::BG_SURFACE)
        .corner_radius(ROUND_MD)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            Grid::new("session_settings_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .min_col_width(180.0)
                .show(ui, |ui| {
                    ui.label(helpers::field_label("Messages in RAM"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.ui_display_window)
                                .speed(5.0)
                                .range(10..=500),
                        );
                        ui.label(
                            RichText::new("messages")
                                .size(10.5)
                                .color(Palette::TEXT_MUTED),
                        );
                    });
                    ui.end_row();

                    ui.label(helpers::field_label("Completion Delay"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.disk_read_delay_ms)
                                .speed(10.0)
                                .range(50..=5000),
                        );
                        ui.label(
                            RichText::new("ms")
                                .size(10.5)
                                .color(Palette::TEXT_MUTED),
                        );
                    });
                    ui.end_row();
                });

            ui.add_space(6.0);
            ui.label(
                RichText::new(
                     "Messages in RAM: controls how many are held in memory and displayed. \
                     Full history is saved to disk and reloaded for API requests. \
                     Completion Delay: minimum pause (ms) between consecutive API calls \
                     to pace rapid tool-use loops.",
                )
                .size(10.0)
                .color(Palette::TEXT_MUTED),
            );
        });
}

// -- Prompt --------------------------------------------------------------------

fn show_prompt(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "System Prompt");

    ui.label(
        RichText::new("Injected as the first message of every new session.")
            .size(11.0)
            .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.system_prompt)
            .desired_rows(20)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("Reset to Default").clicked() {
            state.system_prompt = autocode_core::state::DEFAULT_SYSTEM_PROMPT.to_string();
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    // -- Handoff prompt -------------------------------------------------
    ui.label(
        RichText::new("Handoff Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Sent as the first user message in a fresh session after a handoff. Instructs the agent to read RESUME.md and continue work.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.handoff_prompt)
            .desired_rows(6)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.handoff_prompt = autocode_core::state::DEFAULT_HANDOFF_PROMPT.to_string();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);
}

// -- Timeouts ------------------------------------------------------------------

fn show_timeouts(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "Timeouts");

    ui.label(
        RichText::new(
            "Adjust timeouts to match your model and network conditions. \
             Slower models or high-latency networks may need higher values \
             to avoid premature retries and aborted responses.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(10.0);

    // --- API / streaming timeouts ---

    Frame::NONE
        .fill(Palette::BG_SURFACE)
        .corner_radius(ROUND_MD)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.label(
                RichText::new("API & Streaming")
                    .size(12.0)
                    .color(Palette::TEXT_SECONDARY)
                    .strong(),
            );
            ui.add_space(6.0);

            Grid::new("timeouts_api_grid")
                .num_columns(3)
                .spacing([12.0, 6.0])
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(helpers::field_label("Stream Idle"));
                    ui.add(
                        egui::DragValue::new(&mut state.stream_idle_timeout_secs)
                            .speed(5.0)
                            .range(10..=600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Request Max"));
                    ui.add(
                        egui::DragValue::new(&mut state.request_timeout_secs)
                            .speed(10.0)
                            .range(30..=1800),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Tool Timeout"));
                    ui.add(
                        egui::DragValue::new(&mut state.tool_timeout_secs)
                            .speed(5.0)
                            .range(10..=600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Max Retries"));
                    ui.add(
                        egui::DragValue::new(&mut state.max_retries)
                            .speed(1.0)
                            .range(0..=10),
                    );
                    ui.label(RichText::new("").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Retry Wait Cap"));
                    ui.add(
                        egui::DragValue::new(&mut state.max_retry_wait_secs)
                            .speed(30.0)
                            .range(30..=3600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();
                });

            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Stream Idle: seconds with no SSE delta before aborting. \
                     Request Max: absolute timeout for HTTPS calls. \
                     Tool Timeout: per-operation limit for file/glob/todo tools.",
                )
                .size(10.0)
                .color(Palette::TEXT_MUTED),
            );
        });

    ui.add_space(12.0);

    // --- Shell command timeouts ---

    Frame::NONE
        .fill(Palette::BG_SURFACE)
        .corner_radius(ROUND_MD)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Shell Commands")
                    .size(12.0)
                    .color(Palette::TEXT_SECONDARY)
                    .strong(),
            );
            ui.add_space(6.0);

            Grid::new("timeouts_shell_grid")
                .num_columns(3)
                .spacing([12.0, 6.0])
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(helpers::field_label("Default Timeout"));
                    ui.add(
                        egui::DragValue::new(&mut state.shell_timeout_secs)
                            .speed(10.0)
                            .range(10..=state.shell_timeout_max_secs),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Maximum Timeout"));
                    ui.add(
                        egui::DragValue::new(&mut state.shell_timeout_max_secs)
                            .speed(30.0)
                            .range(60..=3600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();
                });

            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "The model can request a custom timeout per command, \
                     capped by the Maximum. Default is used when it doesn't specify one.",
                )
                .size(10.0)
                .color(Palette::TEXT_MUTED),
            );
        });

    ui.add_space(12.0);

    if ui.button("Reset to Defaults").clicked() {
        state.stream_idle_timeout_secs = autocode_core::helpers::default_stream_idle_timeout();
        state.request_timeout_secs = autocode_core::helpers::default_request_timeout();
        state.shell_timeout_secs = autocode_core::helpers::default_shell_timeout();
        state.shell_timeout_max_secs = autocode_core::helpers::default_shell_timeout_max();
        state.max_retries = autocode_core::helpers::default_max_retries();
        state.max_retry_wait_secs = autocode_core::helpers::default_max_retry_wait();
    }
}

// -- Design ---------------------------------------------------------------------

macro_rules! color_row {
    ($ui:expr, $target:expr, $d:expr, $label:expr, $field:ident) => {
        $ui.label(helpers::field_label($label));
        $ui.horizontal(|ui| {
            ui.color_edit_button_rgb(&mut $d.$field);
            dropper_btn(ui, $target, stringify!($field));
        });
        $ui.end_row();
    };
}

fn show_design(ui: &mut egui::Ui, state: &mut AppState) {
    ui.add_space(4.0);
    // Global tooltip explaining sampling mode.
    if state.sampling_target.is_some() {
        let pos = ui.ctx().viewport_rect().center() - egui::Vec2::new(150.0, 0.0);
        egui::Area::new(egui::Id::new("sampling_tooltip"))
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                Frame::NONE
                    .fill(Color32::from_rgba_premultiplied(0, 0, 0, 200))
                    .corner_radius(ROUND_SM)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("[ Sampling click anywhere to pick a color ]")
                                .size(13.0)
                                .color(Palette::ACCENT),
                        );
                    });
            });
        ui.ctx().request_repaint();
    }
    let d = &mut state.design;
    let target = &mut state.sampling_target;

    ScrollArea::vertical()
        .id_salt("design_scroll")
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.x = 8.0;

            let label_w = 160.0;

            // --- Colors: Bubbles ---
            helpers::section_heading(ui, "Bubble Colors");
            Grid::new("design_bubble_colors")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "User Fill", user_bubble_fill);
                    color_row!(ui, target, d, "User Stroke", user_bubble_stroke);
                    color_row!(ui, target, d, "Tool Fill", tool_bubble_fill);
                    color_row!(ui, target, d, "Tool Stroke", tool_bubble_stroke);
                    color_row!(ui, target, d, "Assistant Fill", assist_bubble_fill);
                    color_row!(ui, target, d, "Assistant Stroke", assist_bubble_stroke);
                    color_row!(ui, target, d, "Error Fill", error_notice_fill);
                    color_row!(ui, target, d, "Error Stroke", error_notice_stroke);
                    color_row!(ui, target, d, "System Pill Fill", system_pill_fill);
                    color_row!(ui, target, d, "System Pill Stroke", system_pill_stroke);
                    color_row!(ui, target, d, "Streaming Fill", stream_fill);
                    color_row!(ui, target, d, "Streaming Stroke", stream_stroke);
                    color_row!(ui, target, d, "Streaming Cursor", stream_cursor);
                    color_row!(ui, target, d, "Waiting Fill", waiting_fill);
                    color_row!(ui, target, d, "Waiting Stroke", waiting_stroke);
                });

            // --- Colors: Terminal ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Terminal Colors");
            Grid::new("design_terminal")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Background", terminal_bg);
                    color_row!(ui, target, d, "Text Color", terminal_text);
                    color_row!(ui, target, d, "Border", terminal_border);
                    color_row!(ui, target, d, "Live Background", live_terminal_bg);
                    color_row!(ui, target, d, "Live Border", live_terminal_border);
                    color_row!(ui, target, d, "Label Text", terminal_label_color);
                });

            // --- Colors: Code Blocks ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Code Block Colors");
            Grid::new("design_code")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Background", code_frame_bg);
                    color_row!(ui, target, d, "Text Color", code_text);
                    color_row!(ui, target, d, "Label Text", code_label_color);
                });

            // --- Colors: Diff ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Diff Colors");
            Grid::new("design_diff")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Frame Background", diff_frame_bg);
                    color_row!(ui, target, d, "Delete Background", diff_del_bg);
                    color_row!(ui, target, d, "Delete Text", diff_del_text);
                    color_row!(ui, target, d, "Add Background", diff_add_bg);
                    color_row!(ui, target, d, "Add Text", diff_add_text);
                    color_row!(ui, target, d, "Context Background", diff_ctx_bg);
                    color_row!(ui, target, d, "Context Text", diff_ctx_text);
                    color_row!(ui, target, d, "Line Number", diff_num_color);
                    color_row!(ui, target, d, "Label Text", diff_label_color);
                });

            // --- Colors: Reasoning ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Reasoning / Thinking Colors");
            Grid::new("design_reason")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Background", reason_bg);
                    color_row!(ui, target, d, "Border", reason_border);
                    color_row!(ui, target, d, "Header", reason_header);
                });

            // --- Colors: Badges ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Badge Colors");
            Grid::new("design_badges")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Assistant", assist_badge);
                    color_row!(ui, target, d, "Tool", tool_badge);
                    color_row!(ui, target, d, "User", user_badge);
                    color_row!(ui, target, d, "System", system_badge);
                });

            // --- Colors: Semantic / Text ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Semantic / Text Colors");
            Grid::new("design_tool_labels")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Success", success_color);
                    color_row!(ui, target, d, "Error", error_color);
                    color_row!(ui, target, d, "Accent", accent_color);
                    color_row!(ui, target, d, "Warning", warning_color);
                    color_row!(ui, target, d, "Muted", muted_color);
                    color_row!(ui, target, d, "Text Primary", text_primary);
                    color_row!(ui, target, d, "Text Secondary", text_secondary);
                });

            // --- Colors: Code Blocks ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Code Block Colors");
            Grid::new("design_code_1")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Background", code_frame_bg);
                    color_row!(ui, target, d, "Text Color", code_text);
                    color_row!(ui, target, d, "Label Text", code_label_color);
                });

            // --- Colors: Diff ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Diff Colors");
            Grid::new("design_diff_1")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Frame Background", diff_frame_bg);
                    color_row!(ui, target, d, "Delete Background", diff_del_bg);
                    color_row!(ui, target, d, "Delete Text", diff_del_text);
                    color_row!(ui, target, d, "Add Background", diff_add_bg);
                    color_row!(ui, target, d, "Add Text", diff_add_text);
                    color_row!(ui, target, d, "Context Background", diff_ctx_bg);
                    color_row!(ui, target, d, "Context Text", diff_ctx_text);
                    color_row!(ui, target, d, "Line Number", diff_num_color);
                    color_row!(ui, target, d, "Label Text", diff_label_color);
                });

            // --- Colors: Reasoning ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Reasoning / Thinking Colors");
            Grid::new("design_reason_1")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Background", reason_bg);
                    color_row!(ui, target, d, "Border", reason_border);
                    color_row!(ui, target, d, "Header Text", reason_header);
                });

            // --- Colors: Badges ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Badge Colors");
            Grid::new("design_badges_1")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Assistant Badge", assist_badge);
                    color_row!(ui, target, d, "Tool Badge", tool_badge);
                    color_row!(ui, target, d, "User Badge", user_badge);
                    color_row!(ui, target, d, "System Badge", system_badge);
                });

            // --- Colors: Tool Labels ---
            ui.add_space(8.0);
            helpers::section_heading(ui, "Tool Label & Text Colors");
            Grid::new("design_tool_labels_1")
                .num_columns(2)
                .spacing([12.0, 5.0])
                .min_col_width(label_w)
                .show(ui, |ui| {
                    color_row!(ui, target, d, "Success Color", success_color);
                    color_row!(ui, target, d, "Error Color", error_color);
                    color_row!(ui, target, d, "Accent Color", accent_color);
                    color_row!(ui, target, d, "Warning Color", warning_color);
                    color_row!(ui, target, d, "Muted Color", muted_color);
                    color_row!(ui, target, d, "Text Primary", text_primary);
                    color_row!(ui, target, d, "Text Secondary", text_secondary);
                });

            ui.add_space(8.0);
            if ui.button("Reset Design Defaults").clicked() {
                *d = Default::default();
            }
        });
}

// -- About ---------------------------------------------------------------------

fn show_about(ui: &mut egui::Ui, state: &mut AppState) {
    ui.add_space(4.0);
    ui.label(
        RichText::new("AutoCode")
            .size(18.0)
            .strong()
            .color(Palette::ACCENT),
    );
    ui.add_space(2.0);
    ui.label(
        RichText::new("Autonomous AI coding assistant")
            .size(13.0)
            .color(Palette::TEXT_SECONDARY),
    );
    ui.add_space(14.0);

    let info = [
        ("Version", "0.1.0"),
        ("UI", "egui 0.34 / eframe 0.34"),
        ("Language", "Rust -- serde, egui only"),
        (
            "Providers",
            "OpenRouter | NVIDIA NIM | OpenAI-compatible | OpenCode Go",
        ),
    ];

    Grid::new("about_grid")
        .num_columns(2)
        .spacing([24.0, 7.0])
        .show(ui, |ui| {
            for (k, v) in info {
                ui.label(RichText::new(k).size(12.0).color(Palette::TEXT_MUTED));
                ui.label(RichText::new(v).size(12.0).color(Palette::TEXT_PRIMARY));
                ui.end_row();
            }
        });

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    helpers::section_heading(ui, "System Information");
    ui.add_space(4.0);

    ui.checkbox(
        &mut state.debug_mode,
        "Debug hover (show widget IDs on hover)",
    )
    .on_hover_text("Shows widget ID, rect, and state info when you hover over UI elements.");
    ui.add_space(4.0);
    ui.checkbox(&mut state.inspection_open, "Inspection overlay (show all widget bounds)")
        .on_hover_text("Paints red bounding boxes around every widget and opens the inspection panel. The fancy one.");

    let sysinfo = &state.sysinfo;
    if sysinfo.report.is_empty() {
        ui.label(
            RichText::new("Detecting system info...")
                .size(12.0)
                .color(Palette::TEXT_MUTED)
                .italics(),
        );
    } else {
        Frame::NONE
            .fill(Palette::BG_SURFACE)
            .corner_radius(ROUND_SM)
            .stroke(egui::Stroke::new(1.0, Palette::BORDER))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&sysinfo.report)
                        .size(11.5)
                        .color(Palette::TEXT_SECONDARY)
                        .monospace(),
                );
            });
    }

    ui.add_space(8.0);

    if ui.button("Refresh System Info").clicked() {
        let rx = autocode_core::sysinfo::start_detect();
        let ctx = ui.ctx().clone();
        std::thread::spawn(move || {
            if let Ok(info) = rx.recv() {
                let _ = info;
            }
            ctx.request_repaint();
        });
        ui.ctx().data_mut(|d| {
            d.insert_temp(egui::Id::new("sysinfo_refresh_requested"), true);
        });
    }

    ui.label(
        RichText::new(
            "Run this after installing new tools (rg, python, etc.) to update the system prompt.",
        )
        .size(10.0)
        .color(Palette::TEXT_MUTED),
    );

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    // OpenGL / Renderer info.
    helpers::section_heading(ui, "Renderer");
    ui.add_space(4.0);
    if autocode_core::sysinfo::has_opengl() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("OpenGL").size(12.0).color(Palette::SUCCESS).strong());
            ui.label(RichText::new("(Glow backend)").size(11.0).color(Palette::TEXT_MUTED));
        });
    } else {
        Frame::NONE
            .fill(egui::Color32::from_rgba_premultiplied(80, 30, 30, 40))
            .corner_radius(ROUND_SM)
            .stroke(egui::Stroke::new(1.0, Palette::ERROR))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("OpenGL not found — using Wgpu fallback (higher RAM usage).")
                        .size(11.0)
                        .color(Palette::ERROR),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "AutoCode runs best with the Glow (OpenGL) renderer.\n\
                        Install Mesa/OpenGL drivers for your system:",
                    )
                    .size(11.0)
                    .color(Palette::TEXT_PRIMARY),
                );
                ui.add_space(4.0);
                if ui.link("OpenGL installation guide (docs.mesa3d.org)").clicked() {
                    #[cfg(target_os = "windows")]
                    { let _ = std::process::Command::new("cmd").args(["/c", "start", "https://docs.mesa3d.org/install.html"]).spawn(); }
                    #[cfg(target_os = "macos")]
                    { let _ = std::process::Command::new("open").arg("https://docs.mesa3d.org/install.html").spawn(); }
                    #[cfg(target_os = "linux")]
                    { let _ = std::process::Command::new("xdg-open").arg("https://docs.mesa3d.org/install.html").spawn(); }
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Debian/Ubuntu:  sudo apt install libgl1-mesa-glx\n\
                        Fedora:          sudo dnf install mesa-libGL\n\
                        Arch:            sudo pacman -S mesa",
                    )
                    .size(11.0)
                    .color(Palette::TEXT_MUTED)
                    .monospace(),
                );
            });
    }

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    Frame::NONE
        .fill(egui::Color32::from_rgba_premultiplied(80, 50, 20, 30))
        .corner_radius(ROUND_SM)
        .stroke(egui::Stroke::new(1.0, Palette::WARNING))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "AutoCode runs commands and writes files without confirmation.\n\
                    Review the active project directory before use in sensitive environments.",
                )
                .size(11.0)
                .color(Palette::WARNING),
            );
        });
}

// -- Eyedropper helpers ---------------------------------------------------------

/// Add an eyedropper button next to a color picker.
/// Sets `state.sampling_target` to `field_name` when clicked.
fn dropper_btn(ui: &mut egui::Ui, target: &mut Option<String>, field_name: &str) {
    let resp = ui.add(
        egui::Button::new(RichText::new("d").size(11.0).color(Palette::TEXT_MUTED))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, Palette::BORDER))
            .min_size(Vec2::new(20.0, 20.0)),
    );
    if resp.clicked() {
        *target = Some(field_name.to_string());
    }
    resp.on_hover_text("Sample color from screen");
}

/// Write a sampled color into the correct DesignSettings field.
pub fn apply_sampled_color(
    design: &mut autocode_core::state::DesignSettings,
    field: &str,
    color: [f32; 3],
) {
    match field {
        "user_bubble_fill" => design.user_bubble_fill = color,
        "user_bubble_stroke" => design.user_bubble_stroke = color,
        "tool_bubble_fill" => design.tool_bubble_fill = color,
        "tool_bubble_stroke" => design.tool_bubble_stroke = color,
        "assist_bubble_fill" => design.assist_bubble_fill = color,
        "assist_bubble_stroke" => design.assist_bubble_stroke = color,
        "error_notice_fill" => design.error_notice_fill = color,
        "error_notice_stroke" => design.error_notice_stroke = color,
        "system_pill_fill" => design.system_pill_fill = color,
        "system_pill_stroke" => design.system_pill_stroke = color,
        "stream_fill" => design.stream_fill = color,
        "stream_stroke" => design.stream_stroke = color,
        "stream_cursor" => design.stream_cursor = color,
        "waiting_fill" => design.waiting_fill = color,
        "waiting_stroke" => design.waiting_stroke = color,
        "terminal_bg" => design.terminal_bg = color,
        "terminal_text" => design.terminal_text = color,
        "terminal_border" => design.terminal_border = color,
        "live_terminal_bg" => design.live_terminal_bg = color,
        "live_terminal_border" => design.live_terminal_border = color,
        "terminal_label_color" => design.terminal_label_color = color,
        "code_frame_bg" => design.code_frame_bg = color,
        "code_text" => design.code_text = color,
        "code_label_color" => design.code_label_color = color,
        "diff_frame_bg" => design.diff_frame_bg = color,
        "diff_del_bg" => design.diff_del_bg = color,
        "diff_del_text" => design.diff_del_text = color,
        "diff_add_bg" => design.diff_add_bg = color,
        "diff_add_text" => design.diff_add_text = color,
        "diff_ctx_bg" => design.diff_ctx_bg = color,
        "diff_ctx_text" => design.diff_ctx_text = color,
        "diff_num_color" => design.diff_num_color = color,
        "diff_label_color" => design.diff_label_color = color,
        "reason_bg" => design.reason_bg = color,
        "reason_border" => design.reason_border = color,
        "reason_header" => design.reason_header = color,
        "assist_badge" => design.assist_badge = color,
        "tool_badge" => design.tool_badge = color,
        "user_badge" => design.user_badge = color,
        "system_badge" => design.system_badge = color,
        "success_color" => design.success_color = color,
        "error_color" => design.error_color = color,
        "accent_color" => design.accent_color = color,
        "warning_color" => design.warning_color = color,
        "muted_color" => design.muted_color = color,
        "text_primary" => design.text_primary = color,
        "text_secondary" => design.text_secondary = color,
        _ => {}
    }
}
