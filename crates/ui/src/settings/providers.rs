use crate::helpers;
use crate::theme::{Palette, ROUND_MD, ROUND_SM};
use autocode_ai::provider;
use autocode_core::state::{AppState, ThinkingApi};
use egui::{CollapsingHeader, Frame, Grid, Margin, RichText, ScrollArea, TextEdit};

use super::state::{ProbeJob, SettingsState};

pub fn show_providers(ui: &mut egui::Ui, state: &mut AppState, settings: &mut SettingsState) {
    helpers::section_heading(ui, "API Providers");

    ui.add_space(4.0);

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
    // Deferred probe launches: (provider label, model id). Collected while
    // iterating provider cards (which borrow state.providers mutably) and
    // spawned after the loop, following the file's deferred-action pattern.
    let mut pending_probes: Vec<(String, String)> = Vec::new();

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
                            // Fetch button — async: spawns a worker thread and
                            // polls the channel every frame, so the UI stays
                            // responsive while the request is in flight.
                            ui.add_space(4.0);
                            let fetching = settings
                                .fetch_status
                                .get(&key)
                                .map(|s| s == "Fetching...")
                                .unwrap_or(false);
                            let p_clone = p.clone();
                            let key_for_fetch = key.clone();
                            if ui
                                .add_enabled(
                                    !fetching,
                                    egui::Button::new("Fetch"),
                                )
                                .on_hover_text("Fetch available models from API")
                                .clicked()
                            {
                                settings.fetch_status.insert(key.clone(), "Fetching...".into());
                                let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();
                                let worker_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                                let _ = std::thread::Builder::new()
                                    .name("model-fetch".into())
                                    .spawn(move || {
                                        let models = if worker_cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                            Vec::new()
                                        } else {
                                            provider::fetch_models(&p_clone)
                                        };
                                        let _ = tx.send(models);
                                    });
                                settings.fetch_jobs.insert(key_for_fetch, rx);
                            }
                            if let Some(status) = settings.fetch_status.get(&key) {
                                let (text, color) = if status == "Fetching..." {
                                    (status.clone(), Palette::WARNING)
                                } else if status.contains("failed") {
                                    (status.clone(), Palette::ERROR)
                                } else {
                                    (status.clone(), Palette::SUCCESS)
                                };
                                ui.label(
                                    RichText::new(text)
                                        .size(10.0)
                                        .color(color),
                                );
                            }
                            // Fetched model list.
                            if let Some(models) = settings.fetched_models.get(&key)
                                && !models.is_empty()
                            {
                                // Clone the list out so the UI closure can
                                // borrow `settings` mutably (filter box).
                                let models: Vec<String> = models.clone();
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
                                        // Filter box.
                                        ui.horizontal(|ui| {
                                            let mut filter = settings
                                                .model_filter
                                                .get(&key)
                                                .cloned()
                                                .unwrap_or_default();
                                            let resp = ui.add(
                                                TextEdit::singleline(&mut filter)
                                                    .id(egui::Id::new(("model_filter", &key)))
                                                    .hint_text("filter models...")
                                                    .desired_width(220.0),
                                            );
                                            if resp.changed() {
                                                settings.model_filter.insert(key.clone(), filter);
                                            }
                                        });
                                        ui.add_space(2.0);
                                        let filter_text = settings
                                            .model_filter
                                            .get(&key)
                                            .cloned()
                                            .unwrap_or_default()
                                            .to_lowercase();
                                        let (_, rect) = ui.allocate_space(egui::vec2(ui.available_size().x, 300.0));
                                        let mut scroll_ui = ui.new_child(
                                            egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()),
                                        );
                                        ScrollArea::vertical()
                                            .id_salt(format!("fetch_scroll_{}", &key))
                                            .max_height(300.0)
                                            .show(&mut scroll_ui, |ui| {
                                                for m in models.iter().filter(|m| {
                                                    filter_text.is_empty()
                                                        || m.to_lowercase().contains(&filter_text)
                                                }) {
                                                    let is_saved = p.saved_models.contains(m);
                                                    let is_active = p.model == *m;
                                                    let probe_active = settings
                                                        .probe_jobs
                                                        .iter()
                                                        .any(|j| j.provider_label == key && j.model_id == *m);
                                                    let probe_result_key = format!("{}\u{1f}{}", key, m);
                                                    let probe_result = settings
                                                        .probe_results
                                                        .get(&probe_result_key)
                                                        .cloned();
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            RichText::new(m)
                                                                .size(11.0)
                                                                .color(if is_active { Palette::ACCENT } else { Palette::TEXT_PRIMARY })
                                                                .monospace(),
                                                        );
                                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                            if probe_active {
                                                                ui.label(
                                                                    RichText::new("Probing...")
                                                                        .size(9.5).color(Palette::WARNING),
                                                                );
                                                            } else if is_saved {
                                                                ui.label(
                                                                    RichText::new("Saved")
                                                                        .size(9.5).color(Palette::TEXT_MUTED),
                                                                );
                                                            } else if ui.small_button("+ Probe & Add").clicked() {
                                                                pending_probes.push((key.clone(), m.clone()));
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
                                                    // Inline progress bar for this model's probe.
                                                    if probe_active
                                                        && let Some(job) = settings
                                                            .probe_jobs
                                                            .iter()
                                                            .find(|j| j.provider_label == key && j.model_id == *m)
                                                    {
                                                        let frac = job.progress_fraction();
                                                        let bar = egui::ProgressBar::new(frac)
                                                            .show_percentage()
                                                            .desired_height(14.0);
                                                        ui.add(bar);
                                                        ui.label(
                                                            RichText::new(job.status_text())
                                                                .size(9.5)
                                                                .color(Palette::TEXT_MUTED),
                                                        );
                                                    } else if let Some((ok, summary)) = probe_result {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "{} — {}",
                                                                if ok { "OK" } else { "FAILED" },
                                                                summary
                                                            ))
                                                            .size(9.5)
                                                            .color(if ok { Palette::SUCCESS } else { Palette::ERROR }),
                                                        );
                                                    }
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
                                                  supports_vision: defs.supports_vision,
                                                  handoff_percent: p.handoff_percent,
                                                  temperature: p.temperature,
                                                  top_p: p.top_p,
                                                  frequency_penalty: p.frequency_penalty,
                                                   presence_penalty: p.presence_penalty,
                                                   loop_aggressiveness: autocode_core::state::LoopAggressiveness::default(),
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

                                                        ui.label(helpers::field_label("LRU Aggression"))
                                                            .on_hover_text("Controls when LRU pruning triggers and how aggressively old messages are removed. Balanced = default.");
                                                        ui.horizontal(|ui| {
                                                            let current = mc.loop_aggressiveness;
                                                            for variant in autocode_core::state::LoopAggressiveness::variants() {
                                                                let is_sel = *variant == current;
                                                                if ui.add(
                                                                    egui::Button::new(
                                                                        RichText::new(variant.label()).size(11.0)
                                                                    ).selected(is_sel).stroke(if is_sel {
                                                                        egui::Stroke::new(1.0, Palette::ACCENT)
                                                                    } else {
                                                                        egui::Stroke::new(1.0, Palette::BORDER)
                                                                    })
                                                                ).clicked() {
                                                                    mc.loop_aggressiveness = *variant;
                                                                    cfg_changed = true;
                                                                }
                                                            }
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

    // Launch any probes requested inside the card loop (deferred because the
    // loop borrows state.providers mutably).
    for (label, model) in pending_probes {
        start_probe(settings, state, &label, &model);
    }

    if let Some((old_key, new_key)) = pending_rename
        && old_key != new_key
        && !state.providers.contains_key(&new_key)
        && let Some(ap) = state.providers.remove(&old_key)
    {
        state.providers.insert(new_key.clone(), ap);
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
        if state.active_provider == key {
            state.active_provider = state.providers.keys().next().cloned().unwrap_or_default();
        }
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

// -- Probe plumbing --------------------------------------------------------------

/// Launch a background probe for one model on one provider and remember the
/// pending state so the UI can render a progress bar. Called from the
/// "+ Probe & Add" button; the result is applied when the worker reports Done.
pub(crate) fn start_probe(
    settings: &mut SettingsState,
    state: &mut AppState,
    provider_label: &str,
    model_id: &str,
) {
    let Some(p) = state.providers.get(provider_label) else {
        return;
    };
    if p.api_key.as_str().is_empty() {
        settings.probe_results.insert(
            format!("{}\u{1f}{}", provider_label, model_id),
            (false, "No API key set for this provider".into()),
        );
        return;
    }
    let Some(spec) =
        autocode_core::helpers::provider_manifest(&p.kind).and_then(|m| m.probe.clone())
    else {
        settings.probe_results.insert(
            format!("{}\u{1f}{}", provider_label, model_id),
            (false, "Provider has no probe config".into()),
        );
        return;
    };
    let handle = autocode_ai::provider::probe::probe_model(p.clone(), model_id.to_string(), spec);
    settings.probe_jobs.push(ProbeJob::new(
        provider_label.to_string(),
        model_id.to_string(),
        handle,
    ));
    // Clear any previous result for this model while it re-probes.
    settings
        .probe_results
        .remove(&format!("{}\u{1f}{}", provider_label, model_id));
}

/// Poll in-flight probe jobs and fetch jobs. Returns true if anything is
/// still running (App::update keeps repainting while true). Runs every frame
/// even when the settings window is closed so probes complete regardless.
pub(crate) fn pump_probe_jobs(settings: &mut SettingsState, state: &mut AppState) -> bool {
    let mut any_running = false;

    // Model-list fetch jobs. A disconnected channel (worker gone) counts as
    // a failed fetch so the job never leaks.
    let done_fetches: Vec<String> = settings
        .fetch_jobs
        .iter()
        .filter_map(|(label, rx)| match rx.try_recv() {
            Ok(models) => Some((label.clone(), models)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some((label.clone(), Vec::new())),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        })
        .flat_map(|(label, models)| {
            if models.is_empty() {
                settings.fetch_status.insert(
                    label.clone(),
                    "Fetch failed — check key/URL, see logs".into(),
                );
            } else {
                settings
                    .fetch_status
                    .insert(label.clone(), format!("{} models", models.len()));
                settings.fetched_models.insert(label.clone(), models);
            }
            Some(label)
        })
        .collect();
    for label in done_fetches {
        settings.fetch_jobs.remove(&label);
    }
    any_running |= !settings.fetch_jobs.is_empty();

    // Model probes: drain updates, split finished jobs out for result
    // handling. Finished handles drop here, which cancels any worker still
    // parked on a socket read.
    let mut running: Vec<ProbeJob> = Vec::new();
    let mut finished: Vec<ProbeJob> = Vec::new();
    for mut job in std::mem::take(&mut settings.probe_jobs) {
        if job.pump() {
            finished.push(job);
        } else {
            running.push(job);
        }
    }
    settings.probe_jobs = running;
    for job in finished {
        let label = job.provider_label.clone();
        let model = job.model_id.clone();
        let ok = job.succeeded();
        let summary = job.summary();
        settings
            .probe_results
            .insert(format!("{}\u{1f}{}", label, model), (ok, summary));
        if ok {
            apply_probed_model(state, &label, &model);
            // Persist immediately so a crash can't lose the probed model.
            persist_providers(state);
        }
    }
    any_running |= !settings.probe_jobs.is_empty();

    any_running
}

/// Backfill a successfully probed model into the provider: saved_models, the
/// per-model config, and the live provider when this model is the one in use
/// (or the provider had none yet).
fn apply_probed_model(state: &mut AppState, label: &str, model_id: &str) {
    let Some(p) = state.providers.get_mut(label) else {
        return;
    };
    if !p.saved_models.iter().any(|m| m == model_id) {
        p.saved_models.push(model_id.to_string());
        p.saved_models.sort();
    }
    // The probe registered the manifest in the probed registry; copy it into
    // the per-model config so the providers.json file on disk carries it.
    //
    // Only the *learned* fields come from the probe. The sampling knobs and the
    // handoff threshold belong to the user's provider, so they are inherited
    // here exactly as every other `ModelEntry` in the codebase builds them (the
    // model-row default in this file, and both branches of
    // `convert_from_providers`). Hardcoding them reset a provider's temperature,
    // top-p, penalties and handoff threshold the moment a model was probed,
    // because `fill_from_config` copies this entry straight back onto the
    // provider.
    if let Some(pm) = autocode_core::helpers::probed::lookup(&p.kind, model_id) {
        let entry = autocode_core::storage::provider_file::ModelEntry {
            id: model_id.to_string(),
            context_window: pm.context_window,
            max_output_tokens: pm.max_output_tokens,
            max_output_tokens_thinking: pm.max_output_tokens_thinking,
            thinking_api: pm.thinking_api.clone(),
            reasoning_efforts: pm.reasoning_efforts.clone(),
            supports_cache_control: pm.supports_cache_control,
            requests_per_hour: pm.requests_per_hour,
            thinking_overrides: pm.thinking_overrides.clone(),
            supports_vision: pm.supports_vision,
            handoff_percent: p.handoff_percent,
            temperature: p.temperature,
            top_p: p.top_p,
            frequency_penalty: p.frequency_penalty,
            presence_penalty: p.presence_penalty,
            loop_aggressiveness: autocode_core::state::LoopAggressiveness::default(),
        };
        p.models_config
            .get_or_insert_with(std::collections::HashMap::new)
            .insert(model_id.to_string(), entry);
    }
    // Put the learned values to work now rather than at the next launch. If this
    // model is the one already in use, the context window the app is running on
    // is exactly what the probe just corrected — re-probing it and seeing
    // nothing change until a restart is the bug. A provider with no model yet
    // adopts this one.
    if p.model.is_empty() {
        p.model = model_id.to_string();
        p.fill_from_config();
    } else if p.model == model_id {
        p.fill_from_config();
    }
}

/// Persist providers + probe results to AutoCode_data/providers.json and
/// fold probed manifests into the on-disk file. Called on window close and
/// after every completed probe.
pub(crate) fn persist_providers(state: &AppState) {
    if let Err(e) = autocode_core::storage::provider_file::save_providers_file(&state.providers) {
        eprintln!("[settings] Failed to save providers file: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autocode_core::state::{ApiProvider, ModelManifest, ProviderKind};

    /// A manifest shaped like a probe result: only the learned fields carry
    /// information, which is exactly what `apply_probed_model` should copy.
    fn learned(context_window: u32, max_output_tokens: u32) -> ModelManifest {
        ModelManifest {
            context_window,
            max_output_tokens,
            max_output_tokens_thinking: None,
            thinking_api: "off".into(),
            reasoning_efforts: Vec::new(),
            supports_cache_control: false,
            requests_per_hour: None,
            thinking_overrides: std::collections::HashMap::new(),
            supports_vision: false,
        }
    }

    /// A provider whose sampling settings differ from every `ModelEntry`
    /// default, so a reset back to those defaults is visible.
    fn configured_provider(kind: &ProviderKind) -> ApiProvider {
        let mut p = ApiProvider::new(kind.clone());
        p.temperature = 0.7;
        p.top_p = 0.9;
        p.frequency_penalty = 0.4;
        p.presence_penalty = 0.3;
        p.handoff_percent = 55;
        p
    }

    /// Probing a model adds what the probe learned and nothing else. The
    /// sampling knobs and handoff threshold are the user's provider settings —
    /// hardcoding them here reset a configured provider the moment a model was
    /// probed, because `fill_from_config` copies the entry straight back onto
    /// the provider.
    #[test]
    fn a_probed_model_inherits_the_providers_sampling_settings() {
        let kind = ProviderKind::new("probe-test-inherit");
        let mut state = AppState::default();
        state
            .providers
            .insert("lab".into(), configured_provider(&kind));

        autocode_core::helpers::probed::insert(&kind, "learned-model", learned(200_000, 8_192));
        apply_probed_model(&mut state, "lab", "learned-model");

        let p = state.providers.get("lab").expect("provider");
        let cfg = p
            .models_config
            .as_ref()
            .and_then(|m| m.get("learned-model"))
            .cloned()
            .expect("the probed model gets a per-model config");
        assert_eq!(
            cfg.temperature, 0.7,
            "the provider's temperature is not the probe's to change"
        );
        assert_eq!(cfg.top_p, 0.9);
        assert_eq!(cfg.frequency_penalty, 0.4);
        assert_eq!(cfg.presence_penalty, 0.3);
        assert_eq!(cfg.handoff_percent, 55);
        assert_eq!(
            cfg.context_window, 200_000,
            "the learned window is recorded"
        );
        assert_eq!(cfg.max_output_tokens, 8_192);
        assert_eq!(
            p.model, "learned-model",
            "a provider with no model adopts the probed one"
        );
        assert_eq!(
            p.max_context_tokens, 200_000,
            "and runs on the learned window immediately"
        );
    }

    /// Re-probing the model that is already in use must correct the running
    /// provider: the context window is the reason it was probed again.
    #[test]
    fn re_probing_the_active_model_refreshes_it_immediately() {
        let kind = ProviderKind::new("probe-test-active");
        let mut state = AppState::default();
        let mut p = configured_provider(&kind);
        p.model = "in-use".to_string();
        p.max_context_tokens = 8_000; // a stale guess, e.g. the safe default
        state.providers.insert("lab".into(), p);

        autocode_core::helpers::probed::insert(&kind, "in-use", learned(1_000_000, 32_000));
        apply_probed_model(&mut state, "lab", "in-use");

        let p = state.providers.get("lab").expect("provider");
        assert_eq!(
            p.max_context_tokens, 1_000_000,
            "the live provider is corrected now, not at the next launch"
        );
        assert_eq!(p.max_output_tokens, 32_000);
        assert_eq!(
            p.temperature, 0.7,
            "refreshing the learned values must not reset the user's sampling setting"
        );
    }
}
