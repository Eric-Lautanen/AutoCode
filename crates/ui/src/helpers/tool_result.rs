// tool_result.rs -- Tool result parsing helpers.

use autocode_core::state::ChatMessage;

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
