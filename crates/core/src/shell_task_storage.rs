use std::path::PathBuf;
use crate::state::ShellTask;
use crate::fsutil;

fn tasks_dir(data_dir_name: &str) -> PathBuf {
    fsutil::exe_dir()
        .join("AutoCode_data")
        .join("projects")
        .join(data_dir_name)
        .join("shell_tasks")
}

pub fn save_task(data_dir_name: &str, task: &ShellTask) -> std::io::Result<()> {
    let dir = tasks_dir(data_dir_name);
    fsutil::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", task.id));
    let json = serde_json::to_string_pretty(task)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fsutil::write(&path, json)
}

pub fn load_task(data_dir_name: &str, task_id: &str) -> Option<ShellTask> {
    let path = tasks_dir(data_dir_name).join(format!("{}.json", task_id));
    if !path.exists() {
        return None;
    }
    let content = fsutil::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn list_tasks(data_dir_name: &str) -> Vec<ShellTask> {
    let dir = tasks_dir(data_dir_name);
    if !dir.exists() {
        return Vec::new();
    }
    let mut tasks = Vec::new();
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.to_string_lossy().ends_with(".json") {
                continue;
            }
            if let Ok(content) = fsutil::read_to_string(&path)
                && let Ok(task) = serde_json::from_str::<ShellTask>(&content)
            {
                tasks.push(task);
            }
        }
    }
    tasks.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    tasks
}

pub fn delete_task(data_dir_name: &str, task_id: &str) -> std::io::Result<()> {
    let path = tasks_dir(data_dir_name).join(format!("{}.json", task_id));
    if path.exists() {
        fsutil::remove_file(&path)?;
    }
    Ok(())
}

pub fn prune_tasks(data_dir_name: &str, max_count: usize) {
    let mut tasks = list_tasks(data_dir_name);
    if tasks.len() <= max_count {
        return;
    }
    tasks.truncate(max_count);
    let keep: std::collections::HashSet<String> =
        tasks.into_iter().map(|t| t.id).collect();
    let dir = tasks_dir(data_dir_name);
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".json") {
                continue;
            }
            let id = name.trim_end_matches(".json").to_string();
            if !keep.contains(&id) {
                if let Err(e) = fsutil::remove_file(&entry.path()) {
                    eprintln!("[shell_task_storage] Failed to remove pruned task file {:?}: {}", entry.path(), e);
                }
            }
        }
    }
}
