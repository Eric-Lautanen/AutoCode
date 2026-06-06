// helpers.rs -- FS-crate helpers: file extraction from AI output, glob matching utilities.

use autocode_core::fsutil;
use autocode_core::helpers as core_helpers;

// -- File extraction from AI output --------------------------------------------

/// Known language tags that should NOT be treated as filenames.
const KNOWN_LANG_TAGS: &[&str] = &[
    "rust", "rs", "toml", "json", "yaml", "yml", "xml", "html", "css", "js",
    "ts", "tsx", "jsx", "py", "python", "sh", "bash", "zsh", "shell", "sql",
    "go", "java", "c", "cpp", "h", "hpp", "cs", "rb", "php", "swift", "kt",
    "scala", "lua", "r", "perl", "dart", "dockerfile", "makefile", "diff",
    "plaintext", "text", "markdown", "md", "ini", "cfg", "conf", "env", "nix",
    "haskell", "hs", "elixir", "ex", "erlang", "clj", "clojure", "vim", "fish",
    "powershell", "ps1", "bat", "cmd", "psm1",
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

// -- Glob matching utilities ---------------------------------------------------

/// Minimal glob matcher supporting `*`, `**`, and `?`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<&str> = pattern.split("**").collect();
    if p.len() == 1 {
        // No `**` -- simple single-segment match.
        return glob_match_segment(pattern, text);
    }
    // With `**`: the part before must match the start, part after must match
    // the end, allowing anything in between.
    // Strip any leading/trailing `/` from prefix/suffix so that `**/target`
    // correctly matches both `"target"` (root-level) and `"foo/target"` (nested).
    if let (Some(prefix), Some(suffix)) = (p.first(), p.last()) {
        let prefix = prefix.trim_end_matches('/');
        let suffix = suffix.trim_start_matches('/');
        if !prefix.is_empty() && !text.starts_with(prefix) {
            return false;
        }
        if !suffix.is_empty() {
            // Try literal path-component match first (e.g. `**/foo/bar`).
            if text == suffix || text.ends_with(&format!("/{}", suffix)) {
                return true;
            }
            // If suffix has wildcards, match from the end of the text.
            // e.g. `**/*.rs` should match `src/main.rs`.
            let suffix_segs: Vec<&str> = suffix.split('/').collect();
            let text_segs: Vec<&str> = text.split('/').collect();
            if text_segs.len() >= suffix_segs.len() {
                let offset = text_segs.len() - suffix_segs.len();
                let all_match = suffix_segs
                    .iter()
                    .enumerate()
                    .all(|(i, seg)| glob_match_segment(seg, text_segs[offset + i]));
                if all_match {
                    return true;
                }
            }
            return false;
        }
        return true;
    }
    false
}

/// Single-segment glob: supports `*` (any chars except `/`) and `?` (one char).
pub fn glob_match_segment(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX;
    let mut star_ti = 0;
    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
}
