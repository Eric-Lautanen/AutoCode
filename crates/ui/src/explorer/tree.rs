// tree.rs -- Recursive file tree renderer with git status coloring.

use egui::{Color32, Frame, Key, Margin, Stroke, TextEdit};

use crate::theme::{Palette, ROUND_SM};
use autocode_core::utils::fsutil;
use autocode_fs::git::GitFileStatus;

pub(crate) fn git_status_color(status: GitFileStatus) -> Color32 {
    match status {
        GitFileStatus::Modified => Palette::WARNING,
        GitFileStatus::Added | GitFileStatus::Untracked => Palette::SUCCESS,
        GitFileStatus::Deleted => Palette::ERROR,
        GitFileStatus::Renamed => Palette::PURPLE,
        GitFileStatus::Conflicted => Palette::ERROR,
    }
}

pub(crate) struct TreeState<'a> {
    pub expanded: &'a mut std::collections::HashSet<String>,
    pub selected: &'a mut Option<String>,
    pub file_content: &'a mut Option<Result<String, String>>,
    pub show_viewer: &'a mut bool,
    pub image_texture: &'a mut Option<(String, egui::TextureHandle)>,
    pub renaming: &'a mut Option<String>,
    pub rename_buffer: &'a mut String,
    pub file_edit_buffer: &'a mut Option<String>,
    /// Repo root for git status, if available.
    pub repo_root: Option<std::path::PathBuf>,
    /// Cached git status: (file_statuses, dir_statuses).
    pub git_status: Option<(
        std::collections::HashMap<std::path::PathBuf, GitFileStatus>,
        std::collections::HashMap<std::path::PathBuf, GitFileStatus>,
    )>,
    /// Project root path used as namespace salt for widget IDs.
    pub root_path: String,
}

pub(crate) fn show_tree(ui: &mut egui::Ui, dir: &std::path::Path, state: &mut TreeState<'_>) {
    let entries = autocode_fs::explorer::list_dir_all(dir);

    let entries = if let Some((ref file_statuses, ref dir_statuses)) = state.git_status {
        if let Some(ref repo_root) = state.repo_root {
            autocode_fs::explorer::merge_git_status(
                entries,
                dir,
                repo_root,
                file_statuses,
                dir_statuses,
            )
        } else {
            entries
        }
    } else {
        entries
    };

    ui.spacing_mut().indent = 12.0;

    for entry in entries {
        let path_str = entry.path.to_string_lossy().to_string();

        if entry.is_dir {
            let is_open = state.expanded.contains(&path_str);
            let id = ui.make_persistent_id((&state.root_path, &path_str));

            let is_renaming = state.renaming.as_deref() == Some(&path_str);
            let mut header_clicked = false;
            let mut header_resp = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                is_open,
            )
            .show_header(ui, |ui| {
                if is_renaming {
                    if state.rename_buffer.is_empty() {
                        *state.rename_buffer = entry.name.clone();
                    }
                    let resp = ui.add_sized(
                        egui::vec2(ui.available_width(), 20.0),
                        TextEdit::singleline(state.rename_buffer).font(egui::TextStyle::Monospace),
                    );
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter
                        && !state.rename_buffer.is_empty()
                        && state.rename_buffer != &entry.name
                    {
                        let new_path = entry.path.with_file_name(state.rename_buffer.as_str());
                        let _ = fsutil::rename(&entry.path, &new_path);
                        if let Some(ref r) = state.repo_root {
                            autocode_fs::git::invalidate_git_cache(r);
                        }
                    }
                    let click_outside =
                        ui.input(|i| i.pointer.any_pressed()) && !resp.contains_pointer();
                    if enter || resp.lost_focus() || click_outside {
                        *state.renaming = None;
                        state.rename_buffer.clear();
                    }
                } else {
                    let item_frame = Frame::NONE
                        .corner_radius(ROUND_SM)
                        .inner_margin(Margin::symmetric(6, 3))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            let name_color = entry
                                .git_status
                                .map(git_status_color)
                                .unwrap_or(Palette::TEXT_SECONDARY);
                            ui.label(
                                egui::RichText::new(&entry.name)
                                    .size(12.0)
                                    .color(name_color),
                            );
                        });
                    let resp = item_frame
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        header_clicked = true;
                    }
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            resp.rect,
                            ROUND_SM,
                            Color32::from_rgba_premultiplied(33, 39, 50, 40),
                        );
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Copy path").clicked() {
                            ui.ctx().copy_text(path_str.clone());
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Rename").clicked() {
                            *state.renaming = Some(path_str.clone());
                            *state.selected = Some(path_str.clone());
                            state.rename_buffer.clear();
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .button(egui::RichText::new("Delete folder").color(Palette::ERROR))
                            .clicked()
                        {
                            let _ = fsutil::remove_dir(&entry.path);
                            if let Some(ref r) = state.repo_root {
                                autocode_fs::git::invalidate_git_cache(r);
                            }
                            if state.selected.as_deref() == Some(&path_str) {
                                *state.selected = None;
                                *state.file_content = None;
                                *state.show_viewer = false;
                            }
                            ui.close();
                        }
                    });
                }
            });
            if header_clicked {
                header_resp.toggle();
            }
            let now_open = header_resp.is_open();
            header_resp.body(|ui| {
                show_tree(ui, &entry.path, state);
            });
            if now_open {
                state.expanded.insert(path_str.clone());
            } else {
                state.expanded.remove(&path_str);
            }
        } else {
            ui.push_id(("file", &state.root_path, &path_str), |ui| {
                let is_selected = state.selected.as_deref() == Some(&path_str);
                let is_renaming = state.renaming.as_deref() == Some(&path_str);

                if is_renaming {
                    if state.rename_buffer.is_empty() {
                        *state.rename_buffer = entry.name.clone();
                    }
                    let resp = ui.add_sized(
                        egui::vec2(ui.available_width(), 20.0),
                        TextEdit::singleline(state.rename_buffer).font(egui::TextStyle::Monospace),
                    );
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter
                        && !state.rename_buffer.is_empty()
                        && state.rename_buffer != &entry.name
                    {
                        let new_path = entry.path.with_file_name(state.rename_buffer.as_str());
                        let _ = fsutil::rename(&entry.path, &new_path);
                        if let Some(ref r) = state.repo_root {
                            autocode_fs::git::invalidate_git_cache(r);
                        }
                    }
                    let click_outside =
                        ui.input(|i| i.pointer.any_pressed()) && !resp.contains_pointer();
                    if enter || resp.lost_focus() || click_outside {
                        *state.renaming = None;
                        state.rename_buffer.clear();
                    }
                } else {
                    let (bg_fill, border_color) = if is_selected {
                        (Palette::BG_ACTIVE, Palette::ACCENT_DIM)
                    } else {
                        (Color32::TRANSPARENT, Color32::TRANSPARENT)
                    };

                    let item_frame = Frame::NONE
                        .fill(bg_fill)
                        .corner_radius(ROUND_SM)
                        .stroke(Stroke::new(1.0, border_color))
                        .inner_margin(Margin {
                            left: 6,
                            right: 6,
                            top: 3,
                            bottom: 3,
                        })
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            let text =
                                egui::RichText::new(&entry.name)
                                    .size(12.0)
                                    .color(if is_selected {
                                        Palette::ACCENT
                                    } else {
                                        entry
                                            .git_status
                                            .map(git_status_color)
                                            .unwrap_or(Palette::TEXT_SECONDARY)
                                    });
                            ui.label(text);
                        });

                    let resp = item_frame
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);

                    let is_deleted = entry.git_status == Some(GitFileStatus::Deleted);
                    if resp.clicked() && !is_deleted {
                        *state.selected = Some(path_str.clone());
                        *state.image_texture = None;
                        let ext = entry
                            .path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default();
                        let is_image = matches!(
                            ext.as_str(),
                            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
                        );
                        if is_image {
                            *state.file_content = None;
                        } else {
                            let result = autocode_fs::explorer::read_file(&entry.path);
                            *state.file_edit_buffer = result.as_ref().ok().cloned();
                            *state.file_content = Some(result);
                        }
                        *state.show_viewer = true;
                    }

                    if resp.hovered() && !is_selected {
                        ui.painter().rect_filled(
                            resp.rect,
                            ROUND_SM,
                            Color32::from_rgba_premultiplied(33, 39, 50, 40),
                        );
                    }

                    if !is_deleted {
                        resp.context_menu(|ui| {
                            if ui.button("Copy path").clicked() {
                                ui.ctx().copy_text(path_str.clone());
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Rename").clicked() {
                                *state.renaming = Some(path_str.clone());
                                *state.selected = Some(path_str.clone());
                                state.rename_buffer.clear();
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .button(egui::RichText::new("Delete file").color(Palette::ERROR))
                                .clicked()
                            {
                                let _ = fsutil::remove_file(&entry.path);
                                if let Some(ref r) = state.repo_root {
                                    autocode_fs::git::invalidate_git_cache(r);
                                }
                                if state.selected.as_deref() == Some(&path_str) {
                                    *state.selected = None;
                                    *state.file_content = None;
                                    *state.show_viewer = false;
                                }
                                ui.close();
                            }
                        });
                    }
                }
            }); // end push_id("file", path_str)
        }
    }
}
