use crate::helpers;
use crate::provider::ToolCall;
use autocode_core::state::ToolMeta;

pub fn file_tool_meta(
    name: &str,
    path: &str,
    result: &str,
    duration_ms: u64,
    is_error: bool,
) -> ToolMeta {
    let (total_lines, total_bytes) = result
        .lines()
        .nth(1)
        .and_then(|l| l.strip_prefix("-- "))
        .and_then(|l| l.strip_suffix(" --"))
        .and_then(|h| h.split_once(" lines, "))
        .and_then(|(l, b)| {
            let lines = l.parse::<usize>().ok()?;
            let bytes = b.strip_suffix(" bytes")?.parse::<usize>().ok()?;
            Some((lines, bytes))
        })
        .unwrap_or((result.lines().count(), result.len()));
    ToolMeta {
        tool_name: name.into(),
        file_path: Some(path.into()),
        line_count: Some(total_lines),
        byte_count: Some(total_bytes),
        is_error,
        duration_ms: Some(duration_ms),
        ..Default::default()
    }
}

pub fn build_tool_meta(tc: &ToolCall, result: &str, duration_ms: u64) -> ToolMeta {
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    let is_error = result.starts_with("{\"error\":") || result.starts_with("Error:");

    match tc.name.as_str() {
        "read_file" => file_tool_meta(
            "read_file",
            args["path"].as_str().unwrap_or(""),
            result,
            duration_ms,
            is_error,
        ),
        "read_entire_file" => file_tool_meta(
            "read_entire_file",
            args["path"].as_str().unwrap_or(""),
            result,
            duration_ms,
            is_error,
        ),
        "read_files" => {
            let paths: Vec<&str> = args["paths"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let file_list = paths.join(", ");
            let total_lines: usize = result
                .split("\n---\n")
                .flat_map(|section| {
                    section
                        .lines()
                        .skip(2) // skip "path:" and "-- N lines, M bytes --" lines
                        .collect::<Vec<_>>()
                })
                .filter(|l| !l.starts_with("[..."))
                .count();
            ToolMeta {
                tool_name: "read_files".into(),
                file_path: Some(file_list),
                line_count: Some(total_lines),
                byte_count: Some(result.len()),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "write_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let bytes = args["content"].as_str().map(|s| s.len()).unwrap_or(0);
            ToolMeta {
                tool_name: "write_file".into(),
                file_path: Some(path),
                byte_count: Some(bytes),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "patch_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let old_text = helpers::strip_line_numbers(args["old_text"].as_str().unwrap_or(""));
            let new_text = helpers::strip_line_numbers(args["new_text"].as_str().unwrap_or(""));
            // Parse "line N" from result: "Patched ... via ... (N -> M bytes, line 42)"
            let edit_line = if !is_error {
                result.rsplit_once(", line ").and_then(|(_, rest)| {
                    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    num.parse::<usize>().ok()
                })
            } else {
                None
            };
            ToolMeta {
                tool_name: "patch_file".into(),
                file_path: Some(path),
                old_text: Some(old_text),
                new_text: Some(new_text),
                is_error,
                duration_ms: Some(duration_ms),
                edit_line,
                ..Default::default()
            }
        }
        "patch_lines" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let start_line = args["start_line"].as_u64().unwrap_or(0) as usize;
            let end_line = args["end_line"].as_u64().unwrap_or(0) as usize;
            ToolMeta {
                tool_name: "patch_lines".into(),
                file_path: Some(path),
                edit_line: Some(start_line),
                line_count: Some(end_line.saturating_sub(start_line).saturating_add(1)),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "run_shell" => {
            let exit_code = result
                .lines()
                .last()
                .and_then(|l| l.strip_prefix("Exit code: "))
                .or_else(|| {
                    // Legacy: first line started with "exit_code: "
                    result
                        .lines()
                        .next()
                        .and_then(|l| l.strip_prefix("exit_code: "))
                })
                .and_then(|c| c.parse::<i32>().ok());
            ToolMeta {
                tool_name: "run_shell".into(),
                exit_code,
                line_count: Some(result.lines().count()),
                byte_count: Some(result.len()),
                is_error: exit_code.map(|c| c != 0).unwrap_or(is_error),
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "grep" => {
            let pattern = args["pattern"].as_str().unwrap_or("").to_string();
            let search_path = args["path"].as_str().unwrap_or("").to_string();
            let match_count = result
                .lines()
                .find_map(|l| {
                    let l = l.trim();
                    l.strip_suffix(" match(es):")
                        .and_then(|n| n.parse::<usize>().ok())
                })
                .unwrap_or(0);
            ToolMeta {
                tool_name: "grep".into(),
                file_path: Some(search_path),
                old_text: Some(pattern),
                line_count: Some(match_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "todo_list" => {
            let args: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
            let total = args["task_items"].as_array().map(|a| a.len()).unwrap_or(0);
            let done = args["task_items"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|v| v["status"].as_str() == Some("completed"))
                        .count()
                })
                .unwrap_or(0);
            ToolMeta {
                tool_name: "todo_list".into(),
                line_count: Some(total),
                byte_count: Some(done),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "project_task_list" => {
            let args: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
            let total = args["task_items"].as_array().map(|a| a.len()).unwrap_or(0);
            let done = args["task_items"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|v| v["status"].as_str() == Some("completed"))
                        .count()
                })
                .unwrap_or(0);
            ToolMeta {
                tool_name: "project_task_list".into(),
                line_count: Some(total),
                byte_count: Some(done),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "glob" => {
            let pattern = args["pattern"].as_str().unwrap_or("").to_string();
            let search_path = args["path"].as_str().unwrap_or("").to_string();
            let match_count = result
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().next())
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
            ToolMeta {
                tool_name: "glob".into(),
                file_path: Some(pattern),
                old_text: Some(search_path),
                line_count: Some(match_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "get_skill" => {
            let keyword = args["keyword"].as_str().unwrap_or("").to_string();
            let not_found = result.starts_with("No skill matching")
                || result.starts_with("No skills directory")
                || result.starts_with("Multiple skills match");
            ToolMeta {
                tool_name: "get_skill".into(),
                file_path: Some(keyword),
                byte_count: Some(result.len()),
                is_error: is_error || not_found,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "list_dir" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let entry_count = result.lines().count();
            ToolMeta {
                tool_name: "list_dir".into(),
                file_path: Some(path),
                line_count: Some(entry_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "project_tree" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let entry_count = result.lines().count();
            ToolMeta {
                tool_name: "project_tree".into(),
                file_path: Some(path),
                line_count: Some(entry_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "delete_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "delete_file".into(),
                file_path: Some(path),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "rename_file" => {
            let from = args["from"].as_str().unwrap_or("").to_string();
            let to = args["to"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "rename_file".into(),
                file_path: Some(from),
                old_text: Some(to),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "create_dir" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "create_dir".into(),
                file_path: Some(path),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "handoff" => {
            let reason = args["reason"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "handoff".into(),
                old_text: Some(reason),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "name_session" => ToolMeta {
            tool_name: "name_session".into(),
            ..Default::default()
        },
        "verify_proof" => ToolMeta {
            tool_name: "verify_proof".into(),
            is_error,
            duration_ms: Some(duration_ms),
            ..Default::default()
        },
        "search_literature" => ToolMeta {
            tool_name: "search_literature".into(),
            is_error,
            duration_ms: Some(duration_ms),
            ..Default::default()
        },
        "explore_theorem" => ToolMeta {
            tool_name: "explore_theorem".into(),
            is_error,
            duration_ms: Some(duration_ms),
            ..Default::default()
        },
        _ => ToolMeta {
            tool_name: tc.name.clone(),
            is_error,
            duration_ms: Some(duration_ms),
            ..Default::default()
        },
    }
}
