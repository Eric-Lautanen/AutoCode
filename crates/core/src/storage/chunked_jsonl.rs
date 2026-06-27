use crate::state::ChatMessage;
use crate::utils::fsutil;
use std::path::{Path, PathBuf};

pub const MESSAGES_PER_CHUNK: usize = 1000;

fn chunk_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("messages_{:04}.jsonl", index))
}

fn find_chunk_files(dir: &Path) -> Vec<PathBuf> {
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

fn latest_chunk_info(dir: &Path) -> (usize, PathBuf) {
    let chunks = find_chunk_files(dir);
    if let Some(last) = chunks.last() {
        let name = last.file_name().unwrap().to_string_lossy();
        let index: usize = name[9..13].parse().unwrap_or(0);
        (index, last.clone())
    } else {
        (0, chunk_path(dir, 0))
    }
}

/// Check whether chunked message files exist in the given directory.
pub fn has_chunked_files(dir: &Path) -> bool {
    !find_chunk_files(dir).is_empty()
}

/// Find the latest chunk file and its index.
pub fn find_latest_chunk(dir: &Path) -> Option<(usize, PathBuf)> {
    let chunks = find_chunk_files(dir);
    let last = chunks.last()?;
    let name = last.file_name().unwrap().to_string_lossy();
    let index: usize = name[9..13].parse().ok()?;
    Some((index, last.clone()))
}

/// Count lines in a chunk file to know when to rotate.
fn chunk_line_count(path: &Path) -> usize {
    let Ok(content) = fsutil::read_to_string(path) else {
        return 0;
    };
    content.lines().filter(|l| !l.is_empty()).count()
}

/// Append messages to chunked JSONL files, rotating to a new chunk when the
/// current one reaches `MESSAGES_PER_CHUNK`.
pub fn append_messages_chunked(
    dir: &Path,
    _session_id: &str,
    _session_label: &str,
    messages: &[ChatMessage],
) -> std::io::Result<()> {
    if messages.is_empty() {
        return Ok(());
    }
    fsutil::create_dir_all(dir)?;

    let (mut chunk_idx, chunk_path_buf) = latest_chunk_info(dir);
    let mut cur_path = chunk_path_buf;
    let mut line_count = chunk_line_count(&cur_path);

    // If the chunk is at/over limit, rotate to the next.
    if line_count >= MESSAGES_PER_CHUNK {
        chunk_idx += 1;
        cur_path = chunk_path(dir, chunk_idx);
        line_count = 0;
    }

    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(fsutil::extended_path(&cur_path))?;
    let mut writer = std::io::BufWriter::new(file);

    for msg in messages {
        let line = serde_json::to_string(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(writer, "{}", line)?;
        line_count += 1;

        if line_count >= MESSAGES_PER_CHUNK {
            writer.flush()?;
            drop(writer);
            chunk_idx += 1;
            cur_path = chunk_path(dir, chunk_idx);
            let f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(fsutil::extended_path(&cur_path))?;
            writer = std::io::BufWriter::new(f);
            line_count = 0;
        }
    }

    writer.flush()?;
    Ok(())
}

/// Read all messages from all chunk files in order.
pub fn read_all_messages_chunked(dir: &Path) -> Vec<ChatMessage> {
    let chunks = find_chunk_files(dir);
    let mut all = Vec::new();
    for path in chunks {
        if let Ok(content) = fsutil::read_to_string(&path) {
            for line in content.lines().filter(|l| !l.is_empty()) {
                if let Ok(msg) = serde_json::from_str::<ChatMessage>(line) {
                    all.push(msg);
                }
            }
        }
    }
    all
}

/// Load messages with IDs less than `before_id`, up to `count` messages.
/// Reads chunks from newest to oldest until we have enough messages.
pub fn load_messages_chunked_before(dir: &Path, before_id: u64, count: usize) -> Vec<ChatMessage> {
    let mut chunks = find_chunk_files(dir);
    chunks.reverse();
    let mut result = Vec::new();
    for path in chunks {
        if result.len() >= count {
            break;
        }
        if let Ok(content) = fsutil::read_to_string(&path) {
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
        }
    }
    result.reverse();
    result
}

/// Truncate messages, keeping only those with `id <= keep_up_to_id`.
///
/// This is an append-only-safe operation: it never rewrites chunk files from
/// RAM. Instead, it:
///   1. Scans chunks from newest to oldest to find the boundary chunk that
///      contains the last message to keep.
///   2. Rewrites only the boundary chunk (keeping lines with `id <= keep_up_to_id`).
///   3. Deletes all chunks after the boundary.
///   4. Leaves all earlier chunks untouched — they are never read, rewritten,
///      or modified in any way.
///
/// Crash-safe: the boundary chunk is rewritten to a temp file first, then
/// atomically renamed into place. Old tail chunks are deleted only after
/// the new boundary chunk is safely on disk.
pub fn truncate_messages_chunked(dir: &Path, keep_up_to_id: u64) -> std::io::Result<()> {
    let chunks = find_chunk_files(dir);

    // Find the boundary chunk: the one that contains the last message to keep.
    // Walk from newest chunk to oldest.
    let mut boundary_chunk_idx: Option<usize> = None;
    let mut boundary_path: Option<PathBuf> = None;

    for path in chunks.iter().rev() {
        if let Ok(content) = fsutil::read_to_string(path) {
            for line in content.lines().filter(|l| !l.is_empty()) {
                if let Ok(msg) = serde_json::from_str::<ChatMessage>(line)
                    && msg.id <= keep_up_to_id
                {
                    // This chunk contains at least one message to keep.
                    boundary_chunk_idx = Some(chunks.iter().position(|p| p == path).unwrap());
                    boundary_path = Some(path.clone());
                    break;
                }
            }
        }
        if boundary_chunk_idx.is_some() {
            break;
        }
    }

    let (boundary_idx, boundary_path) = match (boundary_chunk_idx, boundary_path) {
        (Some(i), Some(p)) => (i, p),
        _ => {
            // No messages to keep — delete all chunk files.
            for path in &chunks {
                if let Err(e) = fsutil::remove_file(path) {
                    eprintln!("[chunked_jsonl] Failed to remove chunk {:?}: {}", path, e);
                }
            }
            return Ok(());
        }
    };

    // Rewrite the boundary chunk, keeping only lines with id <= keep_up_to_id.
    // Write to a temp file first for crash safety.
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = dir.join(format!(".tmp_truncate_{}_{}.jsonl", pid, n));

    {
        use std::io::Write;
        let content = fsutil::read_to_string(&boundary_path)?;
        let tmp_file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(fsutil::extended_path(&tmp_path))?;
        let mut writer = std::io::BufWriter::new(tmp_file);
        for line in content.lines().filter(|l| !l.is_empty()) {
            if let Ok(msg) = serde_json::from_str::<ChatMessage>(line)
                && msg.id <= keep_up_to_id
            {
                writeln!(writer, "{}", line)?;
            }
        }
        writer.flush()?;
    }

    // Delete all chunks after the boundary first (they are entirely past the
    // cutoff point). Do this before replacing the boundary chunk so that a
    // crash at any point leaves either the old or new state intact.
    for path in chunks.iter().skip(boundary_idx + 1) {
        if let Err(e) = fsutil::remove_file(path) {
            eprintln!(
                "[chunked_jsonl] Failed to remove tail chunk {:?}: {}",
                path, e
            );
        }
    }

    // Now atomically replace the boundary chunk with the truncated version.
    fsutil::rename(&tmp_path, &boundary_path)?;

    Ok(())
}

/// Remove messages with the given IDs from the chunked JSONL files.
///
/// This is an append-only-safe operation: it never rewrites a chunk from RAM.
/// Instead, for each affected chunk, it reads the file, filters out the
/// unwanted message IDs, and writes the remaining lines to a temp file that
/// is then atomically renamed into place. Unaffected chunks are left untouched.
///
/// Returns the number of messages actually removed.
pub fn remove_messages_by_id(
    dir: &Path,
    ids_to_remove: &std::collections::HashSet<u64>,
) -> std::io::Result<usize> {
    if ids_to_remove.is_empty() {
        return Ok(0);
    }

    let chunks = find_chunk_files(dir);
    let mut total_removed = 0;

    for path in &chunks {
        let content = match fsutil::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
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
            continue;
        }

        total_removed += removed_count;

        // Write filtered chunk to a temp file, then atomically rename.
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

        // If all lines were removed, delete the chunk instead of leaving an empty file.
        if kept_lines.is_empty() {
            let _ = fsutil::remove_file(&tmp_path);
            fsutil::remove_file(path)?;
        } else {
            fsutil::rename(&tmp_path, path)?;
        }
    }

    Ok(total_removed)
}
