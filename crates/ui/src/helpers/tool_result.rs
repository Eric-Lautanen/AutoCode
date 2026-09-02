// tool_result.rs -- Tool result body extraction + legacy ToolMeta normalization.

use autocode_core::state::{ChatMessage, ToolMeta};

/// Split legacy `Tool \`name\` result:\n...` content into (tool name, body).
/// Legacy sessions predate structured `ToolMeta`; parsing happens once here so
/// the renderer has a single structured code path.
fn split_legacy(content: &str) -> Option<(&str, &str)> {
    if !content.starts_with("Tool `") {
        return None;
    }
    let name_start = 6;
    let name_end = content[name_start..].find('`')? + name_start;
    let name = &content[name_start..name_end];
    let body = content.get(name_end + 1..)?.strip_prefix(" result:\n")?;
    Some((name, body))
}

/// Parse `-- N lines, B bytes --` / `total_lines:` / `total_bytes:` headers.
/// Multiple `-- N lines, B bytes --` headers (read_files sections) sum up.
fn parse_counts(body: &str) -> (Option<usize>, Option<usize>) {
    let mut lines = 0usize;
    let mut bytes = 0usize;
    let mut found = false;
    for l in body.lines() {
        if let Some(rest) = l.strip_prefix("-- ")
            && let Some((n, rest)) = rest.split_once(" lines")
            && let Ok(n) = n.trim().parse::<usize>()
        {
            found = true;
            lines += n;
            if let Some(b) = rest
                .split_once(", ")
                .and_then(|(_, b)| b.strip_suffix(" bytes --"))
            {
                bytes += b.trim().parse::<usize>().ok().unwrap_or(0);
            }
            continue;
        }
        if let Some(v) = l.strip_prefix("total_lines:") {
            found = true;
            lines += v.trim().parse().ok().unwrap_or(0);
        }
        if let Some(v) = l.strip_prefix("total_bytes:") {
            found = true;
            bytes += v.trim().parse().ok().unwrap_or(0);
        }
    }
    (found.then_some(lines), found.then_some(bytes))
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// Normalize a legacy text-encoded tool result into structured metadata so
/// rendering goes through the single structured path. Returns None when the
/// content is not a legacy tool result (plain markdown / assistant text).
pub fn legacy_tool_meta(msg: &ChatMessage) -> Option<ToolMeta> {
    let (name, body) = split_legacy(&msg.content)?;
    let mut meta = ToolMeta {
        tool_name: name.to_string(),
        ..Default::default()
    };
    match name {
        "read_file" | "read_entire_file" => {
            // Counts must be parsed before parse_path_header strips the header.
            let (lines, bytes) = parse_counts(body);
            let (path, _) = parse_path_header(body);
            meta.file_path = non_empty(path);
            meta.line_count = lines;
            meta.byte_count = bytes;
        }
        "read_files" => {
            let (lines, bytes) = parse_counts(body);
            meta.line_count = lines;
            meta.byte_count = bytes;
        }
        "run_shell" => {
            let exit_code = body
                .lines()
                .rev()
                .find_map(|l| {
                    l.strip_prefix("Exit code: ")
                        .or_else(|| l.strip_prefix("exit_code: "))
                        .and_then(|c| c.trim().parse::<i32>().ok())
                })
                .unwrap_or(-1);
            meta.exit_code = Some(exit_code);
            meta.is_error = exit_code != 0;
        }
        "list_dir" | "delete_file" | "rename_file" | "create_dir" | "get_skill" | "todo_list"
        | "patch_file" | "write_file" => {
            // Body-only legacy formats: path/counts unavailable; the renderer
            // falls back to showing the body.
        }
        _ => return None,
    }
    Some(meta)
}

pub fn extract_tool_body(content: &str) -> String {
    if let Some((_, body)) = split_legacy(content) {
        return body.to_string();
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

pub const CODE_DISPLAY_MAX_LINES: usize = 5000;

pub fn strip_exit_code_trailer(body: &str) -> &str {
    if let Some(pos) = body.rfind("\n\nExit code: ") {
        &body[..pos]
    } else if let Some(pos) = body.rfind("\nExit code: ") {
        &body[..pos]
    } else {
        body
    }
}
