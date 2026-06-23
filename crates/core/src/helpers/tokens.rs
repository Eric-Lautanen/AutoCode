use crate::state::ChatMessage;

/// Estimate token count for text using an improved heuristic.
/// This is a fallback when tiktoken or API-based counting is unavailable.
/// Accuracy: ~10-15% for code, ~5-10% for English prose.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if text.len() == 1 {
        return 1;
    }

    let mut word_count = 0usize;
    let mut symbol_count = 0usize;
    let mut cjk_count = 0usize;
    let mut in_word = false;

    // Common code symbols that are typically separate tokens or part of operators
    const CODE_SYMBOLS: &[char] = &[
        '{', '}', '(', ')', '[', ']', ';', ',', '.', ':', '+', '-', '*', '/', '%', '<', '>', '=',
        '!', '&', '|', '^', '~', '?', '@', '#', '$', '\\', '`', '\'', '"',
    ];

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if !in_word {
                word_count += 1;
                in_word = true;
            }
        } else if CODE_SYMBOLS.contains(&ch) {
            symbol_count += 1;
            in_word = false;
        } else if ch.is_whitespace() {
            in_word = false;
        } else {
            // Other punctuation/unicode
            in_word = false;
            if is_cjk(ch) {
                cjk_count += 1;
            } else {
                symbol_count += 1;
            }
        }
    }

    // Detect if text is code-like (high symbol density or code keywords).
    // Keywords are matched at word boundaries (preceded by whitespace or start of text).
    let total_chars = text.chars().count();
    let symbol_density = symbol_count as f32 / total_chars.max(1) as f32;
    let has_code_keyword = text.starts_with("fn ")
        || text.starts_with("function ")
        || text.starts_with("def ")
        || text.starts_with("class ")
        || text.starts_with("struct ")
        || text.starts_with("impl ")
        || text.starts_with("pub ")
        || text.starts_with("const ")
        || text.starts_with("let ")
        || text.starts_with("var ")
        || text.contains("\nfn ")
        || text.contains("\nfunction ")
        || text.contains("\ndef ")
        || text.contains("\nclass ")
        || text.contains("\nstruct ")
        || text.contains("\nimpl ")
        || text.contains("\npub ")
        || text.contains("\nconst ")
        || text.contains("\nlet ")
        || text.contains("\nvar ")
        || text.contains("=>")
        || text.contains("->")
        || text.contains("::");
    let is_code = symbol_density > 0.08 || has_code_keyword;

    // Token estimation based on content type
    // Code: ~3.2 chars/token, English: ~4.0 chars/token, CJK: ~1.3 tokens/char
    let (word_mult, char_per_token) = if is_code {
        (1.3, 3.2) // Code has more symbols, fewer chars per token
    } else {
        (1.5, 4.0) // Prose
    };

    let word_tokens = (word_count as f32 * word_mult) as usize;
    let symbol_tokens = symbol_count; // Most symbols are 1 token each
    let cjk_tokens = (cjk_count as f32 * 1.3) as usize; // ~1.3 tokens per CJK char
    let char_floor = (total_chars as f32 / char_per_token).ceil() as usize;

    // Combine estimates: max of word+symbol, cjk+word, char_floor
    let combined = word_tokens + symbol_tokens;
    let estimate = combined.max(cjk_tokens + word_tokens).max(char_floor);

    // Per-message overhead for API format (role, formatting)
    estimate.saturating_add(3)
}

/// Estimate tokens for a complete ChatMessage including content, tool_calls, and reasoning_content.
/// This provides a more accurate per-message estimate than just content alone.
pub fn estimate_message_tokens(msg: &crate::state::ChatMessage) -> usize {
    let mut total = estimate_tokens(&msg.content);

    // Add tool_calls overhead (JSON structure + content)
    if let Some(tc) = &msg.tool_calls {
        total += estimate_tokens(&serde_json::to_string(tc).unwrap_or_default());
    }

    // Add reasoning_content if present
    if let Some(rc) = &msg.reasoning_content {
        total += estimate_tokens(rc);
    }

    // Add tool_call_id overhead
    if msg.tool_call_id.is_some() {
        total += 2; // "tool_call_id": "xxx"
    }

    total
}

/// Estimate tokens for a single ChatMessage as it would appear in the API
/// JSON request body. This includes the role label, content, tool_calls,
/// tool_call_id, reasoning_content, and JSON structural overhead (braces,
/// quotes, colons, commas). The result is suitable for caching on the
/// message's `full_token_estimate` field so the session running total can
/// be updated incrementally without re-serializing all messages.
///
/// If `model` is provided, uses tiktoken for accuracy. Otherwise falls back
/// to heuristic.
pub fn estimate_single_message_json_tokens(msg: &ChatMessage, model: Option<&str>) -> usize {
    let obj = serde_json::json!({
        "role": msg.role.label(),
        "content": msg.content,
    });
    // Build the same JSON object that estimate_full_request_tokens would
    // produce for this single message, then count tokens on it.
    let mut obj = obj;
    if let Some(id) = &msg.tool_call_id {
        obj["tool_call_id"] = serde_json::json!(id);
    }
    if let Some(tc) = &msg.tool_calls {
        obj["tool_calls"] = tc.clone();
    }
    if let Some(rc) = &msg.reasoning_content {
        obj["reasoning_content"] = serde_json::json!(rc);
    }

    let json_str = serde_json::to_string(&obj).unwrap_or_default();

    // Try tiktoken first for accuracy
    if let Some(model_name) = model
        && let Some(count) = crate::tokenizer::offline_token_count(model_name, &json_str)
    {
        return count;
    }

    // Fallback to heuristic
    estimate_tokens_json(&json_str)
}

/// Estimate tokens for tool definitions JSON. This is a fixed overhead sent
/// with every request but not part of chat history. Cached so the session
/// running total can be updated incrementally.
///
/// If `model` is provided, uses tiktoken for accuracy. Otherwise falls back
/// to heuristic.
pub fn estimate_tools_tokens(tools_json: &serde_json::Value, model: Option<&str>) -> usize {
    let json_str = serde_json::to_string(tools_json).unwrap_or_default();

    if let Some(model_name) = model
        && let Some(count) = crate::tokenizer::offline_token_count(model_name, &json_str)
    {
        return count;
    }

    estimate_tokens_json(&json_str)
}

/// Estimate tokens for a full API request body by serializing the relevant
/// message fields (content, role, tool_calls, tool_call_id, reasoning_content)
/// into a JSON array and applying the tokenizer/heuristic to the full serialized text.
/// This accounts for JSON structural overhead, tool calls, and reasoning content
/// that the per-message `estimate_tokens(&content)` misses.
///
/// If `model` is provided, uses tiktoken for accurate counting. Otherwise falls back to heuristic.
///
/// **Note:** For incremental updates prefer `estimate_single_message_json_tokens`
/// + `estimate_tools_tokens` to avoid re-serializing all messages every time.
pub fn estimate_full_request_tokens(
    messages: &[ChatMessage],
    tools_json: Option<&serde_json::Value>,
    model: Option<&str>,
) -> usize {
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            let mut obj = serde_json::json!({
                "role": m.role.label(),
                "content": m.content,
            });
            if let Some(id) = &m.tool_call_id {
                obj["tool_call_id"] = serde_json::json!(id);
            }
            if let Some(tc) = &m.tool_calls {
                obj["tool_calls"] = tc.clone();
            }
            if let Some(rc) = &m.reasoning_content {
                obj["reasoning_content"] = serde_json::json!(rc);
            }
            obj
        })
        .collect();

    let mut body = serde_json::json!({
        "messages": msgs,
    });
    if let Some(tools) = tools_json {
        body["tools"] = tools.clone();
    }

    let json_str = serde_json::to_string(&body).unwrap_or_default();

    // Try tiktoken first for accuracy
    if let Some(model_name) = model
        && let Some(count) = crate::tokenizer::offline_token_count(model_name, &json_str)
    {
        return count;
    }

    // Fallback to heuristic with adjusted char/token ratio for JSON
    // JSON has more structural chars (braces, quotes, colons) so ~3.5 chars/token
    estimate_tokens_json(&json_str)
}

/// Heuristic token estimation optimized for JSON text (more structural characters).
fn estimate_tokens_json(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if text.len() == 1 {
        return 1;
    }

    let mut word_count = 0usize;
    let mut symbol_count = 0usize;
    let mut cjk_count = 0usize;
    let mut in_word = false;

    const JSON_SYMBOLS: &[char] = &['{', '}', '[', ']', ':', ',', '"', '\\'];

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if !in_word {
                word_count += 1;
                in_word = true;
            }
        } else if JSON_SYMBOLS.contains(&ch) {
            symbol_count += 1;
            in_word = false;
        } else if ch.is_whitespace() {
            in_word = false;
        } else {
            // Other punctuation/unicode
            in_word = false;
            if is_cjk(ch) {
                cjk_count += 1;
            } else {
                symbol_count += 1;
            }
        }
    }

    let total_chars = text.chars().count();
    // JSON: ~3.5 chars/token due to structural overhead
    let char_floor = (total_chars as f32 / 3.5).ceil() as usize;
    let word_tokens = (word_count as f32 * 1.3) as usize;
    let symbol_tokens = symbol_count;
    let cjk_tokens = (cjk_count as f32 * 1.3) as usize;

    let combined = word_tokens + symbol_tokens;
    combined
        .max(cjk_tokens + word_tokens)
        .max(char_floor)
        .saturating_add(3)
}

pub fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{AC00}'..='\u{D7AF}'
        | '\u{3040}'..='\u{30FF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{20000}'..='\u{2A6DF}'
        | '\u{2A700}'..='\u{2B73F}'
    )
}
