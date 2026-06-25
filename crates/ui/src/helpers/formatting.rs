// formatting.rs -- Markdown inline formatting (plain-text strip and rich LayoutJob append).

use egui::{Color32, FontId, TextFormat};

use crate::theme::Palette;

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
