use egui::RichText;

use crate::theme::Palette;
use autocode_core::state::AppState;

pub fn lit_btn(ui: &mut egui::Ui, label: &str, lit: bool) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(if lit {
            Palette::ACCENT
        } else {
            Palette::TEXT_MUTED
        }))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(
            1.0,
            if lit {
                Palette::ACCENT
            } else {
                Palette::BORDER
            },
        )),
    )
}

pub fn show_handoff_toggle(ui: &mut egui::Ui, state: &mut AppState) {
    let looping_active = state.active_session().map(|s| s.looping_window).unwrap_or(false);
    let enabled = state.handoff_enabled;
    if looping_active {
        let resp = lit_btn(ui, "Handoff", false);
        resp.on_hover_text("Handoff is disabled while LRU pruning is active — LRU manages context by pruning old messages instead of handing off to a new session");
    } else {
        let resp = lit_btn(ui, "Handoff", enabled);
        if resp.clicked() {
            state.handoff_enabled = !enabled;
        }
        resp.on_hover_text(if enabled {
            "Handoff enabled — agent can call `handoff` to start a fresh session"
        } else {
            "Handoff disabled — context will fill until manual intervention"
        });
    }
}
