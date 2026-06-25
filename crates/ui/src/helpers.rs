// ui_helpers.rs -- Shared UI helper functions used across multiple UI modules.
// Time formatting, tool result parsing, widget factories, etc.

use egui::{Color32, FontId, RichText, TextFormat};

use crate::theme::Palette;
use autocode_core::state::{ChatMessage, TodoItem, TodoStatus};

// -- Time formatting -----------------------------------------------------------

pub fn format_time(ts: u64) -> String {
    let secs = ts % 86400;
    format!(
        "{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
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
        let total_lines: usize = rest
            .lines()
            .filter_map(|l| {
                l.strip_prefix("-- ")
                    .and_then(|l| l.split_once(" lines"))
                    .and_then(|(n, _)| n.parse::<usize>().ok())
            })
            .sum();
        let total_bytes: usize = rest
            .lines()
            .filter_map(|l| {
                l.strip_prefix("-- ")
                    .and_then(|l| {
                        l.split_once(" lines, ")
                            .and_then(|(_, rest)| rest.strip_suffix(" bytes --"))
                    })
                    .and_then(|b| b.parse::<usize>().ok())
            })
            .sum();
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
                    .last()
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
    } else if content.starts_with("Tool `get_skill` result:\n") {
        let rest = content
            .strip_prefix("Tool `get_skill` result:\n")
            .unwrap_or("");
        Some(format!("[skill] {} bytes", rest.len()))
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
            } else if after_star < byte_len {
                let content_start = after_star;
                let mut search = content_start;
                let mut found = false;
                while search < byte_len {
                    let sc = text[search..].chars().next().unwrap_or('\0');
                    if sc == '*' && search > content_start {
                        result.push_str(&text[content_start..search]);
                        byte_pos = search + 1;
                        found = true;
                        break;
                    }
                    search += sc.len_utf8();
                }
                if found {
                    continue;
                }
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

/// Returns the index of the "currently working" item:
/// the first `InProgress` item, falling back to the first `Pending` item.
pub fn find_current_task_index(items: &[TodoItem]) -> Option<usize> {
    items
        .iter()
        .position(|i| i.status == TodoStatus::InProgress)
        .or_else(|| items.iter().position(|i| i.status == TodoStatus::Pending))
}

// -- Diff helpers ---------------------------------------------------------------

pub(crate) struct DiffLine<'a> {
    pub prefix: char,
    pub text: &'a str,
    /// 1-based line number in the old file (0 for additions)
    pub old_lineno: usize,
    /// 1-based line number in the new file (0 for deletions)
    pub new_lineno: usize,
}

/// LCS-based diff (O(n*m) time/space). Falls back to simple diff for large files.
pub(crate) fn lcs_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let n = old.len();
    let m = new.len();
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    for i in 0..n {
        for j in 0..m {
            table[idx(i + 1, j + 1)] = if old[i] == new[j] {
                table[idx(i, j)] + 1
            } else {
                table[idx(i, j + 1)].max(table[idx(i + 1, j)])
            };
        }
    }

    let mut result = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: j,
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[idx(i, j - 1)] >= table[idx(i - 1, j)]) {
            result.push(DiffLine {
                prefix: '+',
                text: new[j - 1],
                old_lineno: 0,
                new_lineno: j,
            });
            j -= 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: 0,
            });
            i -= 1;
        }
    }
    result.reverse();
    result
}

/// Simple line-by-line diff for very large files (>2000 lines).
/// Walks both files greedily, emitting matching lines as context
/// and unmatched lines as deletions / insertions.
pub(crate) fn simple_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let mut result = Vec::new();
    let (mut o, mut n) = (0, 0);
    while o < old.len() || n < new.len() {
        if o < old.len() && n < new.len() && old[o] == new[n] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        } else if o >= old.len() {
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            n += 1;
        } else if n >= new.len() {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            o += 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        }
    }
    result
}

pub(crate) const CODE_DISPLAY_MAX_LINES: usize = 5000;

pub(crate) fn strip_exit_code_trailer(body: &str) -> &str {
    if let Some(pos) = body.rfind("\n\nExit code: ") {
        &body[..pos]
    } else if let Some(pos) = body.rfind("\nExit code: ") {
        &body[..pos]
    } else {
        body
    }
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
