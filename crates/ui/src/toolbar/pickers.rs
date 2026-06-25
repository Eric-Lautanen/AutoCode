use egui::RichText;

use crate::theme::Palette;
use autocode_core::state::{AppState, Project};

pub fn show_project_picker(ui: &mut egui::Ui, state: &mut AppState) {
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
                        autocode_core::storage::switch_to_project(state, &p.id);
                        autocode_ai::chat::ensure_session(state);
                    }
                });
            }
            ui.separator();
            if ui.button("New Project...").clicked() {
                let current_dir = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string());
                let ctx = ui.ctx();
                crate::helpers::set_temp_bool(ctx, crate::helpers::data::OPEN_NEW_PROJECT, true);
                crate::helpers::set_temp(
                    ctx,
                    crate::helpers::data::NEW_PROJECT_DIALOG_PATH,
                    current_dir,
                );
            }
        });
}

pub fn show_session_picker(ui: &mut egui::Ui, state: &mut AppState) {
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
                        .map(|proj| autocode_core::storage::session_exists(proj, s))
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
                if ui.selectable_label(!has_active, "Select Session").clicked() && has_active {
                    state.active_session_id = None;
                }
                for (sid, slabel) in &sessions_here {
                    ui.push_id(("sess_sel", sid), |ui| {
                        let selected = state.active_session_id.as_deref() == Some(sid);
                        if ui.selectable_label(selected, slabel).clicked() && !selected {
                            // Move to end so reopened tabs appear after all open tabs.
                            if let Some(idx) = state.sessions.iter().position(|s| s.id == *sid) {
                                let mut sess = state.sessions.remove(idx);
                                sess.closed = false;
                                state.sessions.push(sess);
                            }
                            state.active_session_id = Some(sid.clone());
                        }
                    });
                }
            });
        // New session button next to the session picker.
        if super::buttons::lit_btn(ui, "+ Session", false).clicked() {
            state.new_session_for_project(Some(pid.clone()));
            autocode_ai::chat::ensure_session(state);
        }
    }
}

pub fn show_provider_picker(ui: &mut egui::Ui, state: &mut AppState) {
    egui::ComboBox::from_id_salt("provider_picker")
        .selected_text(
            RichText::new(&state.active_provider)
                .size(11.0)
                .color(Palette::TEXT_MUTED),
        )
        .show_ui(ui, |ui| {
            let keys: Vec<String> = state
                .providers
                .iter()
                .filter(|(_, p)| p.enabled)
                .map(|(k, _)| k.clone())
                .collect();
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
                            state.session_meta_dirty = true;
                        }
                    }
                });
            }
        });
}

pub fn show_model_picker(ui: &mut egui::Ui, state: &mut AppState) {
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
                .and_then(|p| autocode_core::helpers::provider_manifest(&p.kind))
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
                        if let Some(prov) = state.providers.get_mut(&state.active_provider) {
                            prov.model.clone_from(model_id);
                            prov.fill_from_config();
                        }
                        if let Some(sess) = state.active_session_mut() {
                            sess.model.clone_from(model_id);
                            state.session_meta_dirty = true;
                        }
                    }
                });
            }
        });
}
