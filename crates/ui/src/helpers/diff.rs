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
