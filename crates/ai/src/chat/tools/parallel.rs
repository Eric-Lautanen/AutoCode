// tools/parallel.rs -- Parallel execution of non-shell tool batches.
//
// Batches are partitioned into conflict groups: calls sharing a mutated path
// (writes/patches/deletes/renames -- or a rename's from/to pair) serialize in
// one group, reads and independent web/skill tools always parallelize, and
// verify_proof groups on its shared attempts.jsonl log. Groups run on
// std::thread::scope workers (at most 4); each worker owns a fresh
// LruPathCache which is returned to the caller for merge-back so the
// runtime-owned cache keeps its cross-batch reuse property.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::chat::runtime::ToolResult;
use crate::provider::ToolCall;
use autocode_core::helpers::LruPathCache;

use super::{ToolExecCtx, build_tool_meta, execute_tool_with_cache};
use crate::helpers;

/// Snapshot of per-session values the batch needs; cloned once instead of
/// borrowing AppState from a scoped worker.
#[derive(Clone)]
pub(crate) struct BatchCtx {
    pub project_root: String,
    pub allow_escape: bool,
    pub session_named: bool,
    pub chrome_path: Option<String>,
    pub use_headless_chrome: bool,
    pub current_todo: autocode_core::state::TodoList,
    pub current_project_tasks: autocode_core::state::TodoList,
}

/// Normalize a raw path argument into a comparable conflict key: separators
/// unified, "."/".." segments collapsed (case-folded on Windows where paths
/// are case-insensitive).
pub(crate) fn normalize_key(raw: &str) -> String {
    let s = raw.trim().replace('\\', "/");
    let mut stack: Vec<&str> = Vec::new();
    for seg in s.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    let joined = stack.join("/");
    #[cfg(target_os = "windows")]
    let joined = joined.to_lowercase();
    joined
}

/// Conflict keys a tool call mutates: calls sharing any key must serialize.
/// Pure readers and independent network/skill tools return no keys.
fn conflict_keys(tc: &ToolCall) -> Vec<String> {
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    let path_key = |v: &serde_json::Value| -> Vec<String> {
        v.as_str()
            .map(normalize_key)
            .filter(|s| !s.is_empty())
            .into_iter()
            .collect()
    };
    match tc.name.as_str() {
        "write_file" | "patch_file" | "patch_lines" | "delete_file" | "create_dir" => {
            path_key(&args["path"])
        }
        "rename_file" => {
            let mut keys = path_key(&args["from"]);
            keys.extend(path_key(&args["to"]));
            keys
        }
        // All verifier attempts append to the same attempts.jsonl log.
        "verify_proof" => vec![normalize_key("proofs/attempts.jsonl")],
        _ => Vec::new(),
    }
}

/// Partition `calls` into serial groups; every returned index list is in
/// original batch order and any two calls sharing a conflict key land in the
/// same group. Groups are disjoint over indices.
pub(crate) fn group_batch(calls: &[ToolCall]) -> Vec<Vec<usize>> {
    let mut groups: Vec<(Vec<usize>, std::collections::HashSet<String>)> = Vec::new();
    let mut key_to_group: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (idx, tc) in calls.iter().enumerate() {
        let keys = conflict_keys(tc);
        // Find the group this call joins; if its keys span several existing
        // groups (only rename_file carries two keys), absorb the extras into
        // the first. Absorbed groups are emptied in place and filtered at the
        // end -- removing them mid-loop would invalidate key_to_group.
        if let Some(target) = keys.iter().find_map(|k| key_to_group.get(k).copied()) {
            for k in &keys {
                if let Some(other) = key_to_group.get(k).copied()
                    && other != target
                {
                    let moved_keys = std::mem::take(&mut groups[other].1);
                    let moved_idx = std::mem::take(&mut groups[other].0);
                    for k in &moved_keys {
                        key_to_group.insert(k.clone(), target);
                        groups[target].1.insert(k.clone());
                    }
                    groups[target].0.extend(moved_idx);
                }
            }
            groups[target].0.push(idx);
            for k in keys {
                key_to_group.insert(k.clone(), target);
                groups[target].1.insert(k);
            }
        } else {
            let gi = groups.len();
            let key_set: std::collections::HashSet<String> = keys.iter().cloned().collect();
            for k in keys {
                key_to_group.insert(k, gi);
            }
            groups.push((vec![idx], key_set));
        }
    }

    groups
        .into_iter()
        .filter_map(|(idx, _)| (!idx.is_empty()).then_some(idx))
        .collect()
}

/// Execute one tool call with panic isolation, timing log, and ToolMeta
/// assembly. Returns `(original_index, result)`.
fn execute_one(
    idx: usize,
    tc: &ToolCall,
    ctx: &BatchCtx,
    cache: &mut LruPathCache,
) -> (usize, ToolResult) {
    let start = std::time::Instant::now();
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execute_tool_with_cache(ToolExecCtx {
            tc,
            project_root: &ctx.project_root,
            path_cache: cache,
            allow_escape: ctx.allow_escape,
            session_named: ctx.session_named,
            chrome_path: ctx.chrome_path.clone(),
            use_headless_chrome: ctx.use_headless_chrome,
            current_todo: ctx.current_todo.clone(),
            current_project_tasks: ctx.current_project_tasks.clone(),
        })
    })) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!(
                "Tool '{}' panicked: {}",
                tc.name,
                autocode_core::helpers::panic_msg(&e)
            );
            helpers::tool_error(&msg, "Re-read the file and try a smaller edit")
        }
    };
    helpers::log_timing(|| {
        format!(
            "tool {} {} -> {}",
            tc.name,
            helpers::format_duration(start.elapsed()),
            if result.starts_with("Error") {
                "error"
            } else {
                "ok"
            }
        )
    });
    let duration_ms = start.elapsed().as_millis() as u64;
    let meta = build_tool_meta(
        tc,
        &result,
        duration_ms,
        &ctx.current_todo,
        &ctx.current_project_tasks,
    );
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    let accessed_paths = match tc.name.as_str() {
        "read_file" | "read_entire_file" | "write_file" | "patch_file" | "patch_lines"
        | "delete_file" | "list_dir" | "grep" | "glob" | "project_tree" | "create_dir" => args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| vec![p.to_string()])
            .unwrap_or_default(),
        "read_files" => args
            .get("paths")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        "rename_file" => {
            let mut paths = Vec::new();
            if let Some(p) = args.get("from").and_then(|v| v.as_str()) {
                paths.push(p.to_string());
            }
            if let Some(p) = args.get("to").and_then(|v| v.as_str()) {
                paths.push(p.to_string());
            }
            paths
        }
        _ => vec![],
    };
    // A "read" action must never overwrite the stored list. Only capture an
    // update when the action is not "read".
    let is_read = args["action"].as_str() == Some("read");
    let todo_update = if tc.name == "todo_list" && !is_read {
        helpers::parse_todo_from_tool_args(&args)
    } else {
        None
    };
    let project_todo_update = if tc.name == "project_task_list" && !is_read {
        helpers::parse_project_task_from_tool_args(&args)
    } else {
        None
    };
    (
        idx,
        ToolResult {
            tool_call: tc.clone(),
            content: result.to_string(),
            meta,
            accessed_paths,
            todo_update,
            project_todo_update,
        },
    )
}

/// Run the batch: group into conflict groups, execute groups on scoped
/// workers (at most 4), return results in ORIGINAL tool_call order plus the
/// per-group path caches for merge-back into the runtime-owned cache.
pub(crate) fn execute_batch(
    calls: &[ToolCall],
    ctx: &BatchCtx,
) -> (Vec<ToolResult>, Vec<LruPathCache>) {
    let groups = group_batch(calls);
    let workers = groups.len().min(4);
    let results: std::sync::Mutex<Vec<(usize, ToolResult)>> = std::sync::Mutex::new(Vec::new());
    let caches: std::sync::Mutex<Vec<LruPathCache>> = std::sync::Mutex::new(Vec::new());
    let next_group = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let g = next_group.fetch_add(1, Ordering::Relaxed);
                    if g >= groups.len() {
                        break;
                    }
                    let mut cache = LruPathCache::new();
                    for &ci in &groups[g] {
                        let (i, r) = execute_one(ci, &calls[ci], ctx, &mut cache);
                        if let Ok(mut guard) = results.lock() {
                            guard.push((i, r));
                        }
                    }
                    if let Ok(mut guard) = caches.lock() {
                        guard.push(cache);
                    }
                }
            });
        }
    });

    let mut ordered = results.into_inner().unwrap_or_default();
    ordered.sort_by_key(|(i, _)| *i);
    (
        ordered.into_iter().map(|(_, r)| r).collect(),
        caches.into_inner().unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ToolCall;

    fn call(name: &str, json: &str) -> ToolCall {
        ToolCall {
            id: format!("call_{}", name),
            name: name.to_string(),
            arguments: json.to_string(),
        }
    }

    #[test]
    fn writes_serialize_reads_parallelize() {
        let calls = vec![
            call("write_file", r#"{"path":"src/a.rs"}"#),
            call("read_file", r#"{"path":"src/b.rs"}"#),
            call("read_file", r#"{"path":"./src/../src/a.rs"}"#),
            call("patch_file", r#"{"path":"src\\a.rs"}"#),
            call("rename_file", r#"{"from":"c.txt","to":"d.txt"}"#),
            call("write_file", r#"{"path":"d.txt"}"#),
        ];
        let groups = group_batch(&calls);
        // write+patch on a.rs share one group (normalized "./x/../" and "\").
        // The read of a.rs stays parallel. rename c->d chains with write d.
        assert_eq!(groups, vec![vec![0, 3], vec![1], vec![2], vec![4, 5]]);
    }

    #[test]
    fn rename_links_both_endpoints() {
        let calls = vec![
            call("write_file", r#"{"path":"x.rs"}"#),
            call("rename_file", r#"{"from":"y.rs","to":"x.rs"}"#),
            call("delete_file", r#"{"path":"y.rs"}"#),
        ];
        let groups = group_batch(&calls);
        // All three serialize through the x/y endpoint chain.
        assert_eq!(groups, vec![vec![0, 1, 2]]);
    }

    #[test]
    fn rename_spanning_two_groups_merges_them() {
        let calls = vec![
            call("write_file", r#"{"path":"a.txt"}"#),
            call("write_file", r#"{"path":"b.txt"}"#),
            call("rename_file", r#"{"from":"a.txt","to":"b.txt"}"#),
            call("read_file", r#"{"path":"zz.txt"}"#),
        ];
        let groups = group_batch(&calls);
        // The rename's from/to span both write groups: all three merge.
        assert_eq!(groups, vec![vec![0, 1, 2], vec![3]]);
    }

    #[test]
    fn verify_proof_shares_attempts_log() {
        let calls = vec![
            call("verify_proof", r#"{"statement":"a","proof_code":"b"}"#),
            call("verify_proof", r#"{"statement":"c","proof_code":"d"}"#),
            call("web_search", r#"{"query":"q"}"#),
        ];
        let groups = group_batch(&calls);
        assert_eq!(groups, vec![vec![0, 1], vec![2]]);
    }

    #[test]
    fn empty_and_single_batches() {
        assert!(group_batch(&[]).is_empty());
        let single = vec![call("read_file", r#"{"path":"a"}"#)];
        assert_eq!(group_batch(&single), vec![vec![0]]);
    }

    #[test]
    fn results_preserve_original_order_despite_parallelism() {
        let dir = std::env::temp_dir().join(format!("ac_par_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let files: Vec<String> = (0..8)
            .map(|i| {
                let p = dir.join(format!("f{}.txt", i));
                // Later files get bigger content so earlier reads finish first.
                let content = "x".repeat((i + 1) * 4096);
                std::fs::write(&p, &content).unwrap();
                p.to_string_lossy().replace('\\', "/")
            })
            .collect();
        let calls: Vec<ToolCall> = files
            .iter()
            .map(|f| {
                call(
                    "read_entire_file",
                    &format!(r#"{{"path":"{}","entire":true}}"#, f),
                )
            })
            .collect();
        let ctx = BatchCtx {
            project_root: dir.to_string_lossy().to_string(),
            allow_escape: true,
            session_named: true,
            chrome_path: None,
            use_headless_chrome: false,
            current_todo: Default::default(),
            current_project_tasks: Default::default(),
        };
        let (results, caches) = execute_batch(&calls, &ctx);
        assert_eq!(results.len(), 8);
        for (i, r) in results.iter().enumerate() {
            let expected = files[i].rsplit('/').next().unwrap_or_default();
            assert!(
                r.meta
                    .file_path
                    .as_deref()
                    .unwrap_or("")
                    .ends_with(expected)
                    || r.tool_call.arguments.contains(expected),
                "result {} does not match call {}: {:?}",
                i,
                i,
                r.meta.file_path
            );
        }
        assert!(!caches.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
