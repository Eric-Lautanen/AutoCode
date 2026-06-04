// ui_helpers.rs -- Shared UI helper functions used across multiple UI modules.
// Time formatting, tool result parsing, widget factories, etc.

use egui::{Color32, FontId, RichText, TextFormat};

use crate::state::ChatMessage;
use crate::theme::Palette;

// -- Time formatting -----------------------------------------------------------

pub fn format_time(ts: u64) -> String {
    let secs = ts % 86400;
    format!("{:02}:{:02}Z", secs / 3600, (secs % 3600) / 60)
}

// -- Tool result parsing -------------------------------------------------------

pub fn extract_tool_summary(content: &str) -> Option<String> {
    if let Some(rest) = content.strip_prefix("Tool `read_file` result:\n") {
        let (filename, body) = parse_path_header(rest);
        let total_lines = body
            .lines()
            .find(|l| l.starts_with("total_lines:"))
            .and_then(|l| l.strip_prefix("total_lines:"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(|| body.lines().count());
        let total_bytes = body
            .lines()
            .find(|l| l.starts_with("total_bytes:"))
            .and_then(|l| l.strip_prefix("total_bytes:"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(body.len());
        Some(format!(
            "[File] read {} — {} lines, {} bytes",
            filename, total_lines, total_bytes
        ))
    } else if let Some(rest) = content.strip_prefix("Tool `read_files` result:\n") {
        let total_lines = rest.lines().count();
        let total_bytes = rest.len();
        Some(format!(
            "[File] read_files — {} lines, {} bytes",
            total_lines, total_bytes
        ))
    } else if content.starts_with("Tool `write_file` result:\n") {
        let rest = content
            .strip_prefix("Tool `write_file` result:\n")
            .unwrap_or("");
        Some(format!("[File] Written: {}", rest.trim()))
    } else if content.starts_with("Tool `patch_file` result:\n") {
        let rest = content
            .strip_prefix("Tool `patch_file` result:\n")
            .unwrap_or("");
        Some(format!("[File] Patched: {}", rest.trim()))
    } else if content.starts_with("Tool `run_shell` result:\n") {
        let rest = content
            .strip_prefix("Tool `run_shell` result:\n")
            .unwrap_or("");
        let exit_code = rest
            .lines()
            .last()
            .and_then(|l| l.strip_prefix("Exit code: "))
            .or_else(|| {
                rest.lines()
                    .next()
                    .and_then(|l| l.strip_prefix("exit_code: "))
            })
            .and_then(|c| c.parse::<i32>().ok())
            .unwrap_or(-1);
        if exit_code == 0 {
            Some("[OK] Shell exited 0".to_string())
        } else {
            Some(format!("[FAIL] Shell exited {}", exit_code))
        }
    } else if content.starts_with("Tool `todo_list` result:\n") {
        let rest = content
            .strip_prefix("Tool `todo_list` result:\n")
            .unwrap_or("");
        Some(format!("[todo] {}", rest.trim()))
    } else if content.starts_with("Tool `list_dir` result:\n") {
        let rest = content
            .strip_prefix("Tool `list_dir` result:\n")
            .unwrap_or("");
        let count = rest.lines().count();
        Some(format!("[File] List directory — {} entries", count))
    } else if content.starts_with("Tool `delete_file` result:\n") {
        let rest = content
            .strip_prefix("Tool `delete_file` result:\n")
            .unwrap_or("");
        Some(format!("[File] Deleted: {}", rest.trim()))
    } else if content.starts_with("Tool `rename_file` result:\n") {
        let rest = content
            .strip_prefix("Tool `rename_file` result:\n")
            .unwrap_or("");
        Some(format!("[File] Renamed: {}", rest.trim()))
    } else if content.starts_with("Tool `create_dir` result:\n") {
        let rest = content
            .strip_prefix("Tool `create_dir` result:\n")
            .unwrap_or("");
        Some(format!("[File] Created directory: {}", rest.trim()))
    } else {
        None
    }
}

pub fn extract_tool_body(content: &str) -> String {
    if let Some(idx) = content.find(" result:\n")
        && content.starts_with("Tool `")
    {
        return content[idx + " result:\n".len()..].to_string();
    }
    content.to_string()
}

pub fn get_tool_body(msg: &ChatMessage) -> String {
    extract_tool_body(&msg.content)
}

pub fn parse_path_header(rest: &str) -> (String, &str) {
    let nl = rest.find('\n').unwrap_or(rest.len());
    let first_line = &rest[..nl];
    let body = rest.get(nl + 1..).unwrap_or("");
    // New format: "filepath\n-- N lines, B bytes --\n..."
    if let Some(body) = body.strip_prefix("-- ")
        && let Some(end) = body.find(" --\n")
    {
        return (first_line.to_string(), &body[end + 4..]);
    }
    // Legacy format: "path:filepath\ntotal_lines:N\n..."
    if let Some(after_path) = rest.strip_prefix("path:") {
        let nl = after_path.find('\n').unwrap_or(after_path.len());
        let full_path = &after_path[..nl];
        let body = after_path.get(nl + 1..).unwrap_or("");
        return (full_path.to_string(), body);
    }
    (String::new(), rest)
}

// -- Toolbar helpers -----------------------------------------------------------

pub fn toolbar_separator(ui: &mut egui::Ui) {
    ui.add(egui::Separator::default().vertical().spacing(8.0));
}

// -- Settings helpers ----------------------------------------------------------

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

// -- Markdown inline formatting (strips markers for plain-text contexts) -------

pub fn parse_inline_formatting(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let byte_len = text.len();
    let mut byte_pos = 0;
    let mut safety = 0;

    while byte_pos < byte_len {
        safety += 1;
        if safety > 50_000 {
            break;
        }
        let c = text[byte_pos..].chars().next().unwrap_or('\0');

        if c == '`' {
            let content_start = byte_pos + 1;
            let mut search = content_start;
            let mut found_close = false;
            while search < byte_len {
                let sc = text[search..].chars().next().unwrap_or('\0');
                if sc == '`' {
                    result.push_str(&text[content_start..search]);
                    byte_pos = search + 1;
                    found_close = true;
                    break;
                }
                search += sc.len_utf8();
            }
            if !found_close {
                result.push(c);
                byte_pos += 1;
            }
            continue;
        }

        if c == '*' {
            let after_star = byte_pos + c.len_utf8();
            if after_star < byte_len && text[after_star..].starts_with('*') {
                let content_start = after_star + 1;
                let mut search = content_start;
                let mut found = false;
                while search + 1 < byte_len {
                    let sc = text[search..].chars().next().unwrap_or('\0');
                    let after = search + sc.len_utf8();
                    if sc == '*' && after < byte_len && text[after..].starts_with('*') {
                        result.push_str(&text[content_start..search]);
                        byte_pos = after + 1;
                        found = true;
                        break;
                    }
                    search += sc.len_utf8();
                }
                if found {
                    continue;
                }
                result.push_str("**");
                byte_pos = after_star + 1;
                continue;
            }
        }

        result.push(c);
        byte_pos += c.len_utf8();
    }
    result
}

// -- Rich inline text append to LayoutJob --------------------------------------

pub fn append_rich_inline_to_job(job: &mut egui::text::LayoutJob, text: &str) {
    let body_font = FontId::proportional(13.0);
    let mono_font = FontId::monospace(12.0);

    let byte_len = text.len();
    let mut byte_pos = 0;
    let mut safety = 0;

    while byte_pos < byte_len {
        safety += 1;
        if safety > 50_000 {
            break;
        }
        let remaining = &text[byte_pos..];
        let next_char = remaining.chars().next().unwrap_or('\0');

        if next_char == '`' {
            let content_start = byte_pos + 1;
            let mut search = content_start;
            let mut found_close = false;
            while search < byte_len {
                let c = text[search..].chars().next().unwrap_or('\0');
                if c == '`' {
                    let code_text = &text[content_start..search];
                    job.append(
                        code_text,
                        0.0,
                        TextFormat {
                            font_id: mono_font.clone(),
                            color: Palette::TEXT_CODE,
                            background: Color32::from_rgb(30, 35, 45),
                            ..Default::default()
                        },
                    );
                    byte_pos = search + 1;
                    found_close = true;
                    break;
                }
                search += c.len_utf8();
            }
            if !found_close {
                job.append(
                    "`",
                    0.0,
                    TextFormat {
                        font_id: body_font.clone(),
                        color: Palette::TEXT_PRIMARY,
                        ..Default::default()
                    },
                );
                byte_pos += 1;
            }
            continue;
        }

        if next_char == '*' {
            let after_star = byte_pos + 1;
            if after_star < byte_len && text[after_star..].starts_with('*') {
                let content_start = after_star + 1;
                let mut search = content_start;
                let mut found = false;
                while search + 1 < byte_len {
                    let c = text[search..].chars().next().unwrap_or('\0');
                    let after = search + c.len_utf8();
                    if c == '*' && after < byte_len && text[after..].starts_with('*') {
                        let bold_text = &text[content_start..search];
                        job.append(
                            bold_text,
                            0.0,
                            TextFormat {
                                font_id: body_font.clone(),
                                color: Color32::WHITE,
                                ..Default::default()
                            },
                        );
                        byte_pos = after + 1;
                        found = true;
                        break;
                    }
                    search += c.len_utf8();
                }
                if found {
                    continue;
                }
                job.append(
                    "**",
                    0.0,
                    TextFormat {
                        font_id: body_font.clone(),
                        color: Palette::TEXT_PRIMARY,
                        ..Default::default()
                    },
                );
                byte_pos = after_star + 1;
                continue;
            } else if after_star < byte_len {
                let content_start = after_star;
                let mut search = content_start;
                let mut found = false;
                while search < byte_len {
                    let c = text[search..].chars().next().unwrap_or('\0');
                    if c == '*' && search > content_start {
                        let italic_text = &text[content_start..search];
                        job.append(
                            italic_text,
                            0.0,
                            TextFormat {
                                font_id: body_font.clone(),
                                color: Palette::TEXT_PRIMARY,
                                italics: true,
                                ..Default::default()
                            },
                        );
                        byte_pos = search + 1;
                        found = true;
                        break;
                    }
                    search += c.len_utf8();
                }
                if found {
                    continue;
                }
                job.append(
                    "*",
                    0.0,
                    TextFormat {
                        font_id: body_font.clone(),
                        color: Palette::TEXT_PRIMARY,
                        ..Default::default()
                    },
                );
                byte_pos += 1;
                continue;
            }
        }

        let mut plain_end = byte_pos;
        let mut scan = byte_pos;
        while scan < byte_len {
            let c = text[scan..].chars().next().unwrap_or('\0');
            if c == '`' || c == '*' {
                break;
            }
            scan += c.len_utf8();
            plain_end = scan;
        }
        if plain_end > byte_pos {
            let plain = &text[byte_pos..plain_end];
            job.append(
                plain,
                0.0,
                TextFormat {
                    font_id: body_font.clone(),
                    color: Palette::TEXT_PRIMARY,
                    ..Default::default()
                },
            );
            byte_pos = plain_end;
        } else {
            byte_pos += next_char.len_utf8().max(1);
        }
    }

    if job.sections.is_empty() {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: body_font,
                color: Palette::TEXT_PRIMARY,
                ..Default::default()
            },
        );
    }
}

// -- Screen pixel sampling (eyedropper) -----------------------------------------
// Uses Windows GetPixel to grab the color at the current cursor position.
// Returns None on non-Windows or on failure.

#[cfg(windows)]
unsafe extern "system" {
    fn GetDC(hwnd: isize) -> isize;
    fn GetPixel(hdc: isize, x: i32, y: i32) -> u32;
    fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
    fn GetCursorPos(lpPoint: *mut Point) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

/// Sample the screen pixel at the current cursor position.
/// Returns `[r, g, b]` with each component in 0.0–1.0 range.
pub fn sample_screen_pixel() -> Option<[f32; 3]> {
    #[cfg(windows)]
    {
        unsafe {
            let hdc = GetDC(0);
            if hdc == 0 {
                return None;
            }
            let mut pt = Point { x: 0, y: 0 };
            if GetCursorPos(&mut pt as *mut Point) == 0 {
                ReleaseDC(0, hdc);
                return None;
            }
            let color = GetPixel(hdc, pt.x, pt.y);
            ReleaseDC(0, hdc);
            // COLORREF is 0x00BBGGRR, 0xFFFFFFFF = error
            if color == 0xFFFFFFFF {
                return None;
            }
            let r = ((color >> 0) & 0xFF) as f32 / 255.0;
            let g = ((color >> 8) & 0xFF) as f32 / 255.0;
            let b = ((color >> 16) & 0xFF) as f32 / 255.0;
            Some([r, g, b])
        }
    }
    #[cfg(not(windows))]
    None
}
