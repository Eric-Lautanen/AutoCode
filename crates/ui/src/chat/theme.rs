// theme.rs -- Theme colors and spacing/type scale for chat.

use egui::Color32;

use crate::theme::Palette;

pub struct ThemeColors {
    pub accent: Color32,
    pub bg_surface: Color32,
    pub bg_base: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_code: Color32,
    pub border: Color32,
    pub success: Color32,
    pub error: Color32,
    pub tool_badge: Color32,
    pub user_bubble_fill: Color32,
    pub user_bubble_stroke: Color32,
    pub assistant_bubble_fill: Color32,
    pub assistant_bubble_stroke: Color32,
    pub live_tool_bg: Color32,
    pub terminal_bg: Color32,
    pub terminal_text: Color32,
    pub terminal_border: Color32,
    pub code_frame_bg: Color32,
    pub diff_frame_bg: Color32,
    pub diff_del_text: Color32,
    pub diff_add_text: Color32,
    pub diff_num: Color32,
    pub reason_bg: Color32,
    pub reason_border: Color32,
}

/// Single const instance — `theme()` is zero-cost and every chat color lives
/// here (no hardcoded `Color32::from_rgb` literals in chat rendering code).
pub(crate) static THEME: ThemeColors = ThemeColors {
    accent: Palette::ACCENT,
    bg_surface: Palette::BG_SURFACE,
    bg_base: Palette::BG_BASE,
    text_primary: Palette::TEXT_PRIMARY,
    text_secondary: Palette::TEXT_SECONDARY,
    text_muted: Palette::TEXT_MUTED,
    text_code: Palette::TEXT_CODE,
    border: Palette::BORDER,
    success: Palette::SUCCESS,
    error: Palette::ERROR,
    tool_badge: Palette::WARNING,
    user_bubble_fill: Color32::from_rgb(28, 41, 74),
    user_bubble_stroke: Color32::from_rgb(46, 64, 110),
    assistant_bubble_fill: Color32::from_rgb(24, 27, 38),
    assistant_bubble_stroke: Palette::BORDER,
    live_tool_bg: Color32::from_rgb(28, 33, 43),
    terminal_bg: Color32::from_rgb(13, 13, 18),
    terminal_text: Palette::TEXT_CODE,
    terminal_border: Color32::from_rgb(36, 46, 36),
    code_frame_bg: Color32::from_rgb(15, 18, 26),
    diff_frame_bg: Color32::from_rgb(15, 18, 26),
    diff_del_text: Color32::from_rgb(255, 140, 140),
    diff_add_text: Color32::from_rgb(140, 255, 161),
    diff_num: Palette::TEXT_MUTED,
    reason_bg: Color32::from_rgb(18, 20, 26),
    reason_border: Color32::from_rgb(41, 48, 61),
};

#[inline]
pub(crate) fn theme() -> &'static ThemeColors {
    &THEME
}

// -- Spacing / type scale ------------------------------------------------------

pub(crate) const SPACE_XS: f32 = 4.0;
pub(crate) const SPACE_S: f32 = 6.0;
pub(crate) const SPACE_M: f32 = 8.0;

pub(crate) const FONT_META: f32 = 9.5;
pub(crate) const FONT_LABEL: f32 = 11.0;
pub(crate) const FONT_SMALL: f32 = 12.0;
pub(crate) const FONT_BODY: f32 = 13.0;
pub(crate) const FONT_H3: f32 = 13.5;
pub(crate) const FONT_H2: f32 = 14.5;
pub(crate) const FONT_H1: f32 = 16.0;
