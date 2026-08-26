use egui::{RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::helpers;
use crate::theme::Palette;
use autocode_core::{helpers as core_helpers, state::AppState};

pub fn show_token_meter(ui: &mut egui::Ui, state: &AppState, frac: f32) {
    let meter_w = 88.0;
    let meter_h = 6.0;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(meter_w, meter_h), Sense::hover());
    let painter = ui.painter();

    // Track.
    painter.rect_filled(rect, egui::CornerRadius::same(3), Palette::BG_SURFACE);

    // Fill.
    let fill_color = if frac > 0.85 {
        Palette::ERROR
    } else if frac > 0.65 {
        Palette::WARNING
    } else {
        Palette::SUCCESS
    };
    let fill_rect =
        egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * frac, rect.height()));
    painter.rect_filled(fill_rect, egui::CornerRadius::same(3), fill_color);

    // Outline.
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(3),
        Stroke::new(1.0, Palette::BORDER),
        StrokeKind::Outside,
    );

    resp.on_hover_text({
        let mut tip = format!("{:.0}% context used", frac * 100.0);
        if let Some(sid) = state.active_session_id.as_ref()
            && let Some(sess) = state.sessions.iter().find(|s| s.id == *sid)
            && sess.looping_window
        {
            let agg = state
                .sessions
                .iter()
                .find(|s| s.id == *sid)
                .and_then(|s| {
                    let label = if !s.provider_label.is_empty() {
                        &s.provider_label
                    } else {
                        &state.active_provider
                    };
                    state.providers.get(label)
                })
                .and_then(|p| {
                    p.models_config
                        .as_ref()
                        .and_then(|mc| mc.get(&p.model))
                        .map(|m| m.loop_aggressiveness)
                })
                .unwrap_or_default();
            tip.push_str(&format!(
                "\nLRU: {} — triggers at {:.0}% context full",
                agg.label(),
                agg.trigger_pct() * 100.0
            ));
        }
        tip
    });

    ui.add_space(4.0);
    let usage_resp = ui.add(
        egui::Label::new(
            RichText::new(core_helpers::usage_display(state))
                .size(10.0)
                .color(Palette::TEXT_MUTED),
        )
        .sense(Sense::hover()),
    );
    if usage_resp.clone().hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    usage_resp.on_hover_text(
        "Context tokens as reported by the provider's last API response.\nUpdated once per completed request.",
    );
}

pub fn show_network_status(ui: &mut egui::Ui, net: &mut autocode_ai::chat::NetworkStatus) {
    let (dot, dot_kind) = net.blink_dot();
    let byte_str = net.format_bytes();

    let show = net.active || !byte_str.is_empty();

    helpers::toolbar_separator(ui);

    let dot_color = if show {
        match dot_kind {
            autocode_ai::chat::BlinkKind::Inactive => Palette::TEXT_MUTED,
            autocode_ai::chat::BlinkKind::Active => Palette::SUCCESS,
            autocode_ai::chat::BlinkKind::Stalled => Palette::ERROR,
        }
    } else {
        Palette::BG_BASE
    };
    let dot_text = RichText::new(dot.to_string())
        .size(10.0)
        .color(dot_color)
        .monospace();
    let dot_resp = ui.add_sized(
        egui::Vec2::new(14.0, 20.0),
        egui::Label::new(dot_text).sense(egui::Sense::hover()),
    );
    if dot_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if show {
        if net.active {
            let tip = match net.idle_secs {
                Some(s) => format!(
                    "Stream idle: {}s\nStall timeout at your Stream Idle setting (Settings)",
                    s
                ),
                None => "Waiting for response...".to_string(),
            };
            dot_resp.on_hover_text(tip);
        } else if net.stalled {
            dot_resp.on_hover_text("Connection stalled");
        }
    }

    if !byte_str.is_empty() {
        ui.label(RichText::new(byte_str).size(10.0).color(if net.stalled {
            Palette::ERROR
        } else {
            Palette::TEXT_MUTED
        }));
    } else {
        ui.label(RichText::new(" ").size(10.0));
    }
}
