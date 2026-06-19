// theme.rs -- Centralised theme.
// Applies a custom dark Visuals to the egui Context.
// All UI code should read colours from `ui.visuals()` where possible;
// only truly app-specific colours are kept here.

use egui::{
    Color32, Context, CornerRadius, FontDefinitions, Margin, Shadow, Stroke, Style, Visuals,
};

// -- Corner radii (shared constants) ------------------------------------------

pub const ROUND_SM: CornerRadius = CornerRadius::same(4);
pub const ROUND_MD: CornerRadius = CornerRadius::same(6);
pub const ROUND_LG: CornerRadius = CornerRadius::same(10);

// -- App-specific colour palette -----------------------------------------------
// Only colours that are NOT available via `ui.visuals()` live here.

pub struct Palette;

impl Palette {
    // Base surfaces -- matched to Visuals so panels blend naturally.
    pub const BG_BASE: Color32 = Color32::from_rgb(15, 17, 21);
    pub const BG_PANEL: Color32 = Color32::from_rgb(20, 23, 28);
    pub const BG_SURFACE: Color32 = Color32::from_rgb(27, 31, 38);
    pub const BG_ACTIVE: Color32 = Color32::from_rgb(33, 39, 50);

    // Accent -- a desaturated blue, not eye-searing.
    pub const ACCENT: Color32 = Color32::from_rgb(99, 155, 234);
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(60, 100, 170);

    // Text hierarchy.
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 224, 232);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(160, 168, 185);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(90, 100, 118);
    pub const TEXT_CODE: Color32 = Color32::from_rgb(188, 210, 180);

    // Border -- subtle, 1 px.
    pub const BORDER: Color32 = Color32::from_rgb(42, 48, 60);

    // Semantic.
    pub const SUCCESS: Color32 = Color32::from_rgb(80, 180, 120);
    pub const WARNING: Color32 = Color32::from_rgb(210, 160, 60);
    pub const ERROR: Color32 = Color32::from_rgb(210, 80, 80);
    pub const PURPLE: Color32 = Color32::from_rgb(160, 120, 220);
}

/// Generate a deterministic accent color for a project, derived from its ID.
/// The result is theme-appropriate (moderately saturated, works on dark backgrounds).
pub fn project_accent(project_id: &str) -> Color32 {
    let hash: u64 = {
        let mut h = 5381u64;
        for b in project_id.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    };
    let hue = (hash % 360) as f32;
    hsv_to_rgb(hue, 0.55, 0.72)
}

/// Convert HSV (hue 0–360, saturation/value 0–1) to an sRGB Color32.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 | 6 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let ri = ((r + m) * 255.0).round() as u8;
    let gi = ((g + m) * 255.0).round() as u8;
    let bi = ((b + m) * 255.0).round() as u8;
    Color32::from_rgb(ri, gi, bi)
}

// -- apply() -- call once from AutocodeApp::new() -------------------------------

pub fn apply(ctx: &Context) {
    // Use egui's default fonts only — no emoji font loaded.
    // Unsupported glyphs (emoji, symbols, etc.) are handled by
    // sanitize_display_text() which strips/replaces them before rendering,
    // avoiding tofu blocks without loading 5-20 MB font files.
    ctx.set_fonts(FontDefinitions::default());

    let mut style = Style::default();

    // Item spacing -- a little more breathing room than the egui default.
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.indent = 16.0;
    style.spacing.text_edit_width = 300.0;

    // Interact size -- minimum widget height (prevents squished buttons).
    style.spacing.interact_size.y = 24.0;

    // Tooltip timing -- show immediately on hover.
    style.interaction.show_tooltips_only_when_still = false;
    style.interaction.tooltip_delay = 0.0;
    style.interaction.tooltip_grace_time = 0.0;

    let mut v = Visuals::dark();

    // Window chrome.
    v.window_fill = Palette::BG_PANEL;
    v.window_stroke = Stroke::new(1.0, Palette::BORDER);
    v.window_shadow = Shadow {
        offset: [2, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };
    v.window_corner_radius = ROUND_MD;

    // Panels.
    v.panel_fill = Palette::BG_PANEL;

    // Widgets (default / hovered / active).
    v.widgets.noninteractive.bg_fill = Palette::BG_SURFACE;
    v.widgets.noninteractive.weak_bg_fill = Palette::BG_SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Palette::TEXT_SECONDARY);
    v.widgets.noninteractive.corner_radius = ROUND_SM;

    v.widgets.inactive.bg_fill = Palette::BG_SURFACE;
    v.widgets.inactive.weak_bg_fill = Palette::BG_SURFACE;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::TEXT_PRIMARY);
    v.widgets.inactive.corner_radius = ROUND_SM;

    v.widgets.hovered.bg_fill = Palette::BG_ACTIVE;
    v.widgets.hovered.weak_bg_fill = Palette::BG_ACTIVE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::ACCENT_DIM);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, Palette::TEXT_PRIMARY);
    v.widgets.hovered.corner_radius = ROUND_SM;

    v.widgets.active.bg_fill = Palette::ACCENT_DIM;
    v.widgets.active.weak_bg_fill = Palette::ACCENT_DIM;
    v.widgets.active.bg_stroke = Stroke::new(1.0, Palette::ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    v.widgets.active.corner_radius = ROUND_SM;

    v.widgets.open.bg_fill = Palette::BG_ACTIVE;
    v.widgets.open.weak_bg_fill = Palette::BG_ACTIVE;
    v.widgets.open.bg_stroke = Stroke::new(1.0, Palette::ACCENT_DIM);
    v.widgets.open.corner_radius = ROUND_SM;

    // Selection highlight.
    v.selection.bg_fill = Color32::from_rgb(50, 90, 150);
    v.selection.stroke = Stroke::new(1.0, Palette::ACCENT);

    // Misc.
    v.extreme_bg_color = Palette::BG_BASE;
    v.faint_bg_color = Color32::from_rgb(22, 26, 32);
    v.code_bg_color = Color32::from_rgb(18, 20, 26);

    v.override_text_color = Some(Palette::TEXT_PRIMARY);

    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    // Separators / stripes.
    v.striped = false;

    style.visuals = v;
    ctx.set_global_style(style);
}


