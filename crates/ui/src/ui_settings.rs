// ui_settings.rs -- Settings window.
// Tabs: Providers * Projects * Prompt * Timeouts * Design * About

use crate::helpers;
use autocode_ai::provider;
use autocode_core::{
    provider_file,
    state::{AppState, Project, Session, ThinkingApi},
    theme::{Palette, ROUND_MD, ROUND_SM},
};
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
    About,
}

#[derive(Default)]
pub struct SettingsState {
    tab: Tab,
    fetched_models: HashMap<String, Vec<String>>,
    fetch_status: HashMap<String, String>,
    /// If set, the provider with this key is being renamed.
    renaming_provider: Option<String>,
    /// Buffer for the rename text input.
    rename_buffer: String,
    /// Buffer for the new-provider name input when adding.
    add_buffer: String,
    /// When true, show an inline name input for adding a new provider.
    adding_provider: bool,
}

use std::collections::HashMap;

// -- Window --------------------------------------------------------------------

pub fn show_window(ctx: &egui::Context, state: &mut AppState, settings: &mut SettingsState) {
    if !state.settings_open {
        // Clean up stale debounce flag from a prior outside-click close.
        // egui insert_temp is persistent across frames, so we must clear it
        // here to avoid the first Settings button click being silently swallowed.
        ctx.data_mut(|d| {
            d.remove_temp::<bool>(egui::Id::new("settings_closed_this_frame"));
        });
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
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings_closed_this_frame"), true));
        let _ = provider_file::save_providers_file(&state.providers);
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
    let mut provider_dirty = false;

    if ui.button("+ Add Provider").clicked() {
        settings.adding_provider = true;
        settings.add_buffer = "My Provider".into();
    }
    if settings.adding_provider {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Name:")
                    .size(12.0)
                    .color(Palette::TEXT_SECONDARY),
            );
            ui.text_edit_singleline(&mut settings.add_buffer);
            let name_ok = !settings.add_buffer.is_empty()
                && !state.providers.contains_key(&settings.add_buffer);
            if ui
                .add_enabled(name_ok, egui::Button::new(RichText::new("Add").size(12.0)))
                .on_hover_text(if settings.add_buffer.is_empty() {
                    "Name cannot be empty"
                } else if state.providers.contains_key(&settings.add_buffer) {
                    "Name already exists"
                } else {
                    "Add this provider"
                })
                .clicked()
            {
                let kind = autocode_core::state::ProviderKind::new("openai-compatible");
                let key = std::mem::take(&mut settings.add_buffer);
                state
                    .providers
                    .insert(key, autocode_core::state::ApiProvider::new(kind));
                settings.adding_provider = false;
                provider_dirty = true;
            }
            if ui
                .button(
                    RichText::new("Cancel")
                        .size(12.0)
                        .color(Palette::TEXT_MUTED),
                )
                .clicked()
            {
                settings.adding_provider = false;
                settings.add_buffer.clear();
            }
        });
    }
    ui.add_space(8.0);

    let mut keys: Vec<String> = state.providers.keys().cloned().collect();
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
    let mut disable_switch_key: Option<String> = None;
    let mut pending_rename: Option<(String, String)> = None;

    let card_max_w = ui.available_width();
    for key in keys {
        ui.push_id(("provider", &key), |ui| {
            ui.set_max_width(card_max_w);
            let is_active = state.active_provider == key;

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
                    let p = state.providers.get_mut(&key).unwrap();
                    // ── Header ─────────────────────────────────────────
                    ui.horizontal(|ui| {
                        let is_renaming = settings.renaming_provider.as_deref() == Some(&key);
                        if is_renaming {
                            ui.text_edit_singleline(&mut settings.rename_buffer);
                            let name_ok = !settings.rename_buffer.is_empty()
                                && settings.rename_buffer != key;
                            if ui
                                .add_enabled(
                                    name_ok,
                                    egui::Button::new(RichText::new("Save").size(12.0)),
                                )
                                .on_disabled_hover_text(
                                    if settings.rename_buffer.is_empty() {
                                        "Name cannot be empty"
                                    } else {
                                        "Name is unchanged"
                                    },
                                )
                                .clicked()
                            {
                                let new_key = std::mem::take(&mut settings.rename_buffer);
                                let old_key = settings
                                    .renaming_provider
                                    .take()
                                    .unwrap_or_default();
                                pending_rename = Some((old_key, new_key));
                            }
                            if ui
                                .button(
                                    RichText::new("Cancel")
                                        .size(12.0)
                                        .color(Palette::TEXT_MUTED),
                                )
                                .clicked()
                            {
                                settings.renaming_provider = None;
                                settings.rename_buffer.clear();
                            }
                        } else {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&key)
                                        .size(13.0)
                                        .color(Palette::TEXT_PRIMARY)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(p.kind.label())
                                        .size(10.0)
                                        .color(Palette::TEXT_MUTED),
                                );
                            });
                            if ui
                                .small_button("✎")
                                .on_hover_text("Rename provider")
                                .clicked()
                            {
                                settings.renaming_provider = Some(key.clone());
                                settings.rename_buffer = key.clone();
                            }
                            if is_active {
                                ui.label(
                                    RichText::new("Active")
                                        .size(10.0)
                                        .color(Palette::SUCCESS),
                                );
                            } else if ui.small_button("Set Active").clicked() {
                                set_active_key = Some((key.clone(), p.model.clone()));
                            }
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
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
                                let enabled_label = if p.enabled {
                                    "Enabled"
                                } else {
                                    "Disabled"
                                };
                                let enabled_color = if p.enabled {
                                    Palette::SUCCESS
                                } else {
                                    Palette::TEXT_MUTED
                                };
                                if ui
                                    .button(
                                        RichText::new(enabled_label)
                                            .size(11.0)
                                            .color(enabled_color),
                                    )
                                    .clicked()
                                {
                                    p.enabled = !p.enabled;
                                    provider_dirty = true;
                                    if !p.enabled && is_active {
                                        disable_switch_key = Some(key.clone());
                                    }
                                }
                            },
                        );
                    });

                    ui.add_space(8.0);

                    CollapsingHeader::new("Settings")
                        .id_salt(format!("provider_body_{}", key))
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);

                    // ── Connection ─────────────────────────────────────
                    CollapsingHeader::new("Connection")
                        .id_salt(format!("conn_{}", key))
                        .default_open(true)
                        .show(ui, |ui| {
                            Grid::new(format!("conn_grid_{}", key))
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .min_col_width(60.0)
                                .show(ui, |ui| {
                                    ui.label(helpers::field_label("API Key"));
                                    let mut key_buf = p.api_key.clone_inner();
                                    let resp = ui.add(
                                            TextEdit::singleline(&mut key_buf)
                                                .id(egui::Id::new(("provider_api_key", &key)))
                                                .password(true)
                                                .desired_width(ui.available_width())
                                                .hint_text("sk-..."),
                                    );
                                    if resp.changed() {
                                        if !key_buf.is_empty() && !p.enabled {
                                            p.enabled = true;
                                        }
                                        p.api_key = autocode_core::state::SecretString::new(key_buf);
                                    }
                                    if resp.lost_focus() {
                                        provider_dirty = true;
                                    }
                                    ui.end_row();

                                    ui.label(helpers::field_label("Base URL"));
                                    let mut url = p.base_url.clone();
                                    if ui
                                        .add(
                                            TextEdit::singleline(&mut url)
                                                .id(egui::Id::new(("provider_base_url", &key)))
                                                .desired_width(ui.available_width()),
                                        )
                                        .changed()
                                    {
                                        p.base_url = url;
                                    }
                                    ui.end_row();

                                    ui.label(helpers::field_label("Models URL"));
                                    let mut models_url = p.models_list_url.clone();
                                    if ui
                                        .add(
                                            TextEdit::singleline(&mut models_url)
                                                .id(egui::Id::new(("provider_models_url", &key)))
                                                .desired_width(ui.available_width()),
                                        )
                                        .changed()
                                    {
                                        p.models_list_url = models_url;
                                    }
                                    ui.end_row();
                                });
                        });

                    ui.add_space(6.0);

                    // ── Model ──────────────────────────────────────────
                    CollapsingHeader::new("Model")
                        .id_salt(format!("model_header_{}", key))
                        .default_open(true)
                        .show(ui, |ui| {
                            // Active model.
                            ui.horizontal(|ui| {
                                ui.label(helpers::field_label("Active Model"));
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
                                    p.fill_from_config();
                                    if is_active {
                                        state.session_meta_dirty = true;
                                    }
                                }
                                let p_clone = p.clone();
                                let key_for_fetch = key.clone();
                                if ui
                                    .button("Fetch")
                                    .on_hover_text("Fetch available models from API")
                                    .clicked()
                                {
                                    let models = provider::fetch_models(&p_clone);
                                    let status = format!("{} models", models.len());
                                    settings.fetched_models.insert(key_for_fetch.clone(), models);
                                    settings.fetch_status.insert(key_for_fetch, status);
                                }
                            });

                            let mut saved = std::mem::take(&mut p.saved_models);

                            // Fetched model list.
                            if let Some(models) = settings.fetched_models.get(&key)
                                && !models.is_empty()
                            {
                                ui.add_space(2.0);
                                Frame::NONE
                                    .fill(Palette::BG_SURFACE)
                                    .corner_radius(ROUND_SM)
                                    .inner_margin(Margin::same(6))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("Fetched Models")
                                                .size(10.5).color(Palette::TEXT_MUTED).strong(),
                                        );
                                        ui.add_space(2.0);
                                        let scroll_h = (models.len() as f32 * 24.0).min(150.0);
                                        ScrollArea::vertical()
                                            .id_salt(format!("fetch_scroll_{}", &key))
                                            .max_height(scroll_h)
                                            .show(ui, |ui| {
                                                for m in models.iter() {
                                                    let is_saved = saved.contains(m);
                                                    let is_active = p.model == *m;
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            RichText::new(m)
                                                                .size(11.0)
                                                                .color(if is_active { Palette::ACCENT } else { Palette::TEXT_PRIMARY })
                                                                .monospace(),
                                                        );
                                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                            if is_saved {
                                                                ui.label(
                                                                    RichText::new("Saved")
                                                                        .size(9.5).color(Palette::TEXT_MUTED),
                                                                );
                                                            } else if ui.small_button("+ Add").clicked() {
                                                                saved.push(m.clone());
                                                                let defs = autocode_core::state::model_or_safe(&p.kind, m);
                                                                 let entry = autocode_core::provider_file::ModelEntry {
                                                                     id: m.clone(),
                                                                     context_window: defs.context_window,
                                                                     max_output_tokens: defs.max_output_tokens,
                                                                     max_output_tokens_thinking: defs.max_output_tokens_thinking,
                                                                     thinking_api: defs.thinking_api.clone(),
                                                                     reasoning_efforts: defs.reasoning_efforts.clone(),
                                                                     supports_cache_control: defs.supports_cache_control,
                                                                     requests_per_hour: defs.requests_per_hour,
                                                                     handoff_percent: p.handoff_percent,
                                                                     temperature: p.temperature,
                                                                     top_p: p.top_p,
                                                                     frequency_penalty: p.frequency_penalty,
                                                                     presence_penalty: p.presence_penalty,
                                                                 };
                                                                let cm = p.models_config.get_or_insert_with(std::collections::HashMap::new);
                                                                cm.insert(m.clone(), entry);
                                                                provider_dirty = true;
                                                            }
                                                              if ui.small_button("Select").clicked() {
                                                                  p.model = m.clone();
                                                                  p.fill_from_config();
                                                                  if state.active_provider == key {
                                                                      state.session_meta_dirty = true;
                                                                  }
                                                              }
                                                        });
                                                    });
                                                }
                                            });
                                    });
                            }

                            // Saved models.
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                let model_count = saved.len();
                                if model_count == 0 {
                                    ui.label(
                                        RichText::new("No saved models")
                                            .size(10.5)
                                            .color(Palette::TEXT_MUTED),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("{} saved", model_count))
                                            .size(10.5)
                                            .color(Palette::TEXT_MUTED),
                                    );
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("+").clicked() {
                                        saved.push(String::new());
                                    }
                                });
                            });
                            if !saved.is_empty() {
                                let mut remove_idx: Option<usize> = None;
                                let model_col_w = ui.available_width() - 50.0;
                                let names: Vec<String> = saved.clone();
                                for (i, name) in names.into_iter().enumerate() {
                                    let is_active = p.model == name;
                                    let mut mc = p.models_config.as_ref()
                                        .and_then(|m| m.get(&name)).cloned()
                                        .unwrap_or_else(|| {
                                            let defs = autocode_core::state::model_or_safe(&p.kind, &name);
                                             autocode_core::provider_file::ModelEntry {
                                                 id: name.clone(),
                                                 context_window: defs.context_window,
                                                 max_output_tokens: defs.max_output_tokens,
                                                 max_output_tokens_thinking: defs.max_output_tokens_thinking,
                                                 thinking_api: defs.thinking_api.clone(),
                                                 reasoning_efforts: defs.reasoning_efforts.clone(),
                                                 supports_cache_control: defs.supports_cache_control,
                                                 requests_per_hour: defs.requests_per_hour,
                                                 handoff_percent: p.handoff_percent,
                                                 temperature: p.temperature,
                                                 top_p: p.top_p,
                                                 frequency_penalty: p.frequency_penalty,
                                                 presence_penalty: p.presence_penalty,
                                             }
                                        });
                                    let mut cfg_changed = false;

                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            let mut new_name = name.clone();
                                            let resp = ui.add_sized(
                                                egui::vec2(model_col_w, 20.0),
                                                TextEdit::singleline(&mut new_name)
                                                    .id(egui::Id::new(("saved_model", &key, i)))
                                                    .hint_text("model-name"),
                                            );
                                            if resp.changed() {
                                                saved[i] = new_name;
                                            }
                                            if !name.is_empty()
                                                && ui.small_button("\u{2605}").on_hover_text("Select this model").clicked() {
                                                    p.model = name.clone();
                                                    p.fill_from_config();
                                                    if state.active_provider == key {
                                                        state.session_meta_dirty = true;
                                                    }
                                                }
                                            if ui.small_button("x").clicked() {
                                                remove_idx = Some(i);
                                            }
                                        });

                                        CollapsingHeader::new(
                                            if is_active { "Advanced (active)" } else { "Advanced" }
                                        )
                                            .id_salt(format!("model_adv_{}_{}", &key, i))
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                Grid::new(format!("model_adv_grid_{}_{}", &key, i))
                                                    .num_columns(2)
                                                    .spacing([12.0, 6.0])
                                                    .min_col_width(60.0)
                                                    .show(ui, |ui| {
                                                        ui.label(helpers::field_label("Context Window"))
                                                            .on_hover_text("Maximum tokens the model can see (prompt + response). Higher = more context but slower.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.context_window as i32;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(1000).range(1024..=2_000_000)
                                                                    .suffix(" tokens"),
                                                            ).on_hover_text("Max context length in tokens").changed() {
                                                                mc.context_window = val.max(1024) as u32;
                                                                if is_active { p.max_context_tokens = mc.context_window; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Max Output"))
                                                            .on_hover_text("Maximum tokens the model can generate in a single response.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.max_output_tokens as i32;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(1000).range(256..=2_000_000)
                                                                    .suffix(" tokens"),
                                                            ).on_hover_text("Max response length in tokens").changed() {
                                                                mc.max_output_tokens = val.max(256) as u32;
                                                                if is_active { p.max_output_tokens = mc.max_output_tokens; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Max (Thinking)"))
                                                            .on_hover_text("Max tokens for reasoning/thinking output. Only used when thinking mode is enabled.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.max_output_tokens_thinking.unwrap_or(mc.max_output_tokens * 2) as i32;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(1000).range(256..=2_000_000)
                                                                    .suffix(" tokens"),
                                                            ).on_hover_text("Thinking/reasoning token budget").changed() {
                                                                mc.max_output_tokens_thinking = Some(val.max(256) as u32);
                                                                if is_active { p.max_output_tokens_thinking = val.max(256) as u32; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Thinking API"))
                                                            .on_hover_text("Which API protocol the model uses for reasoning. DeepSeek/OpenAI send reasoning_content, Anthropic uses thinking blocks.");
                                                        ui.horizontal(|ui| {
                                                            let current_ta = autocode_core::state::parse_thinking_api(&mc.thinking_api);
                                                            let current_label = current_ta.label();
                                                            egui::ComboBox::from_id_salt(format!("th_api_{}_{}", &key, i))
                                                                .selected_text(current_label)
                                                                .width(ui.available_width())
                                                                .show_ui(ui, |ui| {
                                                                    for variant in ThinkingApi::variants() {
                                                                        if ui.selectable_label(
                                                                            current_ta == *variant,
                                                                            variant.label(),
                                                                        ).clicked() {
                                                                            mc.thinking_api = variant.label().to_lowercase();
                                                                            if is_active { p.thinking_api = variant.clone(); }
                                                                            cfg_changed = true;
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Handoff"))
                                                            .on_hover_text("Trigger handoff when context usage reaches this percentage. Prevents hitting the context limit mid-task.");
                                                        ui.horizontal(|ui| {
                                                            let mut pct = mc.handoff_percent as f32;
                                                            ui.add(
                                                                egui::Slider::new(&mut pct, 10.0..=95.0)
                                                                    .step_by(5.0).show_value(false)
                                                                    .trailing_fill(true),
                                                            ).on_hover_text("Handoff threshold % of context window");
                                                            let new_pct = pct as u8;
                                                            mc.handoff_percent = new_pct;
                                                            if is_active { p.handoff_percent = new_pct; }
                                                            cfg_changed = true;
                                                            ui.label(
                                                                RichText::new(format!("{}%", new_pct))
                                                                    .size(11.0).color(Palette::ACCENT).strong(),
                                                            );
                                                            let threshold = (mc.context_window as u64 * new_pct as u64) / 100;
                                                            let threshold_fmt = if threshold >= 1_000_000 {
                                                                format!("{:.1}M", threshold as f64 / 1_000_000.0)
                                                            } else if threshold >= 1_000 {
                                                                format!("{:.1}K", threshold as f64 / 1_000.0)
                                                            } else { threshold.to_string() };
                                                            ui.label(
                                                                RichText::new(format!("@{} tokens", threshold_fmt))
                                                                    .size(10.5).color(Palette::TEXT_MUTED),
                                                            );
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Temperature"))
                                                            .on_hover_text("Controls randomness: 0 = deterministic, 1 = balanced, 2 = very creative. Lower for code, higher for brainstorming.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.temperature;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(0.05).range(0.0..=2.0)
                                                                    .suffix(""),
                                                            ).on_hover_text("0 = conservative, 2 = creative").changed() {
                                                                mc.temperature = val.clamp(0.0, 2.0);
                                                                if is_active { p.temperature = mc.temperature; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Top P"))
                                                            .on_hover_text("Nucleus sampling: cumulatively picks tokens until probability P is reached. 1.0 = disabled (consider all tokens). Lower = more focused.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.top_p;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(0.05).range(0.01..=1.0),
                                                            ).on_hover_text("1.0 = disabled, lower = more focused output").changed() {
                                                                mc.top_p = val.clamp(0.01, 1.0);
                                                                if is_active { p.top_p = mc.top_p; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Freq Penalty"))
                                                            .on_hover_text("Penalizes repeating the same words. Positive values reduce repetition. Range: -2 (encourage repetition) to +2 (strongly discourage).");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.frequency_penalty;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(0.1).range(-2.0..=2.0),
                                                            ).on_hover_text("-2 to +2. Higher = less repetition.").changed() {
                                                                mc.frequency_penalty = val.clamp(-2.0, 2.0);
                                                                if is_active { p.frequency_penalty = mc.frequency_penalty; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Pres Penalty"))
                                                            .on_hover_text("Penalizes repeating the same topics/concepts. Positive values encourage the model to talk about new subjects. Range: -2 to +2.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.presence_penalty;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(0.1).range(-2.0..=2.0),
                                                            ).on_hover_text("-2 to +2. Higher = more topic diversity.").changed() {
                                                                mc.presence_penalty = val.clamp(-2.0, 2.0);
                                                                if is_active { p.presence_penalty = mc.presence_penalty; }
                                                                cfg_changed = true;
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(helpers::field_label("Rate Limit"))
                                                            .on_hover_text("Max API requests per hour. 0 = unlimited. Use to stay within a provider's rate tier.");
                                                        ui.horizontal(|ui| {
                                                            let mut val = mc.requests_per_hour.unwrap_or(0) as i32;
                                                            if ui.add(
                                                                egui::DragValue::new(&mut val)
                                                                    .speed(100).range(0..=1000000),
                                                            ).on_hover_text("0 = no limit").changed() {
                                                                mc.requests_per_hour = if val <= 0 { None } else { Some(val as u32) };
                                                                if is_active { p.requests_per_hour = mc.requests_per_hour; }
                                                                cfg_changed = true;
                                                            }
                                                            ui.label(
                                                                RichText::new("req/hr (0 = unlimited)")
                                                                    .size(10.5).color(Palette::TEXT_MUTED),
                                                            );
                                                        });
                                                        ui.end_row();
                                                    });
                                            });
                                    });

                                    if cfg_changed {
                                        let config_map = p.models_config.get_or_insert_with(std::collections::HashMap::new);
                                        config_map.insert(name, mc);
                                    }
                                }
                                if let Some(idx) = remove_idx {
                                    saved.remove(idx);
                                }
                            }
                            p.saved_models = saved;
                        });

                    // Provider-level Outside Access toggle.
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let mut val = p.allow_project_escape;
                        if ui.checkbox(&mut val, "")
                            .on_hover_text(
                                "When enabled, the AI can read/list/grep files \
                                 anywhere on disk, not just inside the project folder.",
                            ).changed()
                        {
                            p.allow_project_escape = val;
                        }
                        if val {
                            ui.label(RichText::new("Outside Access: Enabled")
                                .size(11.0).color(Palette::WARNING).strong());
                        } else {
                            ui.label(RichText::new("Outside Access: Restricted to project")
                                .size(11.0).color(Palette::TEXT_MUTED));
                        }
                    });
                        }); // provider body collapsing header
                }); // Frame.show

            ui.add_space(8.0);
        });
    }

    if let Some((old_key, new_key)) = pending_rename
        && old_key != new_key
        && !state.providers.contains_key(&new_key)
        && let Some(ap) = state.providers.remove(&old_key)
    {
        state.providers.insert(new_key.clone(), ap);
        provider_dirty = true;
        if state.active_provider == old_key {
            state.active_provider = new_key.clone();
            state.session_meta_dirty = true;
            if let Some(sess) = state.active_session_mut() {
                sess.provider_label = new_key;
            }
        }
    }
    for key in to_remove {
        state.providers.remove(&key);
        provider_dirty = true;
        if state.active_provider == key {
            state.active_provider = state.providers.keys().next().cloned().unwrap_or_default();
        }
    }
    if provider_dirty {
        let _ = provider_file::save_providers_file(&state.providers);
    }
    if let Some((label, model)) = set_active_key {
        state.active_provider = label.clone();
        state.session_meta_dirty = true;
        if let Some(sess) = state.active_session_mut() {
            sess.provider_label = label;
            sess.model = model;
        }
    }
    if let Some(disabled_key) = disable_switch_key {
        let next = state
            .providers
            .iter()
            .find(|(k, v)| *k != &disabled_key && v.enabled)
            .map(|(k, _)| k.clone())
            .or_else(|| {
                state
                    .providers
                    .keys()
                    .find(|k| *k != &disabled_key)
                    .cloned()
            });
        if let Some(next_key) = next {
            let model = state
                .providers
                .get(&next_key)
                .map(|p| p.model.clone())
                .unwrap_or_default();
            state.active_provider = next_key.clone();
            if let Some(sess) = state.active_session_mut() {
                sess.provider_label = next_key;
                sess.model = model;
            }
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
                                ui.label(
                                    RichText::new("Active").size(10.0).color(Palette::SUCCESS),
                                );
                            }
                        });
                    });

                    // Collapsible session list for this project.
                    let proj_sessions: Vec<&Session> = state
                        .sessions
                        .iter()
                        .filter(|s| {
                            s.project_id.as_deref() == Some(&p.id)
                                && autocode_core::session_storage::session_exists(p, s)
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
                                                RichText::new(&sess.id).monospace().size(11.0),
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
                let _ = autocode_core::session_storage::save_session_meta(proj, s);
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
                        ui.label(RichText::new("ms").size(10.5).color(Palette::TEXT_MUTED));
                    });
                    ui.end_row();

                    ui.label(helpers::field_label("Web Rate Limit"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.web_rate_limit_ms)
                                .speed(50.0)
                                .range(0..=10000),
                        );
                        ui.label(RichText::new("ms").size(10.5).color(Palette::TEXT_MUTED));
                    });
                    ui.end_row();

                    ui.label(helpers::field_label("Disk Write Rate"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.disk_write_rate_ms)
                                .speed(10.0)
                                .range(0..=5000),
                        );
                        ui.label(RichText::new("ms").size(10.5).color(Palette::TEXT_MUTED));
                    });
                    ui.end_row();
                });

            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Messages in RAM: controls how many are held in memory and displayed. \
                     Full history is saved to disk and reloaded for API requests. \
                     Completion Delay: minimum pause (ms) between consecutive API calls \
                     to pace rapid tool-use loops. \
                     Disk Write Rate: minimum interval (ms) between message writes to disk. \
                     0 = write every message immediately.",
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
            .desired_width(ui.available_width())
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

    // -- Handoff trigger prompt ------------------------------------------
    ui.label(
        RichText::new("Handoff Trigger Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Sent as a user message when the context threshold is reached and the \
             model hasn't called handoff. Instructs the model to stop work, save \
             progress, and handoff immediately.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.handoff_trigger_prompt)
            .desired_rows(6)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.handoff_trigger_prompt =
            autocode_core::state::DEFAULT_HANDOFF_TRIGGER_PROMPT.to_string();
    }

    ui.add_space(16.0);

    // -- Handoff continuation prompt --------------------------------------
    ui.label(
        RichText::new("Handoff Continuation Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Used as the first message in a new session when a forced handoff \
             occurs (e.g. context exhausted) and there are active project-level \
             tasks. Instructs the model to pick up where things left off.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.handoff_continuation_prompt)
            .desired_rows(6)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.handoff_continuation_prompt =
            autocode_core::state::DEFAULT_HANDOFF_CONTINUATION_PROMPT.to_string();
    }

    ui.add_space(16.0);

    // -- Connection drop prompt --------------------------------------------
    ui.label(
        RichText::new("Connection Drop Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Prepended when the connection drops and tasks are incomplete. \
             Tells the model to pick up where it left off.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.connection_drop_prompt)
            .desired_rows(4)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.connection_drop_prompt =
            autocode_core::state::DEFAULT_CONNECTION_DROP_PROMPT.to_string();
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
                    state.shell_timeout_secs =
                        state.shell_timeout_secs.min(state.shell_timeout_max_secs);
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
        state.tool_timeout_secs = autocode_core::helpers::default_tool_timeout();
    }
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

    let providers_str = autocode_core::state::provider_ids()
        .iter()
        .filter_map(|id| {
            autocode_core::state::provider_manifest(&autocode_core::state::ProviderKind::new(id))
                .map(|m| m.label.as_str())
        })
        .collect::<Vec<&str>>()
        .join(" | ");
    let info = [
        ("Version", "0.1.0"),
        ("UI", "egui 0.34 / eframe 0.34"),
        ("Language", "Rust -- serde, egui only"),
        ("Providers", providers_str.as_str()),
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
            ui.label(
                RichText::new("OpenGL")
                    .size(12.0)
                    .color(Palette::SUCCESS)
                    .strong(),
            );
            ui.label(
                RichText::new("(Glow backend)")
                    .size(11.0)
                    .color(Palette::TEXT_MUTED),
            );
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
                if ui
                    .link("OpenGL installation guide (docs.mesa3d.org)")
                    .clicked()
                {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", "https://docs.mesa3d.org/install.html"])
                            .spawn();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg("https://docs.mesa3d.org/install.html")
                            .spawn();
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg("https://docs.mesa3d.org/install.html")
                            .spawn();
                    }
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
