/// Returns true if `text` matches the given pattern (with optional/incomplete
/// regex support).  When the pattern contains no regex metacharacters this
/// simply checks if `text` *starts with or contains* the pattern (caller
/// decides).  When it does, a minimal backtracking engine is used.
pub fn matches_pattern(pattern: &str, text: &str, anchored: bool) -> bool {
    // Fast path: if the pattern is a plain literal with no metacharacters,
    // just do a substring search.  The caller can anchor by prepending ^.
    if !has_regex_meta(pattern) {
        if anchored {
            text.starts_with(pattern)
        } else {
            text.contains(pattern)
        }
    } else if pattern.contains('|') {
        // Alternation: split on '|' and match if any alternative matches.
        // This handles the common case of "foo|bar|baz" search patterns.
        // We must be careful not to split on escaped pipes or pipes inside
        // character classes — but for the grep use case, simple splitting
        // on top-level '|' is sufficient and expected.
        for alt in pattern.split('|') {
            let alt = alt.trim();
            if alt.is_empty() {
                continue;
            }
            if matches_pattern(alt, text, anchored) {
                return true;
            }
        }
        false
    } else {
        // Pattern has regex metacharacters — use the regex engine.
        match_simple_regex(pattern, text, anchored)
    }
}

/// Returns true if a pattern string contains regex metacharacters.
fn has_regex_meta(s: &str) -> bool {
    s.contains(|c: char| {
        matches!(
            c,
            '^' | '$' | '.' | '*' | '+' | '?' | '[' | ']' | '(' | ')' | '|' | '\\'
        )
    })
}

/// A compiled pattern node.
#[derive(Debug, Clone)]
enum Pat {
    /// A literal character sequence to match exactly.
    Lit(String),
    /// A single character (`.`).
    Any,
    /// Zero or more of the preceding node.
    Star(Box<Pat>),
    /// One or more of the preceding node.
    Plus(Box<Pat>),
    /// Zero or one of the preceding node.
    Opt(Box<Pat>),
    /// Character class: set of allowed chars, negated flag.
    Class {
        chars: Vec<char>,
        ranges: Vec<(char, char)>,
        negated: bool,
    },
    /// Start-of-string anchor.
    AnchorStart,
    /// End-of-string anchor.
    AnchorEnd,
}

/// Compile a pattern string into a list of `Pat` nodes.
/// Quantifiers (`*`, `+`, `?`) are applied immediately to the preceding node.
fn compile_pattern(pattern: &str) -> Vec<Pat> {
    let mut nodes: Vec<Pat> = Vec::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Handle quantifiers: apply to the PREVIOUSLY emitted node.
        if matches!(c, '*' | '+' | '?') {
            i += 1;
            if let Some(prev) = nodes.pop() {
                let pat = match c {
                    '*' => Pat::Star(Box::new(prev)),
                    '+' => Pat::Plus(Box::new(prev)),
                    '?' => Pat::Opt(Box::new(prev)),
                    _ => unreachable!(),
                };
                nodes.push(pat);
            }
            continue;
        }

        if c == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            let lit = match next {
                'd' => {
                    i += 2;
                    nodes.push(Pat::Class {
                        chars: ('0'..='9').collect(),
                        ranges: vec![],
                        negated: false,
                    });
                    continue;
                }
                'w' => {
                    i += 2;
                    let mut w_chars: Vec<char> =
                        ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
                    w_chars.push('_');
                    nodes.push(Pat::Class {
                        chars: w_chars,
                        ranges: vec![],
                        negated: false,
                    });
                    continue;
                }
                's' => {
                    i += 2;
                    let s_chars: Vec<char> = " \t\n\r\u{0C}".chars().collect();
                    nodes.push(Pat::Class {
                        chars: s_chars,
                        ranges: vec![],
                        negated: false,
                    });
                    continue;
                }
                _ => format!("{}", next), // literal escape
            };
            i += 2;
            nodes.push(Pat::Lit(lit));
            continue;
        }
        if c == '^' {
            nodes.push(Pat::AnchorStart);
            i += 1;
            continue;
        }
        if c == '$' {
            nodes.push(Pat::AnchorEnd);
            i += 1;
            continue;
        }
        if c == '.' {
            i += 1;
            nodes.push(Pat::Any);
            continue;
        }
        if c == '[' {
            let (cls, end) = parse_char_class(&chars, i);
            nodes.push(cls);
            i = end;
            continue;
        }
        if c == '(' {
            // Simple group: skip the group markers, parse contents inline.
            i += 1;
            continue;
        }
        if c == ')' {
            i += 1;
            continue;
        }
        if c == '|' {
            // Alternation is handled at the matches_pattern level before
            // compilation, so reaching here means the pattern was not split
            // (e.g. it was inside a group). Treat as literal for safety.
            i += 1;
            nodes.push(Pat::Lit("|".to_string()));
            continue;
        }
        // Literal character (each char is its own node so quantifiers can apply correctly).
        i += 1;
        nodes.push(Pat::Lit(c.to_string()));
    }
    nodes
}

fn parse_char_class(chars: &[char], start: usize) -> (Pat, usize) {
    let mut cset: Vec<char> = Vec::new();
    let mut ranges: Vec<(char, char)> = Vec::new();
    let mut i = start + 1; // skip '['
    let negated = if i < chars.len() && chars[i] == '^' {
        i += 1;
        true
    } else {
        false
    };
    while i < chars.len() && chars[i] != ']' {
        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
            ranges.push((chars[i], chars[i + 2]));
            i += 3;
        } else {
            cset.push(chars[i]);
            i += 1;
        }
    }
    if i < chars.len() {
        i += 1; // skip ']'
    }
    (
        Pat::Class {
            chars: cset,
            ranges,
            negated,
        },
        i,
    )
}

/// Convert a pattern string into a list of `Pat` nodes with quantifiers attached.
fn compile_with_quantifiers(pattern: &str) -> Vec<Pat> {
    compile_pattern(pattern)
}

/// Match `text` against the compiled pattern nodes, returning true on success.
fn match_nodes(nodes: &[Pat], text: &str, anchored: bool) -> bool {
    if nodes.is_empty() {
        return text.is_empty();
    }

    let text_chars: Vec<char> = text.chars().collect();
    let text_len = text_chars.len();

    // If anchored, start search at position 0 only.
    // If not anchored, try starting at each position.
    let start_positions: Vec<usize> = if anchored || matches!(nodes.first(), Some(Pat::AnchorStart))
    {
        vec![0]
    } else {
        (0..=text_len).collect()
    };

    for start in start_positions {
        if try_match_at(&text_chars, start, nodes, 0) {
            return true;
        }
    }
    false
}

/// Try to match `nodes[node_idx..]` against `text_chars[pos..]`.
/// Returns true if the remaining nodes consume the remaining text.
fn try_match_at(text: &[char], pos: usize, nodes: &[Pat], node_idx: usize) -> bool {
    if node_idx >= nodes.len() {
        // All nodes consumed -- must be at end of text (or trailing $ anchor).
        // If we still have a trailing $, it means the rest matches an empty string.
        return true;
    }

    let node = &nodes[node_idx];
    let rest_nodes = &nodes[node_idx + 1..];

    match node {
        Pat::AnchorStart => {
            if pos == 0 {
                try_match_at(text, pos, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::AnchorEnd => {
            if pos >= text.len() {
                try_match_at(text, pos, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Lit(lit) => {
            let lc: Vec<char> = lit.chars().collect();
            if pos + lc.len() <= text.len() && text[pos..pos + lc.len()] == lc[..] {
                try_match_at(text, pos + lc.len(), rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Any => {
            if pos < text.len() {
                try_match_at(text, pos + 1, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Class {
            chars,
            ranges,
            negated,
        } => {
            if pos >= text.len() {
                return false;
            }
            let ch = text[pos];
            let in_class =
                chars.contains(&ch) || ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi);
            let matched = if *negated { !in_class } else { in_class };
            if matched {
                try_match_at(text, pos + 1, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Star(inner) => {
            // Try 0 or more repetitions
            for count in 0..=(text.len() - pos) {
                let mut p = pos;
                let mut ok = true;
                for _ in 0..count {
                    if p >= text.len() || !match_single(inner, text[p]) {
                        ok = false;
                        break;
                    }
                    p += 1;
                }
                if ok && try_match_at(text, p, rest_nodes, 0) {
                    return true;
                }
                // Early exit for large texts
                if count > 256 {
                    break;
                }
            }
            false
        }
        Pat::Plus(inner) => {
            // Try 1 or more repetitions
            for count in 1..=(text.len() - pos) {
                let mut p = pos;
                let mut ok = true;
                for _ in 0..count {
                    if p >= text.len() || !match_single(inner, text[p]) {
                        ok = false;
                        break;
                    }
                    p += 1;
                }
                if ok && try_match_at(text, p, rest_nodes, 0) {
                    return true;
                }
                if count > 256 {
                    break;
                }
            }
            false
        }
        Pat::Opt(inner) => {
            // Try 0 repetitions
            if try_match_at(text, pos, rest_nodes, 0) {
                return true;
            }
            // Try 1 repetition
            if pos < text.len() && match_single(inner, text[pos]) {
                try_match_at(text, pos + 1, rest_nodes, 0)
            } else {
                false
            }
        }
    }
}

fn match_single(pat: &Pat, ch: char) -> bool {
    match pat {
        Pat::Any => true,
        Pat::Lit(s) => s.starts_with(ch),
        Pat::Class {
            chars,
            ranges,
            negated,
        } => {
            let in_class =
                chars.contains(&ch) || ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi);
            if *negated { !in_class } else { in_class }
        }
        _ => false,
    }
}

/// Entry point for the simple regex engine.
fn match_simple_regex(pattern: &str, text: &str, anchored: bool) -> bool {
    let nodes = compile_with_quantifiers(pattern);
    match_nodes(&nodes, text, anchored)
}
