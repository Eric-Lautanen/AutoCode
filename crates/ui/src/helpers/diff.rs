// diff.rs -- Diff algorithm helpers (LCS-based and simple line-by-line).

pub struct DiffLine<'a> {
    pub prefix: char,
    pub text: &'a str,
    /// 1-based line number in the old file (0 for additions)
    pub old_lineno: usize,
    /// 1-based line number in the new file (0 for deletions)
    pub new_lineno: usize,
}

/// LCS-based diff (O(n*m) time/space). Falls back to simple diff for large files.
pub fn lcs_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let n = old.len();
    let m = new.len();
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    for i in 0..n {
        for j in 0..m {
            table[idx(i + 1, j + 1)] = if old[i] == new[j] {
                table[idx(i, j)] + 1
            } else {
                table[idx(i, j + 1)].max(table[idx(i + 1, j)])
            };
        }
    }

    let mut result = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: j,
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[idx(i, j - 1)] >= table[idx(i - 1, j)]) {
            result.push(DiffLine {
                prefix: '+',
                text: new[j - 1],
                old_lineno: 0,
                new_lineno: j,
            });
            j -= 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: 0,
            });
            i -= 1;
        }
    }
    result.reverse();
    result
}

/// Plain-text rendering of the unified diff shown by
/// `chat::diff_view::render_unified_diff`, for the copy button. Same
/// LCS/simple algorithm, same 3-line context hunks and ` [...] ` separators,
/// flattened to `"{num:>w} |{prefix} {text}"` lines so the clipboard matches
/// what the user sees (including file line numbers from `line_offset`).
pub fn format_unified_diff(old: &str, new: &str, line_offset: usize) -> String {
    const CONTEXT: usize = 3;
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let line_data = if old_lines.len() < 2000 && new_lines.len() < 2000 {
        lcs_diff_lines(&old_lines, &new_lines)
    } else {
        simple_diff_lines(&old_lines, &new_lines)
    };

    let mut change_runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, dl) in line_data.iter().enumerate() {
        if dl.prefix != ' ' {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start.take() {
            change_runs.push((start, i));
        }
    }
    if let Some(start) = run_start {
        change_runs.push((start, line_data.len()));
    }

    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (start, end) in &change_runs {
        let hs = start.saturating_sub(CONTEXT);
        let he = (*end + CONTEXT).min(line_data.len());
        if let Some((_ps, pe)) = hunks.last_mut()
            && hs <= *pe
        {
            *pe = he.max(*pe);
            continue;
        }
        hunks.push((hs, he));
    }

    // Flatten hunks; `None` marks a ` [...] ` gap between hunks.
    let mut flat: Vec<Option<&DiffLine<'_>>> = Vec::new();
    for (hi, (start, end)) in hunks.iter().enumerate() {
        if hi > 0 {
            flat.push(None);
        }
        for dl in &line_data[*start..*end] {
            flat.push(Some(dl));
        }
    }

    if flat.is_empty() {
        return "(no differences)".to_string();
    }

    let max_line_num = flat
        .iter()
        .flatten()
        .map(|dl| {
            let raw = if dl.prefix == '-' {
                dl.old_lineno
            } else {
                dl.new_lineno
            };
            if raw > 0 { raw + line_offset } else { 0 }
        })
        .max()
        .unwrap_or(0);
    let num_width = max_line_num.to_string().len().max(2);

    let mut out = String::new();
    for entry in flat {
        let Some(dl) = entry else {
            out.push_str(" [...] \n");
            continue;
        };
        let raw_num = if dl.prefix == '-' {
            dl.old_lineno
        } else {
            dl.new_lineno
        };
        let line_num = if raw_num > 0 {
            raw_num + line_offset
        } else {
            0
        };
        out.push_str(&format!(
            "{:>width$} |{} {}\n",
            line_num,
            dl.prefix,
            dl.text.trim_end(),
            width = num_width
        ));
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Simple line-by-line diff for very large files (>2000 lines).
/// Walks both files greedily, emitting matching lines as context
/// and unmatched lines as deletions / insertions.
pub fn simple_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let mut result = Vec::new();
    let (mut o, mut n) = (0, 0);
    while o < old.len() || n < new.len() {
        if o < old.len() && n < new.len() && old[o] == new[n] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        } else if o >= old.len() {
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            n += 1;
        } else if n >= new.len() {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            o += 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        }
    }
    result
}
