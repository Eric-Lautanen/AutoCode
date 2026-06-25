// viewer.rs -- Floating file viewer window with text/image preview and save support.

use egui::{
    Color32, FontId, Frame, Key, Margin, RichText, ScrollArea, Stroke, TextEdit, TextureHandle,
};

use crate::theme::Palette;

use super::ExplorerPanelState;

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
