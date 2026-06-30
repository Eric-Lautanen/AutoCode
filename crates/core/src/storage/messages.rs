use crate::state::ChatMessage;
use crate::utils::fsutil;
use std::path::Path;

/// The single JSONL file holding all messages for a session.
const MESSAGES_FILE: &str = "messages.jsonl";

fn messages_path(dir: &Path) -> std::path::PathBuf {
    dir.join(MESSAGES_FILE)
}

/// One-time migration: if legacy chunked files (`messages_XXXX.jsonl`) exist,
/// concatenate them in order into a single `messages.jsonl` and delete the
/// old chunks. Idempotent — if `messages.jsonl` already exists, the chunk
/// files are treated as stale leftovers and deleted without merging.
///
/// This runs lazily on every read/append so it works regardless of which
/// session is opened first. The cost is a single directory scan per call,
/// which is negligible (a handful of entries).
fn migrate_from_chunks(dir: &Path) {
    let chunks = find_chunk_files(dir);
    if chunks.is_empty() {
        return;
    }
    let target = messages_path(dir);

    // If the single file doesn't exist yet, build it from the chunks.
    if !target.exists() {
        let pid = std::process::id();
        let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp_path = dir.join(format!(".tmp_migrate_{}_{}.jsonl", pid, n));
        let migrated = {
            use std::io::Write;
            let Ok(tmp_file) = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(fsutil::extended_path(&tmp_path))
            else {
                return; // another writer beat us — leave chunks for next time
            };
            let mut writer = std::io::BufWriter::new(tmp_file);
            let mut ok = true;
            for path in &chunks {
                if let Ok(content) = fsutil::read_to_string(path) {
                    for line in content.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        // Only copy lines that deserialize — drops any
                        // corrupt trailing partial from a past crash.
                        if serde_json::from_str::<ChatMessage>(line).is_ok()
                            && writeln!(writer, "{}", line).is_err()
                        {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    break;
                }
            }
            if writer.flush().is_err() {
                ok = false;
            }
            ok
        };

        if migrated && fsutil::rename(&tmp_path, &target).is_ok() {
            // Success — delete the old chunk files.
            for path in &chunks {
                let _ = fsutil::remove_file(path);
            }
            return;
        }
        // Migration failed — clean up the temp file, leave chunks intact
        // so a future call can retry.
        let _ = fsutil::remove_file(&tmp_path);
        return;
    }

    // The single file already exists. The chunk files are stale leftovers
    // from a partial/aborted migration — safe to delete.
    for path in &chunks {
        let _ = fsutil::remove_file(path);
    }
}

/// Find legacy chunk files (`messages_XXXX.jsonl`) in sorted order.
fn find_chunk_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut chunks = Vec::new();
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("messages_") && name.ends_with(".jsonl") {
                chunks.push(entry.path());
            }
        }
    }
    chunks.sort();
    chunks
}

/// Check whether the messages file exists in the given directory.
/// Also triggers migration from legacy chunk files if any are present.
pub fn has_messages_file(dir: &Path) -> bool {
    migrate_from_chunks(dir);
    messages_path(dir).exists()
}

/// Append messages to the single JSONL file. One open, one flush.
pub fn append_messages(
    dir: &Path,
    _session_id: &str,
    _session_label: &str,
    messages: &[ChatMessage],
) -> std::io::Result<()> {
    if messages.is_empty() {
        return Ok(());
    }
    migrate_from_chunks(dir);
    fsutil::create_dir_all(dir)?;

    let path = messages_path(dir);
    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(fsutil::extended_path(&path))?;
    let mut writer = std::io::BufWriter::new(file);

    for msg in messages {
        let line = serde_json::to_string(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(writer, "{}", line)?;
    }

    writer.flush()?;
    Ok(())
}

/// Read all messages from the JSONL file, in order.
pub fn read_all_messages(dir: &Path) -> Vec<ChatMessage> {
    migrate_from_chunks(dir);
    let path = messages_path(dir);
    let Ok(content) = fsutil::read_to_string(&path) else {
        return Vec::new();
    };
    let mut all = Vec::new();
    for line in content.lines().filter(|l| !l.is_empty()) {
        if let Ok(msg) = serde_json::from_str::<ChatMessage>(line) {
            all.push(msg);
        }
    }
    all
}

/// Load messages with IDs less than `before_id`, up to `count` messages.
/// Reads the file and collects matching messages in ascending order.
pub fn load_messages_before(dir: &Path, before_id: u64, count: usize) -> Vec<ChatMessage> {
    migrate_from_chunks(dir);
    let path = messages_path(dir);
    let Ok(content) = fsutil::read_to_string(&path) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for line in content.lines().filter(|l| !l.is_empty()) {
        if let Ok(msg) = serde_json::from_str::<ChatMessage>(line)
            && msg.id < before_id
        {
            result.push(msg);
            if result.len() >= count {
                break;
            }
        }
    }
    result
}

/// Truncate messages, keeping only those with `id <= keep_up_to_id`.
///
/// Crash-safe: reads the file, filters lines, writes to a temp file,
/// then atomically renames into place.
pub fn truncate_messages(dir: &Path, keep_up_to_id: u64) -> std::io::Result<()> {
    migrate_from_chunks(dir);
    let path = messages_path(dir);
    if !path.exists() {
        return Ok(());
    }

    let content = fsutil::read_to_string(&path)?;

    // Write kept lines to a temp file, then atomically rename.
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = dir.join(format!(".tmp_truncate_{}_{}.jsonl", pid, n));

    {
        use std::io::Write;
        let tmp_file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(fsutil::extended_path(&tmp_path))?;
        let mut writer = std::io::BufWriter::new(tmp_file);
        let mut kept_any = false;
        for line in content.lines().filter(|l| !l.is_empty()) {
            if let Ok(msg) = serde_json::from_str::<ChatMessage>(line)
                && msg.id <= keep_up_to_id
            {
                writeln!(writer, "{}", line)?;
                kept_any = true;
            }
        }
        writer.flush()?;
        if !kept_any {
            // No messages to keep — delete the file and the temp.
            drop(writer);
            let _ = fsutil::remove_file(&tmp_path);
            return fsutil::remove_file(&path);
        }
    }

    fsutil::rename(&tmp_path, &path)?;
    Ok(())
}

/// Remove messages with the given IDs from the JSONL file.
///
/// Crash-safe: reads the file, filters out unwanted IDs, writes the
/// remaining lines to a temp file, then atomically renames into place.
/// Returns the number of messages actually removed.
pub fn remove_messages_by_id(
    dir: &Path,
    ids_to_remove: &std::collections::HashSet<u64>,
) -> std::io::Result<usize> {
    if ids_to_remove.is_empty() {
        return Ok(0);
    }
    migrate_from_chunks(dir);

    let path = messages_path(dir);
    if !path.exists() {
        return Ok(0);
    }

    let content = match fsutil::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(0),
    };

    let mut kept_lines: Vec<&str> = Vec::new();
    let mut removed_count = 0;

    for line in content.lines().filter(|l| !l.is_empty()) {
        if let Ok(msg) = serde_json::from_str::<ChatMessage>(line)
            && ids_to_remove.contains(&msg.id)
        {
            removed_count += 1;
            continue;
        }
        kept_lines.push(line);
    }

    if removed_count == 0 {
        return Ok(0);
    }

    // Write filtered lines to a temp file, then atomically rename.
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = dir.join(format!(".tmp_remove_{}_{}.jsonl", pid, n));

    {
        use std::io::Write;
        let tmp_file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(fsutil::extended_path(&tmp_path))?;
        let mut writer = std::io::BufWriter::new(tmp_file);
        for line in &kept_lines {
            writeln!(writer, "{}", line)?;
        }
        writer.flush()?;
    }

    if kept_lines.is_empty() {
        let _ = fsutil::remove_file(&tmp_path);
        fsutil::remove_file(&path)?;
    } else {
        fsutil::rename(&tmp_path, &path)?;
    }

    Ok(removed_count)
}
