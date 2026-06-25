/// Strip leading line-number prefixes (e.g. "  42 | " or "42 | ") from text
/// that was copied from read_file output.
pub fn strip_line_numbers(text: &str) -> String {
    let mut all_match = true;
    let mut any_non_empty = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        any_non_empty = true;
        if !line_number_prefix_match(trimmed) {
            all_match = false;
            break;
        }
    }
    if !any_non_empty || !all_match {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        if !result.is_empty() {
            result.push('\n');
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            result.push_str(trimmed);
            continue;
        }
        if let Some(idx) = trimmed.find(" | ") {
            result.push_str(&trimmed[idx + 3..]);
        } else {
            result.push_str(line);
        }
    }
    result
}

fn line_number_prefix_match(s: &str) -> bool {
    let mut chars = s.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            continue;
        }
        if c == ' ' || c == '\t' {
            while let Some(peek) = chars.clone().next() {
                if peek == ' ' || peek == '\t' {
                    chars.next();
                } else {
                    break;
                }
            }
            return chars.next() == Some('|') && chars.next() == Some(' ');
        }
        return false;
    }
    false
}
