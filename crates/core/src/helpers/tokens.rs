use crate::state::{ChatMessage, Role};

/// Estimate token count for text using a heuristic.
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
    let cjk_tokens = (cjk_count as f32 * 1.3) as usize; // ~1.3 tokens per CJK char
    let char_floor = (total_chars as f32 / char_per_token).ceil() as usize;

    // Combine estimates: max of word, cjk+word, char_floor
    // symbol_tokens is not added separately — real tokenizers merge
    // adjacent symbols into surrounding word tokens, so the chars/token
    // ratio in char_floor already accounts for them.
    let estimate = word_tokens.max(cjk_tokens + word_tokens).max(char_floor);

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
pub fn estimate_single_message_json_tokens(msg: &ChatMessage) -> usize {
    let mut obj = serde_json::json!({
        "role": msg.role.label(),
        "content": msg.content,
    });
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
    estimate_tokens_json(&json_str)
}

/// Estimate tokens for tool definitions JSON. This is a fixed overhead sent
/// with every request but not part of chat history. Cached so the session
/// running total can be updated incrementally.
pub fn estimate_tools_tokens(tools_json: &serde_json::Value) -> usize {
    let json_str = serde_json::to_string(tools_json).unwrap_or_default();
    estimate_tokens_json(&json_str)
}

/// Estimate tokens for a full API request body by serializing the relevant
/// message fields (content, role, tool_calls, tool_call_id, reasoning_content)
/// into a JSON array and applying the heuristic to the full serialized text.
/// This accounts for JSON structural overhead, tool calls, and reasoning content
/// that the per-message `estimate_tokens(&content)` misses.
///
/// **Note:** For incremental updates prefer `estimate_single_message_json_tokens`
/// + `estimate_tools_tokens` to avoid re-serializing all messages every time.
pub fn estimate_full_request_tokens(
    messages: &[ChatMessage],
    tools_json: Option<&serde_json::Value>,
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
    estimate_tokens_json(&json_str)
}

/// Unified token estimation: given messages and tool-definition token count,
/// returns `(estimated_messages_tokens, estimated_full_tokens)`.
///
/// This is THE single pipeline for all token estimation. All callers
/// (push_to_session, start_completion pre-flight, prepare_request_messages,
/// load_session, replay, display) route through this function. The only
/// difference between callers is **when** they call and where messages come from.
///
/// Always does a full serialization of all messages into a single JSON body
/// so the heuristic sees the complete request structure (message separators,
/// JSON array overhead, all tool_calls in one pass). Never relies on
/// per-message cached estimates which diverge from the full picture.
pub fn compute_request_estimate(messages: &[ChatMessage], tools_tokens: usize) -> (usize, usize) {
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != Role::Error)
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
    let body = serde_json::json!({ "messages": msgs });
    let json_str = serde_json::to_string(&body).unwrap_or_default();
    let msg_tokens = estimate_tokens_json(&json_str);
    let full_tokens = msg_tokens.saturating_add(tools_tokens);
    (msg_tokens, full_tokens)
}

/// Heuristic token estimation optimized for JSON text.
pub fn estimate_tokens_json(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if text.len() == 1 {
        return 1;
    }

    let mut word_count = 0usize;
    let mut cjk_count = 0usize;
    let mut in_word = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if !in_word {
                word_count += 1;
                in_word = true;
            }
        } else if ch.is_whitespace() {
            in_word = false;
        } else {
            in_word = false;
            if is_cjk(ch) {
                cjk_count += 1;
            }
        }
    }

    let total_chars = text.chars().count();
    let char_floor = (total_chars as f32 / 4.5).ceil() as usize;
    let word_tokens = (word_count as f32 * 1.3) as usize;
    let cjk_tokens = (cjk_count as f32 * 1.3) as usize;

    word_tokens
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
