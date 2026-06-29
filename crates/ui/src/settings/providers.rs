use crate::helpers;
use crate::theme::{Palette, ROUND_MD, ROUND_SM};
use autocode_ai::provider;
use autocode_core::state::{AppState, ThinkingApi};
use autocode_core::storage::provider_file;
use egui::{CollapsingHeader, Frame, Grid, Margin, RichText, ScrollArea, TextEdit};

use super::state::SettingsState;

pub fn show_providers(ui: &mut egui::Ui, state: &mut AppState, settings: &mut SettingsState) {
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
                                .small_button("R")
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
                            // Fetch button.
                            ui.add_space(4.0);
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
                            // Fetched model list.
                            if let Some(models) = settings.fetched_models.get(&key)
                                && !models.is_empty()
                            {
                                ui.add_space(4.0);
                                Frame::NONE
                                    .fill(Palette::BG_BASE)
                                    .corner_radius(ROUND_SM)
                                    .inner_margin(Margin::same(6))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("Fetched Models")
                                                .size(10.5).color(Palette::TEXT_MUTED).strong(),
                                        );
                                        ui.add_space(2.0);
                                        let (_, rect) = ui.allocate_space(egui::vec2(ui.available_size().x, 300.0));
                                        let mut scroll_ui = ui.new_child(
                                            egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()),
                                        );
                                        ScrollArea::vertical()
                                            .id_salt(format!("fetch_scroll_{}", &key))
                                            .max_height(300.0)
                                            .show(&mut scroll_ui, |ui| {
                                                for m in models.iter() {
                                                    let is_saved = p.saved_models.contains(m);
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
                                                                p.saved_models.push(m.clone());
                                                                let defs = autocode_core::helpers::model_or_safe(&p.kind, m);
                                                                let entry = autocode_core::storage::provider_file::ModelEntry {
                                                                    id: m.clone(),
                                                                    context_window: defs.context_window,
                                                                    max_output_tokens: defs.max_output_tokens,
                                                                    max_output_tokens_thinking: defs.max_output_tokens_thinking,
                                                                    thinking_api: defs.thinking_api.clone(),
                                                                    reasoning_efforts: defs.reasoning_efforts.clone(),
                                                                    supports_cache_control: defs.supports_cache_control,
                                                                    requests_per_hour: defs.requests_per_hour,
                                                                    thinking_overrides: defs.thinking_overrides.clone(),
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
                            });

                            let mut saved = std::mem::take(&mut p.saved_models);



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
                                            let defs = autocode_core::helpers::model_or_safe(&p.kind, &name);
                                             autocode_core::storage::provider_file::ModelEntry {
                                                 id: name.clone(),
                                                 context_window: defs.context_window,
                                                 max_output_tokens: defs.max_output_tokens,
                                                 max_output_tokens_thinking: defs.max_output_tokens_thinking,
                                                 thinking_api: defs.thinking_api.clone(),
                                                 reasoning_efforts: defs.reasoning_efforts.clone(),
                                                 supports_cache_control: defs.supports_cache_control,
                                                  requests_per_hour: defs.requests_per_hour,
                                                  thinking_overrides: defs.thinking_overrides.clone(),
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
                                                            let current_ta = autocode_core::helpers::parse_thinking_api(&mc.thinking_api);
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
