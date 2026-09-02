// syntax.rs -- Tiny hand-rolled syntax highlighter for chat code views.
//
// Dependency-free on purpose: strings and numbers highlight in every
// language; line/block comments and keywords come from a small
// per-extension profile, and unknown languages get a neutral profile
// (strings + numbers, no comments/keywords) so logs and listings never
// mis-highlight. Markdown/prose (`md`, `txt`, `log`) gets no profile at
// all and renders plain.
//
// Documented simplifications: strings end at end-of-line (no multi-line
// strings); block comments don't nest; `'` opens a string only for real
// char literals (`'x'`, `'\n'`) unless the profile says single quotes are
// strings (Python, shell, SQL…), so Rust lifetimes and apostrophes stay
// plain. Callers tokenize one logical line at a time and thread
// `in_block_comment` across lines so `/* … */` spanning rows works.

/// Token kind for one highlighted span.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tok {
    Normal,
    Keyword,
    Str,
    Comment,
    Number,
    /// `foo(` — identifier immediately followed by `(` (or `!` for
    /// `vec!`-style macros, unless it is `!=`). Keywords win ties
    /// (`if (` stays a keyword).
    Function,
    /// `Foo`, `MAX_SIZE` — identifier starting with an uppercase letter.
    /// Checked after keywords (`Self`, `None` stay keywords).
    Type,
    /// `#[derive(…)]`, `#include`, `@decorator` — preprocessor and
    /// annotation markers.
    Annotation,
    /// `///` and `//!` doc comments — brighter than plain comments.
    Doc,
}

/// One highlighted span of a tokenized line.
#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub kind: Tok,
}

/// Highlighting rules for one language. All keyword tables are lowercase;
/// lookup lowercases the word first, so `True`, `Self` and `SELECT` match.
#[derive(Clone, Copy)]
pub struct Profile {
    pub line_comment: Option<&'static str>,
    pub block_comment: bool,
    /// `'…'` opens an arbitrary string (Python, shell, SQL, TOML…).
    /// Otherwise `'` is only a char literal (`'x'`, `'\n'`).
    pub single_quote_string: bool,
    pub keywords: &'static [&'static str],
}

const RUST_KW: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "none",
    "ok", "err", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true",
    "unsafe", "use", "where", "while",
];

const PY_KW: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "false", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "none", "nonlocal", "not", "or", "pass", "raise", "return", "true", "try", "while",
    "with", "yield",
];

const JS_KW: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "false",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const TS_KW: &[&str] = &[
    "abstract",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "declare",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "infer",
    "instanceof",
    "interface",
    "keyof",
    "let",
    "namespace",
    "new",
    "null",
    "of",
    "override",
    "private",
    "protected",
    "public",
    "readonly",
    "return",
    "satisfies",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "false",
    "try",
    "type",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const SH_KW: &[&str] = &[
    "case", "declare", "do", "done", "elif", "else", "esac", "exit", "export", "false", "fi",
    "for", "function", "if", "in", "local", "readonly", "return", "select", "test", "then", "true",
    "until", "while",
];

const C_KW: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "false", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "true",
    "typedef", "union", "unsigned", "void", "volatile", "while", "null",
];

const CPP_KW: &[&str] = &[
    "auto",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "constexpr",
    "continue",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "nullptr",
    "null",
    "operator",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "template",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

const JAVA_KW: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "void",
    "volatile",
    "while",
];

const GO_KW: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "nil",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
    "true",
    "false",
];

const SQL_KW: &[&str] = &[
    "select",
    "from",
    "where",
    "insert",
    "into",
    "values",
    "update",
    "set",
    "delete",
    "create",
    "table",
    "alter",
    "drop",
    "join",
    "left",
    "right",
    "inner",
    "outer",
    "on",
    "group",
    "by",
    "order",
    "having",
    "limit",
    "offset",
    "distinct",
    "as",
    "and",
    "or",
    "not",
    "null",
    "true",
    "false",
    "primary",
    "key",
    "index",
    "view",
    "database",
    "union",
    "all",
    "exists",
    "in",
    "is",
    "like",
    "between",
    "case",
    "when",
    "then",
    "else",
    "end",
    "cast",
    "constraint",
    "foreign",
    "references",
    "default",
    "unique",
    "check",
];

const LUA_KW: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

const JSON_KW: &[&str] = &["true", "false", "null"];
const TOML_KW: &[&str] = &["true", "false"];
const YAML_KW: &[&str] = &["true", "false", "null"];
const EMPTY_KW: &[&str] = &[];

/// Neutral profile: strings + numbers only. Used for `diff` fences and
/// languages with no comment/keyword rules so nothing mis-highlights.
const NEUTRAL: Profile = Profile {
    line_comment: None,
    block_comment: false,
    single_quote_string: false,
    keywords: EMPTY_KW,
};

/// Look up a profile by filename, extension, or bare language name
/// (`main.rs`, `rs`, `rust` all work; case-insensitive). Returns `None`
/// for prose and unknown types — those render plain.
pub fn profile_for(lang: &str) -> Option<Profile> {
    let l = lang.trim().to_lowercase();
    // Filenames (`main.rs`, `Dockerfile`) and bare names (`rust`) alike:
    // a dotted name resolves by extension, anything else by itself.
    let key = l.rsplit('.').next().unwrap_or(&l);
    let p = match key {
        "rs" | "rust" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: false,
            keywords: RUST_KW,
        },
        "py" | "python" | "pyw" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: true,
            keywords: PY_KW,
        },
        "js" | "javascript" | "jsx" | "mjs" | "cjs" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: true,
            keywords: JS_KW,
        },
        "ts" | "typescript" | "tsx" | "mts" | "cts" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: true,
            keywords: TS_KW,
        },
        "sh" | "bash" | "zsh" | "fish" | "dash" | "shell" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: true,
            keywords: SH_KW,
        },
        "c" | "h" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: false,
            keywords: C_KW,
        },
        "cpp" | "hpp" | "hxx" | "cc" | "cxx" | "c++" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: false,
            keywords: CPP_KW,
        },
        "java" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: false,
            keywords: JAVA_KW,
        },
        "go" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: false,
            keywords: GO_KW,
        },
        "cs" => Profile {
            line_comment: Some("//"),
            block_comment: true,
            single_quote_string: false,
            keywords: CPP_KW,
        },
        "lua" => Profile {
            line_comment: Some("--"),
            block_comment: false,
            single_quote_string: true,
            keywords: LUA_KW,
        },
        "sql" => Profile {
            line_comment: Some("--"),
            block_comment: true,
            single_quote_string: true,
            keywords: SQL_KW,
        },
        "json" => Profile {
            line_comment: None,
            block_comment: false,
            single_quote_string: false,
            keywords: JSON_KW,
        },
        "toml" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: true,
            keywords: TOML_KW,
        },
        "yaml" | "yml" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: true,
            keywords: YAML_KW,
        },
        "css" => Profile {
            line_comment: None,
            block_comment: true,
            single_quote_string: true,
            keywords: EMPTY_KW,
        },
        "html" | "xml" | "svg" => Profile {
            line_comment: None,
            block_comment: false,
            single_quote_string: true,
            keywords: EMPTY_KW,
        },
        "dockerfile" | "containerfile" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: true,
            keywords: EMPTY_KW,
        },
        "makefile" | "mk" | "mak" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: false,
            keywords: EMPTY_KW,
        },
        "ini" | "cfg" | "conf" => Profile {
            line_comment: Some("#"),
            block_comment: false,
            single_quote_string: true,
            keywords: EMPTY_KW,
        },
        "diff" | "patch" => NEUTRAL,
        _ => return None,
    };
    Some(p)
}

/// Consume a `#…` marker (`#[derive(…)]`, `#include`, `#!…`) or `@…`
/// annotation, returning its byte length. Plain `#`/`@` with nothing
/// marker-like after them are not markers.
fn marker_len(rest: &str) -> Option<usize> {
    let mut it = rest.chars();
    match it.next() {
        Some('#') => {
            if let Some(inner) = rest.strip_prefix("#[") {
                // Bracketed attribute: consume to the first `]`.
                let mut len = 2;
                for c in inner.chars() {
                    len += c.len_utf8();
                    if c == ']' {
                        return Some(len);
                    }
                }
                None
            } else {
                // `#`/`#!` plus a word (`include`, `pragma`, `!…`).
                let mut len = 1;
                let mut word = 0;
                for c in rest[1..].chars() {
                    if c == '!' && word == 0 {
                        len += 1;
                    } else if c.is_alphanumeric() || c == '_' {
                        len += c.len_utf8();
                        word += 1;
                    } else {
                        break;
                    }
                }
                (word > 0).then_some(len)
            }
        }
        Some('@') => {
            let mut len = 1;
            for c in rest[1..].chars() {
                if c.is_alphanumeric() || c == '_' {
                    len += c.len_utf8();
                } else {
                    break;
                }
            }
            (len > 1).then_some(len)
        }
        _ => None,
    }
}

/// `'` opens a string only for genuine char literals (`'x'`, `'\n'`).
/// Everything else — Rust lifetimes, shell/Python prose apostrophes —
/// stays plain text (or is handled by `single_quote_string` profiles).
fn is_char_literal(rest: &str) -> bool {
    let mut it = rest.chars();
    if it.next() != Some('\'') {
        return false;
    }
    match it.next() {
        None => false,
        Some('\\') => it.next().is_some() && it.next() == Some('\''),
        Some(_) => it.next() == Some('\''),
    }
}

fn push_span(spans: &mut Vec<Span>, text: String, kind: Tok) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.kind == kind
    {
        last.text.push_str(&text);
    } else {
        spans.push(Span { text, kind });
    }
}

/// Tokenize one logical line. Thread `in_block_comment` across lines in
/// order so `/* … */` spanning rows highlights correctly; every other
/// construct ends at end-of-line.
pub fn highlight_line(line: &str, profile: &Profile, in_block_comment: &mut bool) -> Vec<Span> {
    let mut spans = Vec::new();
    let n = line.len();
    let mut i = 0;
    while i < n {
        let rest = &line[i..];
        if *in_block_comment {
            if let Some(end) = rest.find("*/") {
                push_span(&mut spans, rest[..end + 2].to_string(), Tok::Comment);
                i += end + 2;
                *in_block_comment = false;
            } else {
                push_span(&mut spans, rest.to_string(), Tok::Comment);
                break;
            }
            continue;
        }
        let c = rest.chars().next().unwrap_or('\0');
        if let Some(lc) = profile.line_comment
            && rest.starts_with(lc)
        {
            // `///` / `//!` (but not `////`) are doc comments.
            let kind = if lc == "//"
                && (rest.starts_with("///") && !rest.starts_with("////") || rest.starts_with("//!"))
            {
                Tok::Doc
            } else {
                Tok::Comment
            };
            push_span(&mut spans, rest.to_string(), kind);
            break;
        }
        if profile.block_comment && rest.starts_with("/*") {
            if let Some(end) = rest[2..].find("*/") {
                push_span(&mut spans, rest[..2 + end + 2].to_string(), Tok::Comment);
                i += 2 + end + 2;
            } else {
                push_span(&mut spans, rest.to_string(), Tok::Comment);
                *in_block_comment = true;
                break;
            }
            continue;
        }
        // `#…` / `@…` markers, except where `#` starts a line comment
        // (handled above).
        if (c == '#' || c == '@')
            && let Some(len) = marker_len(rest)
        {
            push_span(&mut spans, rest[..len].to_string(), Tok::Annotation);
            i += len;
            continue;
        }
        if c == '"'
            || c == '`'
            || (c == '\'' && (profile.single_quote_string || is_char_literal(rest)))
        {
            let quote = c;
            let mut j = i + c.len_utf8();
            while j < n {
                let d = line[j..].chars().next().unwrap_or('\0');
                if d == '\\' {
                    j += d.len_utf8();
                    if j < n {
                        j += line[j..].chars().next().map(|e| e.len_utf8()).unwrap_or(0);
                    }
                    continue;
                }
                j += d.len_utf8();
                if d == quote {
                    break;
                }
            }
            push_span(&mut spans, line[i..j.min(n)].to_string(), Tok::Str);
            i = j.min(n);
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < n {
                let d = line[j..].chars().next().unwrap_or('\0');
                if d.is_ascii_alphanumeric() || d == '_' || d == '.' || d == '\'' {
                    j += d.len_utf8();
                } else {
                    break;
                }
            }
            push_span(&mut spans, line[i..j].to_string(), Tok::Number);
            i = j;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < n {
                let d = line[j..].chars().next().unwrap_or('\0');
                if d.is_alphanumeric() || d == '_' {
                    j += d.len_utf8();
                } else {
                    break;
                }
            }
            let word = &line[i..j];
            let after = &line[j..];
            let kind = if profile
                .keywords
                .contains(&word.to_ascii_lowercase().as_str())
            {
                Tok::Keyword
            } else if after.starts_with('(') || (after.starts_with('!') && !after.starts_with("!="))
            {
                Tok::Function
            } else if word.chars().next().is_some_and(|f| f.is_uppercase()) {
                Tok::Type
            } else {
                Tok::Normal
            };
            push_span(&mut spans, word.to_string(), kind);
            i = j;
            continue;
        }
        push_span(&mut spans, c.to_string(), Tok::Normal);
        i += c.len_utf8();
    }
    spans
}

/// One word-diff fragment of a paired diff line.
#[derive(Clone, Debug)]
pub struct WordPart {
    pub text: String,
    /// True when this fragment differs (gets the strong highlight).
    pub changed: bool,
}

fn chars_as_str(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((i, _)) = it.next() {
        let j = it.peek().map(|(k, _)| *k).unwrap_or(s.len());
        out.push(&s[i..j]);
    }
    out
}

fn fold_parts(dl: &[crate::helpers::DiffLine<'_>], want: char, parts: &mut Vec<WordPart>) {
    for d in dl {
        if d.prefix != want && d.prefix != ' ' {
            continue;
        }
        let changed = d.prefix == want;
        if let Some(last) = parts.last_mut()
            && last.changed == changed
        {
            last.text.push_str(d.text);
        } else {
            parts.push(WordPart {
                text: d.text.to_string(),
                changed,
            });
        }
    }
}

/// Char-level diff of two paired `-`/`+` lines for intra-line highlighting.
/// Returns the fragments for the old and new line. `None` when the pair is
/// too big for the O(n·m) LCS to be worth it, or when the lines are too
/// dissimilar — positional pairing on rewritten blocks matches unrelated
/// lines, and highlighting those fragments is noise, so callers fall back
/// to whole-line highlighting in both cases.
pub fn word_diff(old: &str, new: &str) -> Option<(Vec<WordPart>, Vec<WordPart>)> {
    const MAX_CHARS: usize = 1500;
    /// Minimum fraction of unchanged chars for the pair to count as "the
    /// same line, edited" rather than two unrelated lines.
    const MIN_SIMILARITY: f32 = 0.4;
    let o = chars_as_str(old);
    let n = chars_as_str(new);
    if o.len() + n.len() > MAX_CHARS || o.is_empty() || n.is_empty() {
        return None;
    }
    let dl = crate::helpers::lcs_diff_lines(&o, &n);
    let mut old_parts = Vec::new();
    let mut new_parts = Vec::new();
    fold_parts(&dl, '-', &mut old_parts);
    fold_parts(&dl, '+', &mut new_parts);
    let unchanged: usize = old_parts
        .iter()
        .filter(|p| !p.changed)
        .map(|p| p.text.chars().count())
        .sum();
    let total = o.len().max(n.len()).max(1);
    if unchanged as f32 / total as f32 >= MIN_SIMILARITY {
        Some((old_parts, new_parts))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str, profile: &Profile) -> Vec<(String, Tok)> {
        let mut block = false;
        highlight_line(line, profile, &mut block)
            .into_iter()
            .map(|s| (s.text, s.kind))
            .collect()
    }

    /// Kind of the span containing `word` as a whole identifier — adjacent
    /// punctuation merged into the span (e.g. `foo<`) must not match.
    fn kind_of(spans: &[(String, Tok)], word: &str) -> Option<Tok> {
        spans
            .iter()
            .find(|(s, _)| {
                s.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|w| w == word)
            })
            .map(|(_, k)| *k)
    }

    fn rust() -> Profile {
        profile_for("main.rs").unwrap()
    }

    #[test]
    fn rust_basics() {
        let spans = kinds(r#"fn main() { let x = 42; } // hi"#, &rust());
        assert_eq!(kind_of(&spans, "fn"), Some(Tok::Keyword));
        assert_eq!(kind_of(&spans, "let"), Some(Tok::Keyword));
        assert_eq!(kind_of(&spans, "42"), Some(Tok::Number));
        assert_eq!(kind_of(&spans, "x"), Some(Tok::Normal));
        assert!(
            spans
                .iter()
                .any(|(s, k)| s == "// hi" && *k == Tok::Comment)
        );
    }

    #[test]
    fn strings_with_escapes() {
        let spans = kinds(r#"let s = "a\"b";"#, &rust());
        assert!(
            spans
                .iter()
                .any(|(s, k)| s == "\"a\\\"b\"" && *k == Tok::Str)
        );
    }

    #[test]
    fn lifetimes_stay_plain() {
        let spans = kinds("fn foo<'a>(x: &'a str)", &rust());
        assert!(!spans.iter().any(|(_, k)| *k == Tok::Str));
        assert_eq!(kind_of(&spans, "foo"), Some(Tok::Normal));
    }

    #[test]
    fn char_literals_highlight() {
        let spans = kinds("let c = 'x';", &rust());
        assert!(spans.iter().any(|(s, k)| s == "'x'" && *k == Tok::Str));
    }

    #[test]
    fn shell_single_quotes_are_strings() {
        let p = profile_for("run.sh").unwrap();
        let spans = kinds("echo 'hello world' # done", &p);
        assert!(
            spans
                .iter()
                .any(|(s, k)| s == "'hello world'" && *k == Tok::Str)
        );
        assert!(
            spans
                .iter()
                .any(|(s, k)| s == "# done" && *k == Tok::Comment)
        );
        assert_eq!(kind_of(&spans, "echo"), Some(Tok::Normal));
    }

    #[test]
    fn python_keywords_case() {
        let p = profile_for("a.py").unwrap();
        let spans = kinds("def f(x): return True", &p);
        assert!(spans.iter().any(|(s, k)| s == "def" && *k == Tok::Keyword));
        assert!(spans.iter().any(|(s, k)| s == "True" && *k == Tok::Keyword));
    }

    #[test]
    fn block_comment_spans_lines() {
        let p = rust();
        let mut block = false;
        let a = highlight_line("/* open", &p, &mut block);
        assert!(block);
        assert!(a.iter().all(|s| s.kind == Tok::Comment));
        let b: Vec<(String, Tok)> = highlight_line("still comment */ let x = 1;", &p, &mut block)
            .into_iter()
            .map(|s| (s.text, s.kind))
            .collect();
        assert!(!block);
        assert!(b.iter().any(|(s, k)| s == "let" && *k == Tok::Keyword));
        assert!(b.iter().any(|(s, k)| s == "1" && *k == Tok::Number));
    }

    #[test]
    fn sql_case_insensitive() {
        let p = profile_for("q.sql").unwrap();
        let spans = kinds("SELECT a FROM t WHERE x = 1", &p);
        assert!(
            spans
                .iter()
                .any(|(s, k)| s == "SELECT" && *k == Tok::Keyword)
        );
        assert!(spans.iter().any(|(s, k)| s == "1" && *k == Tok::Number));
    }

    #[test]
    fn profiles_resolve() {
        assert!(profile_for("main.rs").is_some());
        assert!(profile_for("rs").is_some());
        assert!(profile_for("Rust").is_some());
        assert!(profile_for("Dockerfile").is_some());
        assert!(profile_for("notes.md").is_none());
        assert!(profile_for("output").is_none());
        assert!(profile_for("diff").is_some());
    }

    #[test]
    fn word_diff_marks_changed_span() {
        let (old, new) = word_diff("let foo_bar = 1;", "let foo_baz = 1;").unwrap();
        assert!(old.iter().any(|p| p.changed && p.text == "r"));
        assert!(new.iter().any(|p| p.changed && p.text == "z"));
        assert!(
            old.iter()
                .any(|p| !p.changed && p.text.contains("let foo_ba"))
        );
        // Reassembly round-trips both lines.
        assert_eq!(
            old.iter().map(|p| p.text.as_str()).collect::<String>(),
            "let foo_bar = 1;"
        );
        assert_eq!(
            new.iter().map(|p| p.text.as_str()).collect::<String>(),
            "let foo_baz = 1;"
        );
    }

    #[test]
    fn functions_and_types() {
        let spans = kinds("let x = foo(a, MAX); Bar::baz(qux);", &rust());
        assert_eq!(kind_of(&spans, "foo"), Some(Tok::Function));
        assert_eq!(kind_of(&spans, "baz"), Some(Tok::Function));
        assert_eq!(kind_of(&spans, "Bar"), Some(Tok::Type));
        assert_eq!(kind_of(&spans, "MAX"), Some(Tok::Type));
        assert_eq!(kind_of(&spans, "let"), Some(Tok::Keyword));
    }

    #[test]
    fn keyword_wins_over_function() {
        let spans = kinds("if (x) { while (y) {} }", &rust());
        assert_eq!(kind_of(&spans, "if"), Some(Tok::Keyword));
        assert_eq!(kind_of(&spans, "while"), Some(Tok::Keyword));
    }

    #[test]
    fn attributes_and_directives() {
        let spans = kinds("#[derive(Debug)]", &rust());
        assert!(
            spans
                .iter()
                .any(|(s, k)| s == "#[derive(Debug)]" && *k == Tok::Annotation)
        );
        let c = profile_for("a.c").unwrap();
        let spans = kinds("#include <stdio.h>", &c);
        assert_eq!(kind_of(&spans, "include"), Some(Tok::Annotation));
        let py = profile_for("a.py").unwrap();
        let spans = kinds("@app.route('/x')", &py);
        assert_eq!(kind_of(&spans, "app"), Some(Tok::Annotation));
        assert_eq!(kind_of(&spans, "route"), Some(Tok::Function));
    }

    #[test]
    fn macro_bang_is_function_not_equal() {
        let spans = kinds("let v = vec![1, 2]; assert!(ok);", &rust());
        assert_eq!(kind_of(&spans, "vec"), Some(Tok::Function));
        assert_eq!(kind_of(&spans, "assert"), Some(Tok::Function));
        let spans = kinds("if (a != b) { }", &rust());
        assert_eq!(kind_of(&spans, "a"), Some(Tok::Normal));
    }

    #[test]
    fn doc_comments() {
        let spans = kinds("/// Adds two numbers.", &rust());
        assert!(spans.iter().all(|(_, k)| *k == Tok::Doc));
        let spans = kinds("//! Module docs.", &rust());
        assert!(spans.iter().all(|(_, k)| *k == Tok::Doc));
        let spans = kinds("//// separator", &rust());
        assert!(spans.iter().all(|(_, k)| *k == Tok::Comment));
        let spans = kinds("// plain", &rust());
        assert!(spans.iter().all(|(_, k)| *k == Tok::Comment));
    }

    #[test]
    fn hash_comments_unaffected() {
        let p = profile_for("run.sh").unwrap();
        let spans = kinds("# just a comment", &p);
        assert!(spans.iter().all(|(_, k)| *k == Tok::Comment));
    }

    #[test]
    fn word_diff_caps_size() {
        let big = "x".repeat(2000);
        assert!(word_diff(&big, "y").is_none());
    }

    #[test]
    fn word_diff_rejects_dissimilar_pairs() {
        // A rewritten line paired positionally with unrelated code must
        // not produce fragments (the artifact: random pink spans).
        assert!(word_diff("return out;", "   out += hex[c >> 4];").is_none());
        assert!(word_diff("auto x = 1;", "static const char *hex = \"0123\";").is_none());
    }

    #[test]
    fn word_diff_keeps_half_edited_lines() {
        let (old, new) = word_diff(
            "if (unreserved || (!component && reserved)) {",
            "if (reserved || (!component && unreserved)) {",
        )
        .unwrap();
        assert!(old.iter().any(|p| p.changed));
        assert!(new.iter().any(|p| p.changed));
    }
}
