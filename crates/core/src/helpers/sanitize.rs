/// Validate and auto-fix tool_calls arguments that contain corrupt/non-JSON data.
/// Modifies the tool_calls Value in place, removing any function call whose
/// arguments field is not valid JSON after repair attempts.
/// Returns true if any changes were made.
pub fn sanitize_tool_calls(tool_calls: &mut Option<serde_json::Value>) -> bool {
    let Some(arr) = tool_calls.as_mut().and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    let mut i = 0;
    while i < arr.len() {
        let args_str = match arr[i]["function"]["arguments"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                i += 1;
                continue;
            }
        };
        if serde_json::from_str::<serde_json::Value>(&args_str).is_ok() {
            i += 1;
            continue;
        }
        changed = true;
        // Attempt repair: try to re-escape content by parsing raw bytes as JSON string.
        if let Ok(repaired) = serde_json::from_str::<String>(&format!("\"{}\"", args_str))
            && serde_json::from_str::<serde_json::Value>(&repaired).is_ok()
        {
            arr[i]["function"]["arguments"] = serde_json::Value::String(repaired);
            i += 1;
            continue;
        }
        // Last resort: find the longest valid JSON prefix.
        let mut end = args_str.len();
        let mut fixed = false;
        for _ in 0..args_str.len().min(256) {
            if end <= 2 {
                break;
            }
            end = args_str.floor_char_boundary(end - 1);
            if serde_json::from_str::<serde_json::Value>(&args_str[..end]).is_ok() {
                arr[i]["function"]["arguments"] =
                    serde_json::Value::String(args_str[..end].to_string());
                fixed = true;
                i += 1;
                break;
            }
            if let Some(prev_quote) = args_str[..end].rfind('"') {
                end = prev_quote + 1;
            }
        }
        if !fixed {
            arr.remove(i);
        }
    }
    changed
}
