// panel.rs -- Explorer side panel entry point.

use egui::{Frame, Margin, RichText, ScrollArea};
use std::path::Path;

use crate::theme::Palette;
use autocode_core::state::AppState;

use super::{ExplorerPanelState, tree};

pub fn show(ui: &mut egui::Ui, state: &mut AppState, panel: &mut ExplorerPanelState) {
    // Sync expanded dirs from persistent state into ephemeral set.
    for p in &state.expanded_dirs {
        panel.expanded.insert(p.clone());
    }

    // Resolve project root early so the Refresh button can use it.
    let root_path = match state.active_project().map(|p| p.root_path.clone()) {
        Some(r) => r,
        None => {
            Frame::NONE
                .fill(Palette::BG_PANEL)
                .inner_margin(Margin {
                    left: 0,
                    right: 0,
                    top: 12,
                    bottom: 9,
                })
                .show(ui, |ui| {
                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("No project open.\nAdd one in Settings > Projects.")
                                .size(11.0)
                                .color(Palette::TEXT_MUTED),
                        );
                    });
                });
            return;
        }
    };
    let root = Path::new(&root_path);
    let repo_root = autocode_fs::explorer::find_project_root(root);
    let git_status = repo_root
        .as_ref()
        .and_then(|r| autocode_fs::git::get_cached_git_status(r));

    Frame::NONE
        .fill(Palette::BG_PANEL)
        .inner_margin(Margin {
            left: 0,
            right: 0,
            top: 12,
            bottom: 9,
        })
        .show(ui, |ui| {
            // -- Header row --------------------------------------------
            ui.horizontal(|ui| {
                ui.add_space(5.0);
                ui.label(
                    RichText::new("EXPLORER")
                        .size(10.0)
                        .color(Palette::TEXT_MUTED)
                        .strong(),
                );
                ui.add_space(5.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    if ui
                        .small_button(
                            RichText::new("Refresh")
                                .size(10.0)
                                .color(Palette::TEXT_MUTED),
                        )
                        .on_hover_text("Clear selection and refresh")
                        .clicked()
                    {
                        panel.selected_file = None;
                        panel.file_content = None;
                        if let Some(ref r) = repo_root {
                            autocode_fs::git::invalidate_git_cache(r);
                        }
                    }
                });
            });

            ui.add_space(4.0);
            ui.add(egui::Separator::default().shrink(0.0));
            ui.add_space(4.0);

            // -- File tree (with root label at top) ---------------------
            Frame::NONE
                .inner_margin(Margin {
                    left: 10,
                    right: 10,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    // -- Project root label at top inside frame ---------
                    ui.label(
                        RichText::new(
                            root.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&root_path),
                        )
                        .size(12.0)
                        .color(Palette::ACCENT)
                        .strong(),
                    );
                    ui.add_space(4.0);

                    ScrollArea::both()
                        .id_salt(("explorer_scroll", &root_path))
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            // Tighter row spacing inside the tree.
                            ui.spacing_mut().item_spacing.y = 1.0;
                            let mut tree_state = tree::TreeState {
                                expanded: &mut panel.expanded,
                                selected: &mut panel.selected_file,
                                file_content: &mut panel.file_content,
                                show_viewer: &mut panel.show_file_viewer,
                                image_texture: &mut panel.image_texture,
                                renaming: &mut panel.renaming,
                                rename_buffer: &mut panel.rename_buffer,
                                file_edit_buffer: &mut panel.file_edit_buffer,
                                repo_root,
                                git_status,
                                root_path: root_path.clone(),
                            };
                            tree::show_tree(ui, root, &mut tree_state);
                        });
                });

            // Persist expanded dirs back to state.
            state.expanded_dirs = panel.expanded.iter().cloned().collect();
        });
}
