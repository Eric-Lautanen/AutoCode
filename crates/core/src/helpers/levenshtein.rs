// levenshtein.rs -- Levenshtein distance computation.

/// Compute the Levenshtein distance between two strings.
/// Uses the standard O(n*m) dynamic-programming algorithm with an early
/// exit when the length difference alone exceeds half the shorter string's
/// length (no amount of edits can close that gap cheaply).
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let a_chars: Vec<char> = a.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }
    if (a_len as i64 - b_len as i64).unsigned_abs() > a_len as u64 / 2 {
        return a_len.max(b_len);
    }
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0usize; b_len + 1];
    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}
