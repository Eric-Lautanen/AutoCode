use crate::fsutil;
use crate::state::ChatMessage;
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
/// Rewrites all chunk files, removing excess messages.
///
/// Crash-safe: new chunks are written to a temp subdirectory first, old
/// files are deleted only after the new data is safely on disk, then temp
/// files are renamed into place.
pub fn truncate_messages_chunked(dir: &Path, keep_up_to_id: u64) -> std::io::Result<()> {
    let all = read_all_messages_chunked(dir);
    let keep: Vec<ChatMessage> = all.into_iter().filter(|m| m.id <= keep_up_to_id).collect();

    // Write new chunks to a temp subdirectory first so that a crash during
    // writing does not destroy the original data.
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_dir = dir.join(format!(".tmp_truncate_{}_{}", pid, n));
    fsutil::create_dir_all(&tmp_dir)?;
    write_messages_chunked(&tmp_dir, &keep)?;

    // Old files can now be safely removed - the new data is on disk.
    for path in find_chunk_files(dir) {
        if let Err(e) = fsutil::remove_file(&path) {
            eprintln!(
                "[chunked_jsonl] Failed to remove old chunk {:?}: {}",
                path, e
            );
        }
    }

    // Rename temp files into place so find_chunk_files can see them.
    for tmp_path in find_chunk_files(&tmp_dir) {
        let name = tmp_path.file_name().unwrap();
        let dest = dir.join(name);
        fsutil::rename(&tmp_path, &dest)?;
    }

    // Clean up the temp subdirectory.
    if let Err(e) = fsutil::remove_dir(&tmp_dir) {
        eprintln!(
            "[chunked_jsonl] Failed to remove temp dir {:?}: {}",
            tmp_dir, e
        );
    }
    Ok(())
}

fn write_messages_to_chunk(dir: &Path, chunk_idx: usize, lines: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    if lines.is_empty() {
        return Ok(());
    }
    let path = chunk_path(dir, chunk_idx);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(fsutil::extended_path(&path))?;
    for line in lines {
        writeln!(file, "{}", line)?;
    }
    file.flush()?;
    Ok(())
}

fn write_messages_chunked(dir: &Path, messages: &[ChatMessage]) -> std::io::Result<()> {
    if messages.is_empty() {
        return Ok(());
    }
    let mut chunk_idx = 0;
    let mut chunk_lines: Vec<String> = Vec::with_capacity(MESSAGES_PER_CHUNK);

    for msg in messages {
        let line = serde_json::to_string(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        chunk_lines.push(line);

        if chunk_lines.len() >= MESSAGES_PER_CHUNK {
            write_messages_to_chunk(dir, chunk_idx, &chunk_lines)?;
            chunk_lines.clear();
            chunk_idx += 1;
        }
    }

    if !chunk_lines.is_empty() {
        write_messages_to_chunk(dir, chunk_idx, &chunk_lines)?;
    }

    Ok(())
}
