// extract.rs -- File extraction from AI code-fence output.

use autocode_core::helpers as core_helpers;
use autocode_core::utils::fsutil;

/// Known language tags that should NOT be treated as filenames.
const KNOWN_LANG_TAGS: &[&str] = &[
    "rust",
    "rs",
    "toml",
    "json",
    "yaml",
    "yml",
    "xml",
    "html",
    "css",
    "js",
    "ts",
    "tsx",
    "jsx",
    "py",
    "python",
    "sh",
    "bash",
    "zsh",
    "shell",
    "sql",
    "go",
    "java",
    "c",
    "cpp",
    "h",
    "hpp",
    "cs",
    "rb",
    "php",
    "swift",
    "kt",
    "scala",
    "lua",
    "r",
    "perl",
    "dart",
    "dockerfile",
    "makefile",
    "diff",
    "plaintext",
    "text",
    "markdown",
    "md",
    "ini",
    "cfg",
    "conf",
    "env",
    "nix",
    "haskell",
    "hs",
    "elixir",
    "ex",
    "erlang",
    "clj",
    "clojure",
    "vim",
    "fish",
    "powershell",
    "ps1",
    "bat",
    "cmd",
    "psm1",
];

/// Parse AI output for files to create. Returns (filename, content) pairs.
pub fn extract_files(text: &str) -> Vec<(String, String)> {
    // Looks for: ```filename.ext ... ```
    // or markers like:
    // File: path/to/file.rs
    let mut files = Vec::new();
    let mut in_block = false;
    let mut filename = String::new();
    let mut content = String::new();

    for line in text.lines() {
        if !in_block {
            let trimmed = line.trim();
            if trimmed.starts_with("```") && trimmed.len() > 3 {
                let lang_or_file = trimmed.trim_start_matches('`').trim();
                // Skip known language tags (e.g. ```rust, ```python) so they
                // are not misidentified as filenames.
                if KNOWN_LANG_TAGS.contains(&lang_or_file) {
                    continue;
                }
                // If it contains a '.' it's likely a filename.
                if lang_or_file.contains('.') && !lang_or_file.contains(' ') {
                    in_block = true;
                    filename = lang_or_file.to_string();
                    content = String::new();
                }
            }
        } else if line.trim() == "```" {
            if !filename.is_empty() && !content.trim().is_empty() {
                files.push((filename.clone(), content.clone()));
            }
            in_block = false;
            filename = String::new();
            content = String::new();
        } else {
            content.push_str(line);
            content.push('\n');
        }
    }

    files
}

/// Write files extracted from AI output into the project root.
pub fn write_extracted_files(
    root: &str,
    files: &[(String, String)],
    allow_escape: bool,
) -> Vec<String> {
    let root_path = std::path::Path::new(root);
    let mut written = Vec::new();
    for (name, content) in files {
        let target = root_path.join(name);
        let resolved = core_helpers::resolve_path_write(name, root, allow_escape);
        if core_helpers::is_blocked_path(&resolved) {
            written.push(format!("{} (BLOCKED: path traversal)", name));
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = fsutil::create_dir_all(parent);
        }
        match fsutil::write(&target, content) {
            Ok(_) => written.push(name.clone()),
            Err(e) => written.push(format!("{} (ERROR: {})", name, e)),
        }
    }
    written
}
