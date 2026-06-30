// comment.rs -- Language-aware line classifier for fuzzy word extraction.
//
// Determines whether a line of source code is "code" (identifiers, logic) or
// "not-code" (comments, doc-comments, string-literal content, blank).  Only
// code lines are tokenized for fuzzy suggestions, which eliminates noise from
// comments and documentation.

/// Classify a source line as code or not-code.
///
/// * `line`        — the raw line text (may include leading whitespace)
/// * `ext`         — file extension *without* the dot, e.g. `"rs"`, `"py"`
/// * `in_block_comment` — mutable state tracking whether we are inside a
///   multi-line block comment.  This is the **only**
///   cross-line state needed.
///
/// Returns `true` if the line is code (should be tokenized), `false` if it
/// is a comment, doc-comment, string-literal content, or blank.
///
/// For unknown extensions the function returns `true` for every line — the
/// safe default that avoids silently losing words.
pub fn is_code_line(line: &str, ext: &str, in_block_comment: &mut bool) -> bool {
    let trimmed = line.trim();

    // Blank lines are never code.
    if trimmed.is_empty() {
        return false;
    }

    // If we are inside a block comment, check for the closing delimiter.
    if *in_block_comment {
        if let Some(_pos) = find_block_close(trimmed, ext) {
            // The close delimiter is on this line.  Everything after it may
            // be code, but in practice closing delimiters sit at or near the
            // end of the line, so we treat the whole line as non-code.
            *in_block_comment = false;
        }
        return false;
    }

    // Check whether this line *opens* a block comment that doesn't close
    // on the same line.  If so, mark state and return non-code.
    if let Some(open_pos) = find_block_open(trimmed, ext) {
        // Check if the block also closes on this same line.
        let close_pos = find_block_close(trimmed, ext);
        if close_pos.is_none() || close_pos.unwrap() <= open_pos {
            *in_block_comment = true;
            return false;
        }
        // Block opens and closes on the same line — treat as non-code.
        return false;
    }

    // Line-level comment classification.
    let comment_prefix = line_comment_prefix(ext);
    match comment_prefix {
        Some(prefix) => {
            // Before treating the line as a comment, verify that the comment
            // marker isn't inside a string literal.  If there's an unclosed
            // string delimiter before the comment marker, the marker is
            // inside a string and the line is code.
            let prefix_pos = trimmed.find(prefix);
            match prefix_pos {
                Some(pos) if !has_unclosed_string_before(&trimmed[..pos], ext) => {
                    // The comment marker is real.  Check for doc-comment
                    // variants (e.g. `///`, `//!`, `/**`, `/*!`, `##`, `#!`).
                    // They are also non-code.
                    false
                }
                _ => true, // Comment marker is inside a string → code.
            }
        }
        None => {
            // No line-comment syntax for this extension → treat as code.
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Line-comment prefixes by extension
// ---------------------------------------------------------------------------

/// Return the line-comment prefix for the given extension, if known.
fn line_comment_prefix(ext: &str) -> Option<&'static str> {
    match ext {
        // C-family: //  (also covers doc-comments /// and //! which start with //)
        "rs" | "js" | "ts" | "jsx" | "tsx" | "go" | "java" | "c" | "h" | "cpp" | "hpp"
        | "swift" | "kt" | "scala" | "css" => Some("//"),

        // Hash languages
        "py" | "rb" | "yaml" | "yml" | "toml" | "bash" | "sh" | "env" | "dockerfile" => Some("#"),

        // Dash languages
        "sql" | "lua" => Some("--"),

        // Semicolon languages
        "clj" | "cljs" | "edn" => Some(";"),

        // TeX
        "tex" | "bib" => Some("%"),

        // HTML/XML — no line-comment prefix; only block comments.
        "html" | "xml" | "svg" => None,

        // Haskell — supports both `--` and `{- -}` block; line prefix is `--`.
        "hs" => Some("--"),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Block-comment delimiters by extension
// ---------------------------------------------------------------------------

/// If the line opens a block comment, return the byte offset of the opener.
fn find_block_open(line: &str, ext: &str) -> Option<usize> {
    match ext {
        // C-family block comments
        "rs" | "js" | "ts" | "jsx" | "tsx" | "go" | "java" | "c" | "h" | "cpp" | "hpp"
        | "swift" | "kt" | "scala" | "css" => line.find("/*"),

        // HTML/XML
        "html" | "xml" | "svg" => line.find("<!--"),

        // Haskell
        "hs" => line.find("{-"),

        // C-family also supports `/**` doc blocks but `/*` already matches.
        _ => None,
    }
}

/// If the line closes a block comment, return the byte offset *past* the
/// closing delimiter so the caller can decide whether code follows.
fn find_block_close(line: &str, ext: &str) -> Option<usize> {
    match ext {
        "rs" | "js" | "ts" | "jsx" | "tsx" | "go" | "java" | "c" | "h" | "cpp" | "hpp"
        | "swift" | "kt" | "scala" | "css" => line.find("*/").map(|p| p + 2),

        "html" | "xml" | "svg" => line.find("-->").map(|p| p + 3),

        "hs" => line.find("-}").map(|p| p + 2),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// String-literal boundary check
// ---------------------------------------------------------------------------

/// Check whether there is an unclosed string delimiter in `text` (the portion
/// of the line *before* the comment marker).  If so, the comment marker is
/// likely inside a string literal and should not be treated as a comment.
///
/// Handles `"`, `'`, and `` ` `` delimiters.  A backslash before a delimiter
/// escapes it.  The check is per-line and conservative: if in doubt, returns
/// `false` (no unclosed string), which means the comment marker is accepted.
fn has_unclosed_string_before(text: &str, ext: &str) -> bool {
    let mut in_double = false;
    let mut in_single = false;
    let mut in_backtick = false;
    let mut escape = false;

    for ch in text.chars() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        match ch {
            '"' if !in_single && !in_backtick => in_double = !in_double,
            '\'' if !in_double && !in_backtick => in_single = !in_single,
            '`' if !in_double && !in_single && backtick_significant(ext) => {
                in_backtick = !in_backtick;
            }
            _ => {}
        }
    }

    in_double || in_single || in_backtick
}

/// Whether backtick delimiters are meaningful in the given language.
fn backtick_significant(ext: &str) -> bool {
    matches!(ext, "js" | "ts" | "jsx" | "tsx" | "go")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_line_comment() {
        let mut bc = false;
        assert!(!is_code_line("// hello", "rs", &mut bc));
        assert!(!bc);
    }

    #[test]
    fn rust_doc_comment() {
        let mut bc = false;
        assert!(!is_code_line("/// doc", "rs", &mut bc));
        assert!(!is_code_line("//! doc", "rs", &mut bc));
    }

    #[test]
    fn rust_code_line() {
        let mut bc = false;
        assert!(is_code_line("let x = 1;", "rs", &mut bc));
    }

    #[test]
    fn rust_block_comment() {
        let mut bc = false;
        assert!(!is_code_line("/* start", "rs", &mut bc));
        assert!(bc);
        assert!(!is_code_line("middle", "rs", &mut bc));
        assert!(bc);
        assert!(!is_code_line("end */", "rs", &mut bc));
        assert!(!bc);
    }

    #[test]
    fn rust_block_same_line() {
        let mut bc = false;
        assert!(!is_code_line("/* inline */", "rs", &mut bc));
        assert!(!bc);
    }

    #[test]
    fn python_hash_comment() {
        let mut bc = false;
        assert!(!is_code_line("# comment", "py", &mut bc));
        assert!(is_code_line("x = 1", "py", &mut bc));
    }

    #[test]
    fn python_string_before_hash() {
        let mut bc = false;
        // The # is inside a string — this is code.
        assert!(is_code_line(
            "url = \"http://foo.com # fragment\"",
            "py",
            &mut bc
        ));
    }

    #[test]
    fn sql_dash_comment() {
        let mut bc = false;
        assert!(!is_code_line("-- select", "sql", &mut bc));
        assert!(is_code_line("SELECT 1", "sql", &mut bc));
    }

    #[test]
    fn html_block_comment() {
        let mut bc = false;
        assert!(!is_code_line("<!-- comment", "html", &mut bc));
        assert!(bc);
        assert!(!is_code_line("content", "html", &mut bc));
        assert!(!is_code_line("-->", "html", &mut bc));
        assert!(!bc);
    }

    #[test]
    fn unknown_ext_all_code() {
        let mut bc = false;
        assert!(is_code_line("anything", "xyz", &mut bc));
    }

    #[test]
    fn blank_line() {
        let mut bc = false;
        assert!(!is_code_line("", "rs", &mut bc));
        assert!(!is_code_line("   ", "rs", &mut bc));
    }

    #[test]
    fn rust_string_with_slash() {
        let mut bc = false;
        // The // is inside a string — this is code.
        assert!(is_code_line(
            "let s = \"http://example.com\";",
            "rs",
            &mut bc
        ));
    }

    #[test]
    fn clojure_semicolon_comment() {
        let mut bc = false;
        assert!(!is_code_line("; comment", "clj", &mut bc));
        assert!(is_code_line("(defn foo [])", "clj", &mut bc));
    }

    #[test]
    fn tex_percent_comment() {
        let mut bc = false;
        assert!(!is_code_line("% comment", "tex", &mut bc));
        assert!(is_code_line("\\section{Intro}", "tex", &mut bc));
    }

    #[test]
    fn haskell_block_comment() {
        let mut bc = false;
        assert!(!is_code_line("{- start", "hs", &mut bc));
        assert!(bc);
        assert!(!is_code_line("middle", "hs", &mut bc));
        assert!(!is_code_line("-}", "hs", &mut bc));
        assert!(!bc);
    }
}
