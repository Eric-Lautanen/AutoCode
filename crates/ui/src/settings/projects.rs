use crate::helpers;
use crate::theme::{Palette, ROUND_SM};
use autocode_core::state::{AppState, Project, Session};
use egui::{CollapsingHeader, Frame, Margin, RichText};

pub fn show_projects(ui: &mut egui::Ui, state: &mut AppState) {
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
                                autocode_core::storage::switch_to_project(state, &p.id);
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
                                && autocode_core::storage::session_exists(p, s)
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

                    // Project-level thinking defaults for new sessions.
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Thinking defaults for new sessions:")
                            .size(11.0)
                            .color(Palette::TEXT_MUTED),
                    );
                    ui.horizontal(|ui| {
                        let mut meta =
                            autocode_core::storage::load_project_meta(p).unwrap_or_default();
                        let mut thinking = meta.project_thinking_mode;
                        let changed_thinking = ui.checkbox(&mut thinking, "Thinking").changed();
                        let mut effort_buf = meta.project_reasoning_effort.clone();
                        let effort_resp = egui::ComboBox::from_id_salt(("proj_effort", &p.id))
                            .selected_text(if effort_buf.is_empty() {
                                "default".to_string()
                            } else {
                                effort_buf.clone()
                            })
                            .show_ui(ui, |ui| {
                                for label in ["high", "medium", "low", "max"] {
                                    ui.selectable_value(&mut effort_buf, label.to_string(), label);
                                }
                                ui.selectable_value(
                                    &mut effort_buf,
                                    String::new(),
                                    "(model default)",
                                );
                            });
                        let changed_effort = effort_resp.response.changed()
                            || effort_buf != meta.project_reasoning_effort;
                        if changed_thinking || changed_effort {
                            meta.project_thinking_mode = thinking;
                            meta.project_reasoning_effort = effort_buf;
                            let _ = autocode_core::storage::save_project_meta(p, &meta);
                        }
                    });
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
                let _ = autocode_core::storage::save_session_meta(proj, s);
            }
        }
    }
    for sid in delete_ops {
        autocode_ai::chat::delete_session(state, &sid);
    }
    if let Some(pid) = delete_all_for_project {
        let ids: Vec<String> = state
            .sessions
            .iter()
            .filter(|s| s.project_id.as_deref() == Some(&pid))
            .map(|s| s.id.clone())
            .collect();
        for sid in ids {
            autocode_ai::chat::delete_session(state, &sid);
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
            .map(autocode_core::storage::project_sessions_dir);
        for sid in sess_ids {
            autocode_ai::chat::delete_session(state, &sid);
        }
        if let Some(dir) = proj_dir {
            let _ = autocode_core::utils::fsutil::remove_dir(&dir);
        }
        state.projects.retain(|p| p.id != id);
        if state.active_project_id.as_deref() == Some(&id) {
            state.active_project_id = state.projects.last().map(|p| p.id.clone());
            state.active_session_id = None;
        }
    }
}
