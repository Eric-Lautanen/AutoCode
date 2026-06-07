// ui_explorer.rs -- File explorer side panel.
// Uses egui CollapsingHeader for directory nodes (native open/close triangles),
// selectable labels for files, and a floating code-viewer window.

use egui::{Color32, Frame, Key, Margin, RichText, ScrollArea, Stroke, TextEdit, TextureHandle};
use std::collections::HashSet;
use std::path::Path;

use autocode_core::{
    fsutil,
    state::AppState,
    theme::{Palette, ROUND_SM},
};

/// Ephemeral (non-persisted) state for the explorer panel.
#[derive(Default)]
pub struct ExplorerPanelState {
    expanded: HashSet<String>,
    selected_file: Option<String>,
    file_content: Option<Result<String, String>>,
    show_file_viewer: bool,
    image_texture: Option<(String, TextureHandle)>,
    /// Path of the item currently being renamed, or None.
    renaming: Option<String>,
    /// Current text in the rename input (persisted across frames).
    rename_buffer: String,
}

// -- Panel entry point ---------------------------------------------------------

pub fn show(ui: &mut egui::Ui, state: &mut AppState, panel: &mut ExplorerPanelState) {
    // Sync expanded dirs from persistent state into ephemeral set.
    for p in &state.expanded_dirs {
        panel.expanded.insert(p.clone());
    }

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
                    }
                });
            });

            ui.add_space(4.0);
            ui.add(egui::Separator::default().shrink(0.0));
            ui.add_space(4.0);

            // -- No-project placeholder --------------------------------
            let root_path = match state.active_project().map(|p| p.root_path.clone()) {
                Some(r) => r,
                None => {
                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("No project open.\nAdd one in Settings > Projects.")
                                .size(11.0)
                                .color(Palette::TEXT_MUTED),
                        );
                    });
                    return;
                }
            };

            let root = Path::new(&root_path);

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

                    ScrollArea::vertical()
                        .id_salt("explorer_scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            // Tighter row spacing inside the tree.
                            ui.spacing_mut().item_spacing.y = 1.0;
                            let mut tree_state = TreeState {
                                expanded: &mut panel.expanded,
                                selected: &mut panel.selected_file,
                                file_content: &mut panel.file_content,
                                show_viewer: &mut panel.show_file_viewer,
                                image_texture: &mut panel.image_texture,
                                renaming: &mut panel.renaming,
                                rename_buffer: &mut panel.rename_buffer,
                            };
                            show_tree(ui, root, &mut tree_state);
                        });
                });

            // Persist expanded dirs back to state.
            state.expanded_dirs = panel.expanded.iter().cloned().collect();
        });
}

// -- Recursive tree renderer ---------------------------------------------------

struct TreeState<'a> {
    expanded: &'a mut std::collections::HashSet<String>,
    selected: &'a mut Option<String>,
    file_content: &'a mut Option<Result<String, String>>,
    show_viewer: &'a mut bool,
    image_texture: &'a mut Option<(String, TextureHandle)>,
    renaming: &'a mut Option<String>,
    rename_buffer: &'a mut String,
}

fn show_tree(ui: &mut egui::Ui, dir: &std::path::Path, state: &mut TreeState<'_>) {
    let entries = autocode_fs::explorer::list_dir(dir);

    ui.spacing_mut().indent = 12.0;

    for entry in entries {
        let path_str = entry.path.to_string_lossy().to_string();

        if entry.is_dir {
            let is_open = state.expanded.contains(&path_str);
            let id = ui.make_persistent_id(&path_str);

            let is_renaming = state.renaming.as_deref() == Some(&path_str);
            let header_resp = egui::collapsing_header::CollapsingState::load_with_default_open(
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
                        ui.available_size(),
                        TextEdit::singleline(state.rename_buffer).font(egui::TextStyle::Monospace),
                    );
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter
                        && !state.rename_buffer.is_empty()
                        && state.rename_buffer != &entry.name
                    {
                        let new_path = entry.path.with_file_name(state.rename_buffer.as_str());
                        let _ = fsutil::rename(&entry.path, &new_path);
                        *state.selected = Some(new_path.to_string_lossy().to_string());
                    }
                    let click_outside =
                        ui.input(|i| i.pointer.any_pressed()) && !resp.contains_pointer();
                    if enter || resp.lost_focus() || click_outside {
                        *state.renaming = None;
                        *state.selected = None;
                        state.rename_buffer.clear();
                    }
                } else {
                    let item_frame = Frame::NONE
                        .corner_radius(ROUND_SM)
                        .inner_margin(Margin::symmetric(6, 3))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(
                                egui::RichText::new(&entry.name)
                                    .size(12.0)
                                    .color(autocode_core::theme::Palette::TEXT_SECONDARY),
                            );
                        });
                    let resp = item_frame
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
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
                            .button(
                                egui::RichText::new("Delete folder")
                                    .color(autocode_core::theme::Palette::ERROR),
                            )
                            .clicked()
                        {
                            let _ = fsutil::remove_dir(&entry.path);
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
            ui.push_id(("file", &path_str), |ui| {
                let is_selected = state.selected.as_deref() == Some(&path_str);
                let is_renaming = state.renaming.as_deref() == Some(&path_str);

                if is_renaming {
                    if state.rename_buffer.is_empty() {
                        *state.rename_buffer = entry.name.clone();
                    }
                    let resp = ui.add_sized(
                        ui.available_size(),
                        TextEdit::singleline(state.rename_buffer).font(egui::TextStyle::Monospace),
                    );
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter
                        && !state.rename_buffer.is_empty()
                        && state.rename_buffer != &entry.name
                    {
                        let new_path = entry.path.with_file_name(state.rename_buffer.as_str());
                        let _ = fsutil::rename(&entry.path, &new_path);
                        *state.selected = Some(new_path.to_string_lossy().to_string());
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
                                        Palette::TEXT_SECONDARY
                                    });
                            ui.label(text);
                        });

                    let resp = item_frame
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);

                    if resp.clicked() {
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
                            *state.file_content =
                                Some(autocode_fs::explorer::read_file(&entry.path));
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
                            .button(
                                egui::RichText::new("Delete file")
                                    .color(autocode_core::theme::Palette::ERROR),
                            )
                            .clicked()
                        {
                            let _ = fsutil::remove_file(&entry.path);
                            if state.selected.as_deref() == Some(&path_str) {
                                *state.selected = None;
                                *state.file_content = None;
                                *state.show_viewer = false;
                            }
                            ui.close();
                        }
                    });
                }
            }); // end push_id("file", path_str)
        }
    }
}

// -- Floating file viewer window -----------------------------------------------

pub fn show_file_viewer(ctx: &egui::Context, panel: &mut ExplorerPanelState) {
    if !panel.show_file_viewer {
        return;
    }

    // Store open state in ctx.data so the chat input focus logic
    // can check if a popup window is currently open.
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("file_viewer_open"), true);
    });

    let mut open = panel.show_file_viewer;

    // Force zero rounding on this window — CornerRadius::ZERO on the Frame
    // only controls fill/stroke; the shadow and clip shape still use
    // visuals.window_rounding, so we override that too.
    ctx.global_style_mut(|s| {
        s.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        s.visuals.window_shadow = egui::Shadow::NONE;
        s.spacing.window_margin = egui::Margin::ZERO;
    });

    egui::Window::new("file_viewer")
        .id(egui::Id::new("file_viewer_window"))
        .open(&mut open)
        .title_bar(false)
        .resizable(true)
        .default_size([720.0, 400.0])
        .min_size([320.0, 200.0])
        .max_size([f32::INFINITY, 400.0])
        .default_pos([200.0, 120.0])
        .frame(
            Frame::NONE
                .fill(Palette::BG_BASE)
                .corner_radius(egui::CornerRadius::ZERO)
                .stroke(Stroke::new(1.0, Palette::BORDER))
                .inner_margin(Margin::same(0)),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let file_path = panel.selected_file.as_deref().unwrap_or("");
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);

            Frame::NONE
                .fill(Palette::BG_SURFACE)
                .corner_radius(egui::CornerRadius::ZERO)
                .inner_margin(Margin {
                    left: 12,
                    right: 8,
                    top: 10,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("[file]").size(14.0).color(Palette::ACCENT));
                        ui.label(
                            RichText::new(file_name)
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
                                    .min_size(egui::Vec2::new(20.0, 20.0)),
                                )
                                .on_hover_text("Close")
                                .clicked()
                            {
                                ui.data_mut(|d| {
                                    d.insert_temp(egui::Id::new("file_viewer_close"), true);
                                });
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Copy").size(11.0).color(Palette::TEXT_MUTED),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE)
                                    .min_size(egui::Vec2::new(36.0, 20.0)),
                                )
                                .on_hover_text("Copy file path")
                                .clicked()
                            {
                                ui.ctx().copy_text(file_path.to_string());
                            }
                        });
                    });
                });

            // Render image or text content.
            // We check file_content first to avoid simultaneous borrows.
            let is_image = panel.file_content.is_none()
                && panel.selected_file.as_deref().is_some_and(|p| {
                    let ext = std::path::Path::new(p)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    matches!(
                        ext.to_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
                    )
                });

            if is_image {
                if let Some(path) = &panel.selected_file.clone() {
                    if let Some(tex) = render_image(ui.ctx(), path, panel) {
                        ui.centered_and_justified(|ui| {
                            let avail = ui.available_size();
                            let img_size = tex.size_vec2();
                            let scale = (avail.x / img_size.x)
                                .min(avail.y / img_size.y)
                                .clamp(0.1, 2.0);
                            let display_size = img_size * scale;
                            ui.add(egui::Image::from_texture(tex).fit_to_exact_size(display_size));
                        });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("Could not render image").color(Palette::ERROR));
                        });
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("No file selected")
                                .color(Palette::TEXT_MUTED)
                                .size(13.0),
                        );
                    });
                }
            } else {
                match &panel.file_content {
                    Some(Ok(content)) => {
                        let mut text = content.clone();
                        ScrollArea::both()
                            .id_salt("file_viewer_scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add_sized(
                                    ui.available_size(),
                                    egui::TextEdit::multiline(&mut text)
                                        .id(egui::Id::new(("file_viewer_text", file_path)))
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .interactive(false)
                                        .text_color(Palette::TEXT_CODE),
                                );
                            });
                    }
                    Some(Err(e)) => {
                        ui.add_space(12.0);
                        ui.label(RichText::new(e).color(Palette::ERROR));
                    }
                    None => {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("No file selected")
                                    .color(Palette::TEXT_MUTED)
                                    .size(13.0),
                            );
                        });
                    }
                }
            }
        });

    if ctx.data_mut(|d| {
        d.remove_temp::<bool>(egui::Id::new("file_viewer_close"))
            .unwrap_or(false)
    }) {
        open = false;
    }

    if !open {
        panel.show_file_viewer = false;
        panel.selected_file = None;
        panel.file_content = None;
        panel.image_texture = None;
        // Clear the open flag so the chat input can reclaim focus.
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("file_viewer_open"), false);
            d.insert_temp(egui::Id::new("popup_just_closed"), true);
        });
    }
}

/// Decode and cache an image texture for the given file path.
/// Returns None if the file is not an image or decoding fails.
fn render_image<'a>(
    ctx: &egui::Context,
    path: &str,
    panel: &'a mut ExplorerPanelState,
) -> Option<&'a TextureHandle> {
    // Check cache (immutable borrow first, released before mutation below).
    if panel.image_texture.as_ref().is_some_and(|(p, _)| p == path) {
        return panel.image_texture.as_ref().map(|(_, t)| t);
    }

    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
    ) {
        return None;
    }

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return None,
    };

    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i.to_rgba8(),
        Err(_) => return None,
    };
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();

    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    let texture = ctx.load_texture(path, color_image, egui::TextureOptions::LINEAR);
    panel.image_texture = Some((path.to_string(), texture));
    panel.image_texture.as_ref().map(|(_, t)| t)
}
