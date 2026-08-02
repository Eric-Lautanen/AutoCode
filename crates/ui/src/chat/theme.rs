// theme.rs -- Theme colors for the chat panel.

use egui::Color32;

use crate::theme::Palette;

pub struct ThemeColors {
    pub accent: Color32,
    pub accent_dim: Color32,
    pub bg_surface: Color32,
    pub bg_base: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_code: Color32,
    pub border: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub user_badge: Color32,
    pub assist_badge: Color32,
    pub tool_badge: Color32,
    pub system_badge: Color32,
    pub user_bubble_fill: Color32,
    pub user_bubble_stroke: Color32,
    pub assistant_bubble_fill: Color32,
    pub assistant_bubble_stroke: Color32,
    pub live_tool_bg: Color32,
    pub terminal_bg: Color32,
    pub terminal_text: Color32,
    pub terminal_border: Color32,
    pub terminal_label: Color32,
    pub live_terminal_bg: Color32,
    pub live_terminal_border: Color32,
    pub code_frame_bg: Color32,
    pub diff_frame_bg: Color32,
    pub diff_del_text: Color32,
    pub diff_add_text: Color32,
    pub diff_num: Color32,
    pub reason_bg: Color32,
    pub reason_border: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            accent: Color32::from_rgb(99, 156, 235),
            accent_dim: Palette::ACCENT_DIM,
            bg_surface: Palette::BG_SURFACE,
            bg_base: Palette::BG_BASE,
            text_primary: Color32::from_rgb(219, 224, 232),
            text_secondary: Color32::from_rgb(161, 168, 186),
            text_muted: Color32::from_rgb(89, 99, 117),
            text_code: Color32::from_rgb(189, 209, 181),
            border: Palette::BORDER,
            success: Color32::from_rgb(79, 181, 120),
            warning: Color32::from_rgb(209, 161, 61),
            error: Color32::from_rgb(209, 79, 79),
            user_badge: Color32::from_rgb(99, 156, 235),
            assist_badge: Color32::from_rgb(79, 181, 120),
            tool_badge: Color32::from_rgb(209, 161, 61),
            system_badge: Color32::from_rgb(161, 120, 219),
            user_bubble_fill: Color32::from_rgb(28, 41, 74),
            user_bubble_stroke: Color32::from_rgb(46, 64, 110),
            assistant_bubble_fill: Color32::from_rgb(24, 27, 38),
            assistant_bubble_stroke: Palette::BORDER,
            live_tool_bg: Color32::from_rgb(28, 33, 43),
            terminal_bg: Color32::from_rgb(13, 13, 18),
            terminal_text: Color32::from_rgb(171, 199, 166),
            terminal_border: Color32::from_rgb(36, 46, 36),
            terminal_label: Color32::from_rgb(89, 99, 117),
            live_terminal_bg: Color32::from_rgb(13, 13, 18),
            live_terminal_border: Color32::from_rgb(36, 46, 36),
            code_frame_bg: Color32::from_rgb(15, 18, 26),
            diff_frame_bg: Color32::from_rgb(15, 18, 26),
            diff_del_text: Color32::from_rgb(255, 140, 140),
            diff_add_text: Color32::from_rgb(140, 255, 161),
            diff_num: Color32::from_rgb(89, 99, 117),
            reason_bg: Color32::from_rgb(18, 20, 26),
            reason_border: Color32::from_rgb(41, 48, 61),
        }
    }
}

pub(crate) fn theme() -> ThemeColors {
    ThemeColors::default()
}
