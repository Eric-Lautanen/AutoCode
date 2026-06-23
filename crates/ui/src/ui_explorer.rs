// ui_explorer.rs -- File explorer side panel.
// Uses egui CollapsingHeader for directory nodes (native open/close triangles),
// selectable labels for files, and a floating code-viewer window.

use egui::{
    Color32, FontId, Frame, Key, Margin, RichText, ScrollArea, Stroke, TextEdit, TextureHandle,
};
use std::collections::HashSet;
use std::path::Path;

use crate::theme::{Palette, ROUND_SM};
use autocode_core::{utils::fsutil, state::AppState};
use autocode_fs::git::GitFileStatus;

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
    /// Editable buffer for the file viewer text content.
    file_edit_buffer: Option<String>,
    /// Whether the unsaved-changes confirmation dialog is open.
    show_close_confirm: bool,
    /// Ephemeral scroll offset for the file viewer — never persisted.
    /// Driven explicitly so egui never writes it to its ron memory store.
    viewer_scroll: egui::Vec2,
}

// -- Panel entry point ---------------------------------------------------------

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
                            let mut tree_state = TreeState {
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
                            show_tree(ui, root, &mut tree_state);
                        });
                });

            // Persist expanded dirs back to state.
            state.expanded_dirs = panel.expanded.iter().cloned().collect();
        });
}

// -- Recursive tree renderer ---------------------------------------------------

fn git_status_color(status: GitFileStatus) -> Color32 {
    match status {
        GitFileStatus::Modified => Palette::WARNING,
        GitFileStatus::Added | GitFileStatus::Untracked => Palette::SUCCESS,
        GitFileStatus::Deleted => Palette::ERROR,
        GitFileStatus::Renamed => Palette::PURPLE,
        GitFileStatus::Conflicted => Palette::ERROR,
    }
}

struct TreeState<'a> {
    expanded: &'a mut std::collections::HashSet<String>,
    selected: &'a mut Option<String>,
    file_content: &'a mut Option<Result<String, String>>,
    show_viewer: &'a mut bool,
    image_texture: &'a mut Option<(String, TextureHandle)>,
    renaming: &'a mut Option<String>,
    rename_buffer: &'a mut String,
    file_edit_buffer: &'a mut Option<String>,
    /// Repo root for git status, if available.
    repo_root: Option<std::path::PathBuf>,
    /// Cached git status: (file_statuses, dir_statuses).
    git_status: Option<(
        std::collections::HashMap<std::path::PathBuf, GitFileStatus>,
        std::collections::HashMap<std::path::PathBuf, GitFileStatus>,
    )>,
    /// Project root path used as namespace salt for widget IDs.
    root_path: String,
}

fn show_tree(ui: &mut egui::Ui, dir: &std::path::Path, state: &mut TreeState<'_>) {
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

    // Save original global style values so we can restore them after the window.
    let (orig_radius, orig_shadow, orig_margin) = {
        let s = ctx.global_style();
        (
            s.visuals.window_corner_radius,
            s.visuals.window_shadow,
            s.spacing.window_margin,
        )
    };
    // Force zero rounding on this window — CornerRadius::ZERO on the Frame
    // only controls fill/stroke; the shadow and clip shape still use
    // visuals.window_rounding, so we override that too.
    ctx.global_style_mut(|s| {
        s.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        s.visuals.window_shadow = egui::Shadow::NONE;
        s.spacing.window_margin = egui::Margin::ZERO;
    });

    let is_modified = match &panel.file_content {
        Some(Ok(original)) => panel.file_edit_buffer.as_deref() != Some(original.as_str()),
        _ => false,
    };

    let window_resp = egui::Window::new("file_viewer")
        .id(egui::Id::new("file_viewer_window"))
        .title_bar(false)
        .resizable(true)
        .default_size([960.0, 600.0])
        .min_size([320.0, 200.0])
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
                            let ctrl_s =
                                ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::S));
                            let save_clicked = is_modified
                                && ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Save").size(11.0).color(Palette::ACCENT),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .min_size(egui::Vec2::new(36.0, 20.0)),
                                    )
                                    .on_hover_text("Save changes (Ctrl+S)")
                                    .clicked();
                            if (save_clicked || (is_modified && ctrl_s))
                                && let Some(path) = &panel.selected_file
                                && let Some(buffer) = &panel.file_edit_buffer
                                && std::fs::write(path, buffer).is_ok()
                            {
                                panel.file_content = Some(Ok(buffer.clone()));
                                // Invalidate git cache so explorer colors update.
                                let saved = std::path::Path::new(path);
                                if let Some(parent) = saved.parent()
                                    && let Some(repo) =
                                        autocode_fs::explorer::find_project_root(parent)
                                {
                                    autocode_fs::git::invalidate_git_cache(&repo);
                                }
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
            } else if panel.file_content.as_ref().is_some_and(|r| r.is_ok()) {
                if let Some(buffer) = panel.file_edit_buffer.as_mut() {
                    let line_count = buffer.lines().count().max(1);
                    let digits = line_count.ilog10() as usize + 1;
                    let gutter_w = 12.0 + digits as f32 * 8.0;
                    let font_id = FontId::monospace(13.0);
                    let char_width = ui.fonts_mut(|f| f.glyph_width(&font_id, 'W'));
                    let max_line_len = buffer.lines().map(|l| l.chars().count()).max().unwrap_or(0);
                    let right_pad = 48.0;
                    // gutter_gap is the space between the gutter separator and the text column.
                    // It must exactly match the left Margin on the TextEdit below.
                    let gutter_gap = 8.0f32;
                    let max_content_width =
                        gutter_w + gutter_gap + max_line_len as f32 * char_width + right_pad;
                    let scroll_out = ScrollArea::both()
                        .id_salt("file_viewer_scroll")
                        .auto_shrink([false; 2])
                        .scroll_offset(panel.viewer_scroll)
                        .scroll_source(egui::scroll_area::ScrollSource {
                            drag: false,
                            ..Default::default()
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            ui.set_min_width(max_content_width);

                            let row_h = ui.fonts_mut(|f| f.row_height(&font_id));
                            let bottom_pad = 2.0 * row_h;

                            // Strategy: reserve the gutter column, then place the TextEdit
                            // immediately to its right using TextEdit::show() (not ui.add()).
                            // show() returns TextEditOutput::galley_pos — the exact screen-space
                            // origin where the galley is painted — and the galley itself with per-row
                            // layout. We use galley_pos.y as the y-anchor for gutter numbers,
                            // so they are always pixel-perfect regardless of margin/font rounding.
                            // We need to auto-scroll after the horizontal_top layout
                            // is fully complete — calling scroll_to_rect *inside* the
                            // closure clips the painter mid-draw and hides the gutter.
                            // Capture what we need here and scroll after the closure.
                            let mut scroll_target: Option<egui::Rect> = None;
                            let clip_rect_before = ui.clip_rect();

                            ui.horizontal_top(|ui| {
                                // Reserve the gutter column. Height uses row_h estimate — it only
                                // affects the painted gutter background, not the number positions.
                                let content_h = line_count as f32 * row_h;
                                let (gutter_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(gutter_w, content_h + bottom_pad),
                                    egui::Sense::hover(),
                                );

                                // Use a context-level painter with an explicit clip set to the
                                // gutter rect. ui.painter() is clipped to the current child's
                                // allocated rect at clone-time, which can be too small or already
                                // scrolled out of the visible area. Painting directly on the
                                // context layer bypasses that and always renders correctly.
                                let mut ctx_painter = ui.ctx().layer_painter(egui::LayerId::new(
                                    egui::Order::Middle,
                                    egui::Id::new("gutter_layer"),
                                ));
                                ctx_painter.set_clip_rect(clip_rect_before);
                                ctx_painter.rect_filled(
                                    gutter_rect,
                                    egui::CornerRadius::ZERO,
                                    Palette::BG_SURFACE,
                                );
                                ctx_painter.line_segment(
                                    [gutter_rect.right_top(), gutter_rect.right_bottom()],
                                    Stroke::new(1.0, Palette::BORDER),
                                );

                                // Zero out all TextEdit margins so galley_pos.x == te_rect.left().
                                // Setting left=gutter_gap here would shift the text right by that
                                // amount, but we already consumed gutter_w in allocate_exact_size,
                                // so we use Margin::ZERO and rely on the gutter allocation for spacing.
                                ui.add_space(gutter_gap);
                                let text_width = max_line_len as f32 * char_width + right_pad;
                                let te_output = TextEdit::multiline(buffer)
                                    .font(font_id.clone())
                                    .text_color(Palette::TEXT_CODE)
                                    .frame(egui::Frame::NONE)
                                    .margin(Margin::ZERO)
                                    .desired_width(text_width)
                                    .show(ui);

                                // galley_pos is the exact screen-space position of the galley origin.
                                // Each PlacedRow has a `pos: Pos2` giving its offset within the
                                // galley (per the row-relative coordinate system from PR #5411).
                                // Screen y = galley_pos.y + placed_row.pos.y.
                                let galley_pos = te_output.galley_pos;
                                for (i, row) in te_output.galley.rows.iter().enumerate() {
                                    ctx_painter.text(
                                        egui::pos2(
                                            gutter_rect.right() - 4.0,
                                            galley_pos.y + row.pos.y,
                                        ),
                                        egui::Align2::RIGHT_TOP,
                                        format!("{:>width$}", i + 1, width = digits),
                                        font_id.clone(),
                                        Palette::TEXT_MUTED,
                                    );
                                }

                                // Compute the scroll target while we still have te_output,
                                // but don't call scroll_to_rect yet — do it after this
                                // closure so layout is not disturbed.
                                let pointer_down = ui.input(|i| i.pointer.primary_down());
                                if pointer_down && let Some(cursor_range) = &te_output.cursor_range
                                {
                                    let primary = cursor_range.primary;
                                    let cursor_local = te_output.galley.pos_from_cursor(primary);
                                    let cursor_top = galley_pos.y + cursor_local.min.y;
                                    let cursor_bot = galley_pos.y + cursor_local.max.y;
                                    let clip = clip_rect_before;
                                    let near_top = cursor_top < clip.min.y + row_h;
                                    let near_bot = cursor_bot > clip.max.y - row_h;
                                    if near_top || near_bot {
                                        scroll_target = Some(egui::Rect::from_min_max(
                                            egui::pos2(clip.min.x, cursor_top),
                                            egui::pos2(clip.max.x, cursor_bot),
                                        ));
                                    }
                                }
                            });

                            // Safe to scroll now — layout and painting are complete.
                            if let Some(rect) = scroll_target {
                                ui.scroll_to_rect(rect, None);
                            }

                            ui.add_space(bottom_pad);
                        });
                    // Mirror the live offset back into our ephemeral field so the
                    // next frame's .scroll_offset() call starts from where the user
                    // actually is. Because we always override the ScrollArea with
                    // .scroll_offset(panel.viewer_scroll), egui's internally stored
                    // offset is overwritten every frame and never has a chance to
                    // diverge or leak into the ron persistence file.
                    panel.viewer_scroll = scroll_out.state.offset;
                }
            } else if let Some(Err(e)) = &panel.file_content {
                ui.add_space(12.0);
                ui.label(RichText::new(e).color(Palette::ERROR));
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No file selected")
                            .color(Palette::TEXT_MUTED)
                            .size(13.0),
                    );
                });
            }
        });

    // --- Click-outside-to-close detection ---
    // If the user clicked somewhere outside the file viewer window this frame,
    // either close immediately (no unsaved changes) or show the confirm dialog.
    let clicked_outside = ctx.input(|i| i.pointer.any_click())
        && window_resp
            .as_ref()
            .map(|r| {
                !r.response
                    .rect
                    .contains(ctx.input(|i| i.pointer.interact_pos().unwrap_or_default()))
            })
            .unwrap_or(false)
        && !panel.show_close_confirm;

    if clicked_outside {
        if is_modified {
            panel.show_close_confirm = true;
        } else {
            open = false;
        }
    }

    // X button / egui close signal
    if ctx.data_mut(|d| {
        d.remove_temp::<bool>(egui::Id::new("file_viewer_close"))
            .unwrap_or(false)
    }) {
        if is_modified {
            panel.show_close_confirm = true;
        } else {
            open = false;
        }
    }

    // --- Unsaved-changes confirmation modal ---
    if panel.show_close_confirm
        && let Some(viewer_rect) = window_resp.as_ref().map(|r| r.response.rect)
    {
        // Dialog box centered in the viewer.
        let dialog_size = egui::vec2(320.0, 148.0);
        let dialog_pos = viewer_rect.center() - dialog_size * 0.5;

        egui::Area::new(egui::Id::new("close_confirm_dialog"))
            .fixed_pos(dialog_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::NONE
                    .fill(Palette::BG_SURFACE)
                    .corner_radius(egui::CornerRadius::same(4))
                    .stroke(Stroke::new(1.0, Palette::BORDER))
                    .inner_margin(Margin::same(20))
                    .show(ui, |ui| {
                        ui.set_width(dialog_size.x - 40.0);
                        ui.spacing_mut().item_spacing.y = 12.0;

                        ui.label(
                            RichText::new("Unsaved changes")
                                .size(13.0)
                                .strong()
                                .color(Palette::TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new("You have unsaved changes. Save before closing?")
                                .size(12.0)
                                .color(Palette::TEXT_MUTED),
                        );

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            // Save & close
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Save & Close")
                                            .size(12.0)
                                            .color(Palette::BG_BASE),
                                    )
                                    .fill(Palette::ACCENT)
                                    .stroke(Stroke::NONE)
                                    .min_size(egui::vec2(96.0, 26.0)),
                                )
                                .clicked()
                            {
                                if let Some(path) = &panel.selected_file
                                    && let Some(buffer) = &panel.file_edit_buffer
                                {
                                    let _ = std::fs::write(path, buffer);
                                }
                                panel.show_close_confirm = false;
                                open = false;
                            }

                            // Discard & close
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Discard").size(12.0).color(Palette::ERROR),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, Palette::ERROR))
                                    .min_size(egui::vec2(72.0, 26.0)),
                                )
                                .clicked()
                            {
                                panel.show_close_confirm = false;
                                open = false;
                            }

                            // Cancel — go back to editing
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Cancel")
                                            .size(12.0)
                                            .color(Palette::TEXT_MUTED),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, Palette::BORDER))
                                    .min_size(egui::vec2(64.0, 26.0)),
                                )
                                .clicked()
                            {
                                panel.show_close_confirm = false;
                            }
                        });
                    });
            });
    }

    // Restore original global style values so other windows are not affected.
    ctx.global_style_mut(|s| {
        s.visuals.window_corner_radius = orig_radius;
        s.visuals.window_shadow = orig_shadow;
        s.spacing.window_margin = orig_margin;
    });

    if !open {
        panel.show_file_viewer = false;
        panel.show_close_confirm = false;
        panel.selected_file = None;
        panel.file_content = None;
        panel.image_texture = None;
        panel.file_edit_buffer = None;
        panel.viewer_scroll = egui::Vec2::ZERO;
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
