// widgets.rs -- Shared UI widget helpers (toolbar, settings, todo scroll).

use egui::RichText;

use crate::theme::Palette;
use autocode_core::state::TodoItem;

pub fn toolbar_separator(ui: &mut egui::Ui) {
    ui.add(egui::Separator::default().vertical().spacing(8.0));
}

pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(8.0);
}

pub fn field_label(text: &str) -> RichText {
    RichText::new(text).size(11.5).color(Palette::TEXT_MUTED)
}

pub fn todo_scroll_area(
    ui: &mut egui::Ui,
    items: &[TodoItem],
    full_w: f32,
    scroll_target_idx: Option<usize>,
    render_item: impl Fn(&mut egui::Ui, &TodoItem, f32),
    render_empty: impl FnOnce(&mut egui::Ui),
) {
    egui::ScrollArea::vertical()
        .max_height(500.0)
        .show(ui, |ui: &mut egui::Ui| {
            ui.set_min_width(full_w);
            if items.is_empty() {
                render_empty(ui);
            } else {
                let item_w = full_w - 16.0;
                for (i, item) in items.iter().enumerate() {
                    if Some(i) == scroll_target_idx {
                        ui.scroll_to_cursor(Some(egui::Align::TOP));
                    }
                    render_item(ui, item, item_w);
                    ui.add_space(3.0);
                }
            }
        });
}
