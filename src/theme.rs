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
    pub const TAB_ACCENT: Color32 = Color32::from_rgb(220, 140, 50);
    pub const ERROR: Color32 = Color32::from_rgb(210, 80, 80);
    pub const PURPLE: Color32 = Color32::from_rgb(160, 120, 220);

}

// -- apply() -- call once from AutocodeApp::new() -------------------------------

pub fn apply(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    // Optionally add a system emoji font as a fallback so emojis and other
    // Unicode characters (CJK, symbols, etc.) render instead of showing
    // a missing-glyph box.
    //
    // System emoji fonts are 5-20 MB on disk and loaded fully into RAM,
    // so they are disabled by default. Set AUTOCODE_EMOJI_FONT=1 to enable.
    if std::env::var("AUTOCODE_EMOJI_FONT").as_deref() == Ok("1")
        && let Some(emoji_font) = load_system_emoji_font()
    {
        fonts
            .font_data
            .insert("system_emoji".to_owned(), emoji_font);
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            list.push("system_emoji".to_owned());
        }
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            list.push("system_emoji".to_owned());
        }
    }

    ctx.set_fonts(fonts);

    let mut style = Style::default();

    // Item spacing -- a little more breathing room than the egui default.
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.indent = 16.0;
    style.spacing.text_edit_width = 300.0;

    // Interact size -- minimum widget height (prevents squished buttons).
    style.spacing.interact_size.y = 24.0;

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

fn load_system_emoji_font() -> Option<std::sync::Arc<egui::FontData>> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\seguiemj.ttf",
            "C:\\Windows\\Fonts\\seguisym.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Apple Color Emoji.ttc",
            "/Library/Fonts/Apple Color Emoji.ttc",
        ]
    } else {
        &[
            "/usr/share/fonts/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/noto-color-emoji/NotoColorEmoji.ttf",
            "/usr/share/fonts/truetype/fonts-emoji/NotoColorEmoji.ttf",
        ]
    };

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            return Some(std::sync::Arc::new(egui::FontData::from_owned(data)));
        }
    }
    None
}
