use crate::helpers;
use autocode_core::state::TodoItem;
use autocode_core::state::TodoStatus;

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}

fn write_json(path: &std::path::Path, val: &serde_json::Value) {
    if let Ok(json) = serde_json::to_string_pretty(val) {
        let _ = std::fs::write(path, &json);
    }
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

pub struct ToolExecCtx<'a> {
    pub tc: &'a crate::provider::ToolCall,
    pub project_root: &'a str,
    pub path_cache: &'a mut autocode_core::helpers::LruPathCache,
    pub allow_escape: bool,
    pub ctx_used: usize,
    pub ctx_max: usize,
    pub max_output: usize,
    pub session_named: bool,
}

pub fn execute_tool_with_cache(ctx: ToolExecCtx<'_>) -> String {
    let ToolExecCtx {
        tc,
        project_root,
        path_cache,
        allow_escape,
        ctx_used,
        ctx_max,
        max_output,
        session_named,
    } = ctx;
    use autocode_core::helpers::{resolve_path_cached, resolve_path_write_cached};
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    match tc.name.as_str() {
        "read_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            match autocode_core::utils::fsutil::read_to_string(&path) {
                Ok(content) => {
                    let all_lines: Vec<&str> = content.lines().collect();
                    let total_lines = all_lines.len();
                    let total_bytes = content.len();

                    // offset is 1-based; default to line 1
                    let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
                    let limit = args["limit"].as_u64().unwrap_or(2000).max(1) as usize;

                    let start_idx = (offset - 1).min(total_lines);
                    let end_idx = start_idx + limit;

                    let mut out = format!(
                        "{}\n-- {} lines, {} bytes --\n",
                        path.display(),
                        total_lines,
                        total_bytes
                    );

                    if start_idx >= total_lines {
                        out.push_str(&format!(
                            "Offset {} exceeds file length ({} lines). No content returned.\n",
                            offset, total_lines
                        ));
                    } else {
                        let truncated = end_idx < total_lines;
                        let slice = &all_lines[start_idx..end_idx.min(total_lines)];
                        // Calculate width for line number padding
                        let last_line_num = start_idx + slice.len();
                        let width = format!("{}", last_line_num).len();
                        for (i, line) in slice.iter().enumerate() {
                            let line_num = start_idx + i + 1;
                            out.push_str(&format!(
                                "{:>width$} | {}\n",
                                line_num,
                                line,
                                width = width
                            ));
                        }
                        if truncated {
                            let remaining = total_lines - end_idx;
                            out.push_str(&format!(
                                "\n... {} more line(s) below (use offset={} to continue reading)",
                                remaining,
                                end_idx + 1
                            ));
                        }
                    }
                    out
                }
                Err(e) => helpers::tool_error(
                    &format!("Error reading {}: {}", path.display(), e),
                    "Check the path is correct and the file is readable",
                ),
            }
        }

        "read_entire_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            match autocode_core::utils::fsutil::read_to_string(&path) {
                Ok(content) => {
                    let all_lines: Vec<&str> = content.lines().collect();
                    let total_lines = all_lines.len();
                    let total_bytes = content.len();
                    let width = format!("{}", total_lines).len();
                    let mut out = format!(
                        "{}\n-- {} lines, {} bytes --\n",
                        path.display(),
                        total_lines,
                        total_bytes
                    );
                    for (i, line) in all_lines.iter().enumerate() {
                        out.push_str(&format!("{:>width$} | {}\n", i + 1, line, width = width));
                    }
                    out
                }
                Err(e) => helpers::tool_error(
                    &format!("Error reading {}: {}", path.display(), e),
                    "Check the path is correct and the file is readable",
                ),
            }
        }

        "read_files" => {
            let paths = match args["paths"].as_array() {
                Some(a) => a.clone(),
                None => {
                    if let Some(s) = args["paths"].as_str() {
                        vec![serde_json::Value::String(s.to_string())]
                    } else {
                        return format!(
                            "Error: 'paths' must be an array of strings, got: {}",
                            args["paths"]
                        )
                        .to_string();
                    }
                }
            };
            if paths.is_empty() {
                return "Error: paths array is empty".to_string();
            }
            const MAX_BYTES: usize = 32 * 1024;
            let mut out = String::new();
            for val in &paths {
                let raw = match val.as_str() {
                    Some(s) => s,
                    None => continue,
                };
                let path = resolve_path_cached(raw, project_root, path_cache, allow_escape);
                if autocode_core::helpers::is_blocked_path(&path) {
                    out.push_str(&autocode_core::helpers::blocked_error(raw));
                    out.push_str("\n---\n");
                    continue;
                }
                out.push_str(&format!("path:{}\n", path.display()));
                match autocode_core::utils::fsutil::read_to_string(&path) {
                    Ok(content) => {
                        let all_lines: Vec<&str> = content.lines().collect();
                        let total_lines = all_lines.len();
                        let total_bytes = content.len();
                        let width = format!("{}", total_lines).len();
                        out.push_str(&format!(
                            "-- {} lines, {} bytes --\n",
                            total_lines, total_bytes
                        ));

                        if content.len() <= MAX_BYTES {
                            for (i, line) in all_lines.iter().enumerate() {
                                out.push_str(&format!(
                                    "{:>width$} | {}\n",
                                    i + 1,
                                    line,
                                    width = width
                                ));
                            }
                        } else {
                            let head_bytes = (MAX_BYTES * 3) / 5;
                            let tail_bytes = MAX_BYTES - head_bytes;

                            let mut head_lines: Vec<&str> = Vec::new();
                            let mut budget = head_bytes;
                            for line in &all_lines {
                                if line.len() + 1 > budget {
                                    break;
                                }
                                budget -= line.len() + 1;
                                head_lines.push(line);
                            }

                            let mut tail_lines: Vec<&str> = Vec::new();
                            budget = tail_bytes;
                            for line in all_lines.iter().rev() {
                                if line.len() + 1 > budget {
                                    break;
                                }
                                budget -= line.len() + 1;
                                tail_lines.push(line);
                            }
                            tail_lines.reverse();

                            for (i, line) in head_lines.iter().enumerate() {
                                out.push_str(&format!(
                                    "{:>width$} | {}\n",
                                    i + 1,
                                    line,
                                    width = width
                                ));
                            }

                            let omitted = total_lines - head_lines.len() - tail_lines.len();
                            if omitted > 0 {
                                out.push_str(&format!("\n[... {} lines omitted ...]\n\n", omitted));
                                for (i, line) in tail_lines.iter().enumerate() {
                                    let line_num = total_lines - tail_lines.len() + i + 1;
                                    out.push_str(&format!(
                                        "{:>width$} | {}\n",
                                        line_num,
                                        line,
                                        width = width
                                    ));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        out.push_str(&helpers::tool_error(
                            &format!("Error reading {}: {}", path.display(), e),
                            "Check the path is correct and the file is readable",
                        ));
                    }
                }
                out.push_str("\n---\n");
            }
            out
        }

        "write_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let content = args["content"].as_str().unwrap_or("");
            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            if let Some(parent) = path.parent()
                && let Err(e) = autocode_core::utils::fsutil::create_dir_all(parent)
            {
                return helpers::tool_error(
                    &format!(
                        "Error creating parent directory for {}: {}",
                        path.display(),
                        e
                    ),
                    "Check that the parent path is writable",
                );
            }
            match autocode_core::utils::fsutil::write(&path, content) {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!("Written {} bytes to {}", content.len(), path.display())
                }
                Err(e) => helpers::tool_error(
                    &format!("Error writing {}: {}", path.display(), e),
                    "Check that the path is writable and parent directories exist",
                ),
            }
        }

        "list_dir" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            if !path.exists() {
                return format!("Error listing {}: path does not exist", path.display());
            }
            let entries = autocode_fs::explorer::list_dir(&path);
            if entries.is_empty() && autocode_core::utils::fsutil::read_dir(&path).is_err() {
                return format!(
                    "Error listing {}: permission denied or invalid path",
                    path.display()
                );
            }
            let mut lines: Vec<String> = entries
                .iter()
                .map(|e| {
                    if e.is_dir {
                        format!("{}/", e.name)
                    } else {
                        e.name.clone()
                    }
                })
                .collect();
            lines.sort();
            lines.join("\n")
        }

        "project_tree" => {
            let raw_path = args["path"].as_str().unwrap_or(project_root);
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            if !path.exists() {
                return format!("Error: path does not exist: {}", path.display());
            }
            if path.is_file() {
                return format!("Error: '{}' is a file, not a directory", path.display());
            }
            let entries = autocode_fs::explorer::project_tree(&path);
            if entries.is_empty() {
                if autocode_core::utils::fsutil::read_dir(&path).is_err() {
                    return helpers::tool_error(
                        &format!("Error reading directory: {}", path.display()),
                        "Check permissions; the directory exists but cannot be read",
                    );
                }
                return "(empty tree)".to_string();
            }
            entries.join("\n")
        }

        "delete_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            let result = if autocode_core::utils::fsutil::is_dir(&path) {
                autocode_core::utils::fsutil::remove_dir(&path)
            } else {
                autocode_core::utils::fsutil::remove_file(&path)
            };
            match result {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!("Deleted: {}", path.display())
                }
                Err(e) => helpers::tool_error(
                    &format!("Error deleting {}: {}", path.display(), e),
                    "Ensure the path exists and you have permission; use list_dir to verify",
                ),
            }
        }

        "rename_file" => {
            let raw_from = match args["from"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'from' argument".to_string(),
            };
            let raw_to = match args["to"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'to' argument".to_string(),
            };
            let from = resolve_path_cached(raw_from, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&from) {
                return autocode_core::helpers::blocked_error(raw_from);
            }
            let to = resolve_path_write_cached(raw_to, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&to) {
                return autocode_core::helpers::blocked_error(raw_to);
            }
            if let Some(parent) = to.parent()
                && let Err(e) = autocode_core::utils::fsutil::create_dir_all(parent)
            {
                return helpers::tool_error(
                    &format!(
                        "Error creating parent directory for {}: {}",
                        to.display(),
                        e
                    ),
                    "Check that the destination path is writable",
                );
            }
            match autocode_core::utils::fsutil::rename(&from, &to) {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!("Renamed {} -> {}", from.display(), to.display())
                }
                Err(e) => helpers::tool_error(
                    &format!(
                        "Error renaming {} -> {}: {}",
                        from.display(),
                        to.display(),
                        e
                    ),
                    "Verify the source path exists and the destination is writable",
                ),
            }
        }

        "create_dir" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            match autocode_core::utils::fsutil::create_dir_all(&path) {
                Ok(_) => format!("Created directory: {}", path.display()),
                Err(e) => format!("Error creating dir {}: {}", path.display(), e),
            }
        }

        "grep" => {
            let pattern = match args["pattern"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'pattern' argument".to_string(),
            };
            let search_root = args["path"].as_str().unwrap_or(project_root);
            let search_path =
                resolve_path_cached(search_root, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&search_path) {
                return autocode_core::helpers::blocked_error(search_root);
            }
            let file_glob = args["file_glob"].as_str().unwrap_or("*");
            let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(true);
            let max_results = args["max_results"].as_u64().unwrap_or(50).min(200) as usize;

            autocode_fs::explorer::grep_files(
                &search_path,
                pattern,
                file_glob,
                case_sensitive,
                max_results,
            )
        }

        "patch_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let raw_old_text = match args["old_text"].as_str() {
                Some(t) => t,
                None => return "Error: missing 'old_text' argument".to_string(),
            };
            let raw_new_text = args["new_text"].as_str().unwrap_or("");
            let replace_all = args["replace_all"].as_bool().unwrap_or(false);

            // Strip line-number prefixes if the AI copied from read_file output
            let old_text = helpers::strip_line_numbers(raw_old_text);
            let new_text = helpers::strip_line_numbers(raw_new_text);

            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            let content = match autocode_core::utils::fsutil::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    return helpers::tool_error(
                        &format!("Error reading {}: {}", path.display(), e),
                        "Verify the path exists with list_dir before patching",
                    );
                }
            };

            match helpers::fuzzy_find_replace(&content, &old_text, &new_text, replace_all) {
                Some((patched, strategy, start_line)) => {
                    match autocode_core::utils::fsutil::write(&path, &patched) {
                        Ok(_) => {
                            autocode_fs::git::invalidate_git_cache(std::path::Path::new(
                                project_root,
                            ));
                            // start_line is 0-based; convert to 1-based for display
                            let line_num = start_line + 1;
                            format!(
                                "Patched {} via {} ({} -> {} bytes, line {})",
                                path.display(),
                                strategy,
                                content.len(),
                                patched.len(),
                                line_num,
                            )
                        }
                        Err(e) => helpers::tool_error(
                            &format!("Error writing {}: {}", path.display(), e),
                            "Check that the path is writable",
                        ),
                    }
                }
                None => {
                    let old_lines: Vec<&str> = old_text.lines().collect();
                    let first_old = old_lines.first().copied().unwrap_or("");
                    let nearby = helpers::find_nearby_lines(&content, first_old, 5);
                    format!(
                        "Error: 'old_text' not found in {}. No changes made.\n\
                         --- old_text (first line) ---\n{}\n\
                         --- nearest lines in file ---\n{}\n\
                         --- tip ---\n\
                         Re-read the file with read_file and copy the exact text for old_text.",
                        path.display(),
                        if old_text.len() > 500 {
                            format!("{}... ({} chars total)", &old_text[..500], old_text.len())
                        } else {
                            old_text.to_string()
                        },
                        nearby,
                    )
                }
            }
        }

        "patch_lines" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let start_line = args["start_line"].as_u64().unwrap_or(0) as usize;
            let end_line = args["end_line"].as_u64().unwrap_or(0) as usize;
            let new_text = args["new_text"].as_str().unwrap_or("");

            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if autocode_core::helpers::is_blocked_path(&path) {
                return autocode_core::helpers::blocked_error(raw_path);
            }
            let content = match autocode_core::utils::fsutil::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    return helpers::tool_error(
                        &format!("Error reading {}: {}", path.display(), e),
                        "Verify the path exists with list_dir before patching",
                    );
                }
            };

            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            if start_line < 1 || start_line > total {
                return format!(
                    "Error: start_line {} out of range (file has {} lines)",
                    start_line, total,
                );
            }
            if end_line < start_line || end_line > total {
                return format!(
                    "Error: end_line {} out of range (file has {} lines)",
                    end_line, total,
                );
            }

            let ends_with_nl = content.ends_with('\n');
            let mut result = String::with_capacity(content.len() + new_text.len());
            for line in lines[..start_line - 1].iter() {
                result.push_str(line);
                result.push('\n');
            }
            result.push_str(new_text);
            if !new_text.ends_with('\n') {
                result.push('\n');
            }
            for line in lines[end_line..].iter() {
                result.push_str(line);
                result.push('\n');
            }
            if !ends_with_nl && result.ends_with('\n') {
                result.pop();
            }

            match autocode_core::utils::fsutil::write(&path, &result) {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!(
                        "Patched {} lines {}-{} ({} -> {} bytes)",
                        path.display(),
                        start_line,
                        end_line,
                        content.len(),
                        result.len(),
                    )
                }
                Err(e) => helpers::tool_error(
                    &format!("Error writing {}: {}", path.display(), e),
                    "Check that the path is writable",
                ),
            }
        }

        "web_search" => {
            let query = match args["query"].as_str() {
                Some(q) => q,
                None => return "Error: missing 'query' argument".to_string(),
            };
            let num_results = args["num_results"].as_u64().unwrap_or(5).min(10) as usize;

            let cache_key = format!("ddg:{}:{}", query, num_results);
            if let Some(cached) = autocode_core::utils::extract::search_cache_get(&cache_key) {
                return cached;
            }

            let encoded: String = query
                .chars()
                .map(|c| match c {
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                    ' ' => "+".to_string(),
                    c => format!("%{:02X}", c as u32),
                })
                .collect();

            let url = format!("https://html.duckduckgo.com/html/?q={}", encoded);
            match crate::provider::native_get(&url, 15, 512_000) {
                Err(e) => format!("Web search error: {}", e),
                Ok(data) => {
                    let html = String::from_utf8_lossy(&data);
                    let results =
                        autocode_core::utils::extract::extract_ddg_results(&html, num_results);
                    if results.is_empty() {
                        format!("No web results for \"{}\"", query)
                    } else {
                        autocode_core::utils::extract::search_cache_set(&cache_key, &results);
                        results
                    }
                }
            }
        }

        "fetch_url" => {
            let url = match args["url"].as_str() {
                Some(u) => u,
                None => return "Error: missing 'url' argument".to_string(),
            };
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return helpers::tool_error(
                    &format!(
                        "Invalid URL scheme for \"{}\": only http/https allowed",
                        url
                    ),
                    "Provide an http:// or https:// URL",
                );
            }
            let max_bytes = args["max_bytes"].as_u64().unwrap_or(32_768).min(131_072) as usize;

            match crate::provider::native_get(url, 20, max_bytes) {
                Err(e) => format!("fetch error for {}: {}", url, e),
                Ok(data) => {
                    let body = String::from_utf8_lossy(&data);
                    let is_html = body.trim_start().starts_with("<!")
                        || body.trim_start().starts_with("<html")
                        || body.contains("<html");
                    let text = if is_html {
                        autocode_core::utils::extract::extract_html_content(&body, url)
                    } else {
                        body.to_string()
                    };

                    if text.trim().is_empty() {
                        format!("Empty response from {}", url)
                    } else {
                        // Cap at max_bytes to prevent runaway token usage
                        text.chars().take(max_bytes).collect()
                    }
                }
            }
        }

        "todo_list" => {
            let items_val = match args["task_items"].as_array() {
                Some(a) => a,
                None => return "Error: missing 'task_items' array".to_string(),
            };
            let items: Vec<TodoItem> = items_val
                .iter()
                .filter_map(|v| {
                    let id = v["id"].as_str()?.to_string();
                    let content = v["content"].as_str()?.to_string();
                    let status_str = v["status"].as_str().unwrap_or("pending");
                    let status = match status_str {
                        "completed" => TodoStatus::Completed,
                        "in_progress" => TodoStatus::InProgress,
                        "cancelled" => TodoStatus::Cancelled,
                        _ => TodoStatus::Pending,
                    };
                    let priority = v["priority"].as_str().unwrap_or("medium").to_string();
                    Some(TodoItem {
                        id,
                        content,
                        status,
                        priority,
                    })
                })
                .collect();
            let done = items
                .iter()
                .filter(|i| i.status == TodoStatus::Completed)
                .count();
            let total = items.len();
            let name_hint = if !session_named {
                " | Session: call name_session."
            } else {
                ""
            };
            format!(
                "Task list updated -- {}/{} complete | {}{}",
                done,
                total,
                super::super::session_ops::format_context_usage(ctx_used, ctx_max, max_output),
                name_hint,
            )
        }

        "project_task_list" => {
            let items_val = match args["task_items"].as_array() {
                Some(a) => a,
                None => return "Error: missing 'task_items' array".to_string(),
            };
            let items: Vec<TodoItem> = items_val
                .iter()
                .filter_map(|v| {
                    let id = v["id"].as_str()?.to_string();
                    let content = v["content"].as_str()?.to_string();
                    let status_str = v["status"].as_str().unwrap_or("pending");
                    let status = match status_str {
                        "completed" => TodoStatus::Completed,
                        "in_progress" => TodoStatus::InProgress,
                        "cancelled" => TodoStatus::Cancelled,
                        _ => TodoStatus::Pending,
                    };
                    let priority = v["priority"].as_str().unwrap_or("medium").to_string();
                    Some(TodoItem {
                        id,
                        content,
                        status,
                        priority,
                    })
                })
                .collect();
            let done = items
                .iter()
                .filter(|i| i.status == TodoStatus::Completed)
                .count();
            let total = items.len();
            format!(
                "Project tasks updated -- {}/{} complete | {}",
                done,
                total,
                super::super::session_ops::format_context_usage(ctx_used, ctx_max, max_output),
            )
        }

        "glob" => {
            let pattern = match args["pattern"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'pattern' argument".to_string(),
            };
            let search_path = Some(
                args["path"]
                    .as_str()
                    .map(|p| resolve_path_cached(p, project_root, path_cache, allow_escape))
                    .unwrap_or_else(|| std::path::PathBuf::from(&project_root)),
            );
            if let Some(ref sp) = search_path
                && autocode_core::helpers::is_blocked_path(sp)
            {
                return autocode_core::helpers::blocked_error(
                    args["path"].as_str().unwrap_or(project_root),
                );
            }
            let results = autocode_fs::explorer::glob_files(search_path.as_deref(), pattern);
            if results.is_empty() {
                format!("No files match '{}'", pattern)
            } else {
                format!(
                    "{} file(s) matching '{}':\n{}",
                    results.len(),
                    pattern,
                    results.join("\n")
                )
            }
        }

        "get_skill" => {
            let keyword = match args["keyword"].as_str() {
                Some(k) => k.trim(),
                None => return "Error: missing 'keyword' argument".to_string(),
            };
            if keyword.is_empty() {
                let dir = autocode_fs::skills::skills_dir(std::path::Path::new(project_root));
                let skills = autocode_fs::skills::list_skills_with_info(&dir);
                if skills.is_empty() {
                    return "No skill files found.".to_string();
                }
                let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
                return format!("Available skills: {}", names.join(", "));
            }

            let dir = autocode_fs::skills::skills_dir(std::path::Path::new(project_root));
            let skills = autocode_fs::skills::list_skills_with_info(&dir);
            if skills.is_empty() {
                return format!(
                    "No skills directory found at {} (or it's empty).",
                    dir.display()
                );
            }

            let read_one = |s: &autocode_fs::skills::SkillInfo| -> String {
                match autocode_fs::skills::read_skill(&dir, &s.name) {
                    Ok(content) => content,
                    Err(e) => format!("Error reading skill '{}': {}", s.name, e),
                }
            };

            // Single pass: exact, fuzzy, and substring match simultaneously.
            let kw_lower = keyword.to_lowercase();
            let kw_short = keyword.len() < 3;
            let mut exact: Option<&autocode_fs::skills::SkillInfo> = None;
            let mut fuzzy: Vec<(&autocode_fs::skills::SkillInfo, f64)> = Vec::new();
            let mut sub: Vec<&autocode_fs::skills::SkillInfo> = Vec::new();

            for s in skills.iter() {
                let n_lower = s.name.to_lowercase();
                let d_lower = s.description.to_lowercase();

                if n_lower == kw_lower || d_lower == kw_lower {
                    exact = Some(s);
                    break;
                }

                let ns = helpers::similarity_score(&s.name, keyword);
                let ds = helpers::similarity_score(&s.description, keyword);
                if ns >= 0.35 || ds >= 0.35 {
                    fuzzy.push((s, ns.max(ds)));
                }

                if !kw_short && (n_lower.contains(&kw_lower) || d_lower.contains(&kw_lower)) {
                    sub.push(s);
                }
            }

            if let Some(s) = exact {
                return read_one(s);
            }

            if !fuzzy.is_empty() {
                fuzzy.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                if fuzzy.len() == 1 || fuzzy[0].1 - fuzzy[1].1 >= 0.15 {
                    return read_one(fuzzy[0].0);
                }
                let candidates: Vec<&str> =
                    fuzzy.iter().take(5).map(|(s, _)| s.name.as_str()).collect();
                return format!(
                    "Multiple skills match '{}': {}. Call get_skill again with the exact name.",
                    keyword,
                    candidates.join(", ")
                );
            }

            if !sub.is_empty() {
                if sub.len() == 1 {
                    return read_one(sub[0]);
                }
                let candidates: Vec<&str> = sub.iter().take(5).map(|s| s.name.as_str()).collect();
                return format!(
                    "Multiple skills match '{}': {}. Call get_skill again with the exact name.",
                    keyword,
                    candidates.join(", ")
                );
            }

            let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
            format!(
                "No skill matching '{}'. Available skills: {}",
                keyword,
                names.join(", ")
            )
        }

        "verify_proof" => super::proof::run_verify_proof(project_root, &args),

        "search_literature" => {
            let query = args["query"].as_str().unwrap_or("");
            let max_results = args["max_results"].as_u64().unwrap_or(5).min(20);
            if query.is_empty() {
                return "Error: missing 'query' argument".to_string();
            }
            let url = format!(
                "https://export.arxiv.org/api/query?search_query=all:{}&start=0&max_results={}",
                urlencoding(query),
                max_results
            );
            match crate::provider::native_get(&url, 15, 65536) {
                Ok(bytes) => {
                    let xml = String::from_utf8_lossy(&bytes);
                    let mut results = Vec::new();
                    for entry in xml.split("<entry>").skip(1) {
                        let title = entry
                            .split("<title>")
                            .nth(1)
                            .and_then(|s| s.split("</title>").next())
                            .unwrap_or("")
                            .trim()
                            .replace('\n', " ")
                            .replace("  ", " ");
                        let authors: Vec<String> = entry
                            .split("<author>")
                            .skip(1)
                            .filter_map(|a| a.split("<name>").nth(1))
                            .filter_map(|n| n.split("</name>").next())
                            .map(|s| s.trim().to_string())
                            .collect();
                        let summary = entry
                            .split("<summary>")
                            .nth(1)
                            .and_then(|s| s.split("</summary>").next())
                            .unwrap_or("")
                            .trim()
                            .chars()
                            .take(300)
                            .collect::<String>();
                        results.push(format!(
                            "---\nTitle: {}\nAuthors: {}\nAbstract: {}...\n",
                            title,
                            authors.join("; "),
                            summary
                        ));
                    }
                    if results.is_empty() {
                        format!("No arXiv results for '{}'.", query)
                    } else {
                        results.join("\n")
                    }
                }
                Err(e) => format!("arXiv query failed: {}. Try web_search instead.", e),
            }
        }

        "explore_theorem" => {
            let theorem = args["theorem"].as_str().unwrap_or("");
            let action = args["action"].as_str().unwrap_or("status");
            let goal_id = args["goal_id"].as_str().unwrap_or("");
            let notes = args["notes"].as_str().unwrap_or("");
            let sub_goals: Vec<&str> = args["sub_goals"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            if theorem.is_empty() {
                return "Error: missing 'theorem' argument".to_string();
            }

            let state_dir = std::path::Path::new(project_root)
                .join(".autocode")
                .join("theorems");
            let _ = std::fs::create_dir_all(&state_dir);
            let safe_name: String = theorem
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .take(64)
                .collect();
            let state_path = state_dir.join(format!("{}.json", safe_name));

            match action {
                "init" => {
                    let state = serde_json::json!({
                        "theorem": theorem,
                        "goals": {"1": {"statement": theorem, "children": [], "status": "pending", "notes": notes}},
                        "next_id": 2,
                    });
                    write_json(&state_path, &state);
                    format!(
                        "Initialized theorem exploration for: {}\nRoot goal ID: 1",
                        theorem
                    )
                }
                "refine" => {
                    if goal_id.is_empty() {
                        return "Error: 'goal_id' required for refine action".to_string();
                    }
                    if sub_goals.is_empty() {
                        return "Error: 'sub_goals' required for refine action".to_string();
                    }
                    let mut state = read_json(&state_path);
                    if state.is_null() {
                        return format!("No state for '{}'. Call action=init first.", theorem);
                    }
                    let next_id = state["next_id"].as_u64().unwrap_or(2);
                    let mut child_ids = Vec::new();
                    let mut sid = next_id;
                    for sg in &sub_goals {
                        let cid = sid.to_string();
                        state["goals"][&cid] = serde_json::json!({"statement": sg, "children": [], "status": "pending", "notes": ""});
                        if let Some(children) = state["goals"][goal_id]["children"].as_array_mut() {
                            children.push(serde_json::json!(cid));
                        }
                        child_ids.push(cid);
                        sid += 1;
                    }
                    state["next_id"] = serde_json::json!(sid);
                    write_json(&state_path, &state);
                    format!("Refined goal {} into: {}", goal_id, child_ids.join(", "))
                }
                "prove" | "fail" => {
                    if goal_id.is_empty() {
                        return "Error: 'goal_id' required".to_string();
                    }
                    let mut state = read_json(&state_path);
                    if state.is_null() {
                        return format!("No state for '{}'. Call action=init first.", theorem);
                    }
                    let status = if action == "prove" {
                        "proven"
                    } else {
                        "failed"
                    };
                    if let Some(g) = state["goals"][goal_id].as_object_mut() {
                        g.insert("status".into(), serde_json::json!(status));
                        if !notes.is_empty() {
                            g.insert("notes".into(), serde_json::json!(notes));
                        }
                    }
                    write_json(&state_path, &state);
                    format!(
                        "Marked goal {} as {}.{}",
                        goal_id,
                        status,
                        if notes.is_empty() {
                            "".into()
                        } else {
                            format!(" Notes: {}", notes)
                        }
                    )
                }
                _ => {
                    let content = std::fs::read_to_string(&state_path).unwrap_or_default();
                    if content.is_empty() {
                        return format!("No state for '{}'. Call action=init.", theorem);
                    }
                    format!("Theorem state:\n{}", content)
                }
            }
        }

        "handoff" => {
            let reason = args["reason"].as_str().unwrap_or("no reason given");
            let next_prompt = args["next_prompt"].as_str().unwrap_or("");
            format!("HANDOFF:{}|||NEXT:{}", reason, next_prompt)
        }

        other => {
            format!("Unknown tool: {}", other)
        }
    }
}
