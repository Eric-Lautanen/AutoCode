// storage/attachments.rs -- Staging and resolution of chat attachments (F3).
//
// Bytes are copied into the session directory at stage time; the JSONL only
// ever records metadata. Because staged copies live inside the session
// folder, deleting a session cleans them with zero extra code.

use std::path::PathBuf;

use crate::state::{Attachment, AttachmentKind, Project, Session};
use crate::utils::fsutil;

/// Per-image byte cap at stage time.
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
/// Total staged bytes per message.
pub const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
/// Text injected into the conversation is capped via truncate_middle.
pub const MAX_TEXT_INJECTION_BYTES: usize = 128 * 1024;

/// Coarse content class used by the injection matrix (D4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttClass {
    Image,
    Text,
    Binary,
}

/// Classify a file by extension. No magic-byte sniffing: the picker already
/// knows what it picked, and unknown extensions are treated as binary so
/// nothing unexpected is dumped into context.
pub fn classify(name: &str) -> AttClass {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "svg" => AttClass::Image,
        "txt" | "md" | "markdown" | "json" | "jsonl" | "toml" | "yaml" | "yml" | "xml" | "csv"
        | "tsv" | "log" | "ini" | "cfg" | "conf" | "env" | "rs" | "py" | "js" | "ts" | "jsx"
        | "tsx" | "c" | "h" | "cpp" | "hpp" | "cc" | "java" | "kt" | "swift" | "go" | "rb"
        | "php" | "cs" | "fs" | "hs" | "ml" | "lua" | "pl" | "sh" | "bash" | "zsh" | "fish"
        | "ps1" | "bat" | "cmd" | "sql" | "html" | "htm" | "css" | "scss" | "less" | "vue"
        | "svelte" | "dart" | "r" | "jl" | "ex" | "exs" | "erl" | "clj" | "scala" | "zig"
        | "nim" | "v" | "lean" | "tex" | "bib" | "srt" | "nfo" | "license" | "gitignore"
        | "dockerfile" | "makefile" | "lock" => AttClass::Text,
        _ => {
            // Common extension-less text files.
            let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
            match base.to_ascii_lowercase().as_str() {
                "license" | "readme" | "changelog" | "dockerfile" | "makefile" | ".gitignore"
                | ".env" | ".editorconfig" => AttClass::Text,
                _ => AttClass::Binary,
            }
        }
    }
}

fn sanitize_component(name: &str) -> String {
    crate::helpers::sanitize_filename(name)
}

/// Minimal standard base64 encoder (std-only; no new crates).
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Image mime type by extension (for data URLs). Defaults are fine: providers
/// sniff actual bytes when the mime is generic.
pub fn image_mime(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "tiff" | "tif" => "image/tiff",
        _ => "application/octet-stream",
    }
}

/// The session directory's attachments folder.
pub fn attachments_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("attachments")
}

/// Resolve a staged attachment to its on-disk copy.
pub fn resolve_path(project: &Project, session: &Session, att: &Attachment) -> PathBuf {
    crate::storage::session_messages_dir(project, session).join(&att.rel_path)
}

/// Copy `src` into the session's attachments/ directory under a unique name,
/// enforcing the per-file size cap. Returns the staged metadata.
/// `total_staged_bytes` is what has been staged for this message so far.
pub fn stage_file(
    project: &Project,
    session: &Session,
    src: &std::path::Path,
    kind: AttachmentKind,
    total_staged_bytes: u64,
) -> Result<Attachment, String> {
    let file_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "attachment".to_string());
    let meta = std::fs::metadata(fsutil::extended_path(src))
        .map_err(|e| format!("cannot read {}: {}", file_name, e))?;
    let bytes = meta.len();
    if kind == AttachmentKind::Image && bytes > MAX_IMAGE_BYTES {
        return Err(format!(
            "{} is {} MB -- images are capped at 8 MB",
            file_name,
            bytes / (1024 * 1024)
        ));
    }
    if total_staged_bytes + bytes > MAX_TOTAL_BYTES {
        return Err(format!(
            "attachments for this message exceed the 32 MB total cap ({}/{} MB staged)",
            total_staged_bytes / (1024 * 1024),
            MAX_TOTAL_BYTES / (1024 * 1024)
        ));
    }

    let dir = attachments_dir(&crate::storage::session_messages_dir(project, session));
    fsutil::create_dir_all(&dir).map_err(|e| format!("cannot create attachments dir: {}", e))?;

    let id = crate::helpers::generate_id();
    let safe = sanitize_component(&file_name);
    let stored_name = format!("{}_{}", id, safe);
    let dest = dir.join(&stored_name);
    std::fs::copy(fsutil::extended_path(src), fsutil::extended_path(&dest))
        .map_err(|e| format!("cannot copy {}: {}", file_name, e))?;

    Ok(Attachment {
        id,
        kind,
        name: file_name,
        mime: String::new(),
        bytes,
        rel_path: PathBuf::from("attachments")
            .join(&stored_name)
            .to_string_lossy()
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_by_extension() {
        assert_eq!(classify("photo.PNG"), AttClass::Image);
        assert_eq!(classify("notes.md"), AttClass::Text);
        assert_eq!(classify("main.rs"), AttClass::Text);
        assert_eq!(classify("archive.zip"), AttClass::Binary);
        assert_eq!(classify("Dockerfile"), AttClass::Text);
        assert_eq!(classify("data.bin"), AttClass::Binary);
    }

    #[test]
    fn base64_known_vectors() {
        // RFC 4648 test vectors.
        for (raw, encoded) in [
            (&b""[..], ""),
            (&b"f"[..], "Zg=="),
            (&b"fo"[..], "Zm8="),
            (&b"foo"[..], "Zm9v"),
            (&b"foob"[..], "Zm9vYg=="),
            (&b"fooba"[..], "Zm9vYmE="),
            (&b"foobar"[..], "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(raw), encoded);
        }
    }

    #[test]
    fn image_mime_mapping() {
        assert_eq!(image_mime("a.png"), "image/png");
        assert_eq!(image_mime("B.JPG"), "image/jpeg");
        assert_eq!(image_mime("x.weird"), "application/octet-stream");
    }
}
