// glob_match.rs -- Minimal glob matching utilities.

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
