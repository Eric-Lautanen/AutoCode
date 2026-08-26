// chat/attachments.rs -- Attachment chips, staging, and thumbnail textures.

use std::path::PathBuf;

use egui::load::SizedTexture;
use egui::{Frame, TextureHandle, Vec2};

use autocode_core::state::{AppState, Attachment, AttachmentKind};

use crate::chat::ChatPanelState;
use crate::chat::theme;
use crate::theme::{Palette, ROUND_SM};

/// Texture cache bound (audit risk 4: bound growth with many image chips).
const MAX_CACHED_TEXTURES: usize = 16;

/// Stage picked/dropped files into the active session and append them to the
/// pending draft chips. Returns human-readable rejection messages.
pub(crate) fn stage_paths(
    state: &mut AppState,
    panel_state: &mut ChatPanelState,
    paths: &[String],
) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(sid) = state.active_session_id.clone() else {
        return vec!["Pick a project and session before attaching files.".to_string()];
    };
    let Some(sess_idx) = state.sessions.iter().position(|s| s.id == sid) else {
        return vec!["Active session not found.".to_string()];
    };
    let proj = state.sessions[sess_idx]
        .project_id
        .clone()
        .and_then(|pid| state.projects.iter().find(|p| p.id == pid).cloned());
    let Some(proj) = proj else {
        return vec!["Active session has no project.".to_string()];
    };

    for path in paths {
        let p = PathBuf::from(path);
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let kind = match autocode_core::storage::classify(&name) {
            autocode_core::storage::AttClass::Image => AttachmentKind::Image,
            _ => AttachmentKind::File,
        };
        // D5: reject decoded images beyond 20 MP at stage time.
        if kind == AttachmentKind::Image
            && let Ok(dims) = image::ImageReader::open(&p)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
                .and_then(|r| {
                    r.into_dimensions()
                        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
                })
            && (dims.0 as u64) * (dims.1 as u64) > 20_000_000
        {
            errors.push(format!(
                "{} is {}x{} pixels -- images are capped at 20 MP",
                name, dims.0, dims.1
            ));
            continue;
        }
        let total: u64 = panel_state
            .pending_attachments
            .iter()
            .map(|a| a.bytes)
            .sum();
        // Borrow state immutably only inside stage_file.
        match autocode_core::storage::stage_file(&proj, &state.sessions[sess_idx], &p, kind, total)
        {
            Ok(att) => panel_state.pending_attachments.push(att),
            Err(e) => errors.push(e),
        }
    }
    errors
}

/// Drop a staged chip and best-effort remove its staged copy.
pub(crate) fn remove_chip(state: &mut AppState, panel_state: &mut ChatPanelState, index: usize) {
    if index >= panel_state.pending_attachments.len() {
        return;
    }
    let att = panel_state.pending_attachments.remove(index);
    panel_state
        .attachment_textures
        .remove(&(att.rel_path.clone(), att.bytes));
    if let Some(sess) = state.active_session()
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
    {
        let path = autocode_core::storage::resolve_path(proj, sess, &att);
        let _ = autocode_core::utils::fsutil::remove_file(&path);
    }
}

fn load_texture(
    ctx: &egui::Context,
    panel_state: &mut ChatPanelState,
    att: &Attachment,
    abs_path: Option<PathBuf>,
) -> Option<TextureHandle> {
    let key = (att.rel_path.clone(), att.bytes);
    if let Some(tex) = panel_state.attachment_textures.get(&key) {
        return Some(tex.clone());
    }
    let abs_path = abs_path?;
    let bytes = std::fs::read(autocode_core::utils::fsutil::extended_path(
        abs_path.as_path(),
    ))
    .ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    // Thumbnails decode small so history rendering stays cheap.
    let thumb = img.thumbnail(72, 72);
    let size_pixels = [thumb.width() as usize, thumb.height() as usize];
    let pixels = thumb.into_rgba8().into_raw();
    let tex = ctx.load_texture(
        format!("att_{}_{}", key.0, key.1),
        egui::ColorImage::from_rgba_unmultiplied(size_pixels, &pixels),
        Default::default(),
    );
    // FIFO eviction keeps the cache bounded.
    while panel_state.attachment_textures.len() >= MAX_CACHED_TEXTURES {
        let oldest = match panel_state.attachment_textures.keys().next() {
            Some(k) => k.clone(),
            None => break,
        };
        panel_state.attachment_textures.remove(&oldest);
    }
    panel_state.attachment_textures.insert(key, tex.clone());
    Some(tex)
}

/// Render the pending-attachment chips row above the input box.
pub(crate) fn show_pending_chips(
    ui: &mut egui::Ui,
    state: &mut AppState,
    panel_state: &mut ChatPanelState,
) {
    if panel_state.pending_attachments.is_empty() {
        return;
    }
    ui.add_space(6.0);
    let att_dir: Option<PathBuf> = state.active_session().and_then(|sess| {
        sess.project_id.as_ref().and_then(|pid| {
            state
                .projects
                .iter()
                .find(|p| &p.id == pid)
                .map(|proj| autocode_core::storage::session_messages_dir(proj, sess))
        })
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        let count = panel_state.pending_attachments.len();
        for i in (0..count).rev() {
            let att = panel_state.pending_attachments[i].clone();
            let abs = att_dir.clone().map(|d| d.join(&att.rel_path));
            Frame::NONE
                .fill(theme().bg_surface)
                .corner_radius(ROUND_SM)
                .stroke(egui::Stroke::new(1.0, theme().border))
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if att.kind == AttachmentKind::Image {
                            let tex = load_texture(ui.ctx(), panel_state, &att, abs);
                            if let Some(tex) = tex {
                                ui.image(SizedTexture::new(&tex, Vec2::new(36.0, 36.0)));
                            }
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(shorten_name(&att.name, 28))
                                    .size(10.5)
                                    .color(theme().text_primary),
                            );
                            ui.label(
                                egui::RichText::new(format!("{} KB", att.bytes.max(1) / 1024))
                                    .size(9.5)
                                    .color(theme().text_muted),
                            );
                        });
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("X")
                                        .size(10.0)
                                        .color(Palette::TEXT_MUTED),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .min_size(Vec2::new(18.0, 18.0)),
                            )
                            .clicked()
                        {
                            remove_chip(state, panel_state, i);
                        }
                    });
                });
        }
    });
}

fn shorten_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_string();
    }
    let head: String = name.chars().take(max / 2 - 1).collect();
    let tail: String = {
        let all: Vec<char> = name.chars().collect();
        all[all.len() - (max / 2 - 2)..].iter().collect()
    };
    format!("{}\u{2026}{}", head, tail)
}

/// Inline thumbnails + labels inside a sent user bubble (history view).
pub(crate) fn show_bubble_attachments(
    ui: &mut egui::Ui,
    msg: &autocode_core::state::ChatMessage,
    panel_state: &mut ChatPanelState,
    att_dir: Option<PathBuf>,
) {
    if msg.attachments.is_empty() {
        return;
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        for att in &msg.attachments {
            let abs = att_dir.clone().map(|d| d.join(&att.rel_path));
            Frame::NONE
                .fill(theme().bg_base)
                .corner_radius(ROUND_SM)
                .stroke(egui::Stroke::new(1.0, theme().border))
                .inner_margin(egui::Margin::symmetric(5, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if att.kind == AttachmentKind::Image {
                            let tex = load_texture(ui.ctx(), panel_state, att, abs);
                            if let Some(tex) = tex {
                                ui.image(SizedTexture::new(&tex, Vec2::new(48.0, 48.0)));
                            }
                        } else {
                            ui.label(egui::RichText::new("\u{1F4C4}").size(14.0));
                        }
                        ui.label(egui::RichText::new(shorten_name(&att.name, 24)).size(10.0));
                    });
                });
        }
    });
}
