pub fn tool_error(message: &str, suggestion: &str) -> String {
    if suggestion.is_empty() {
        format!(
            "{{\"error\":{}}}",
            serde_json::Value::String(message.to_string())
        )
    } else {
        format!(
            "{{\"error\":{},\"suggestion\":{}}}",
            serde_json::Value::String(message.to_string()),
            serde_json::Value::String(suggestion.to_string()),
        )
    }
}
