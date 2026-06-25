use autocode_core::state::AppState;

/// Generate a random tool-call ID in UUID v4 format (e.g.
/// "6acbfb8e-63e6-4fd5-907b-ecc1b366f09c"). Used for synthetic bootstrap
/// messages that simulate a tool call/result pair.
pub fn gen_tool_call_id() -> String {
    let mut rng = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    // xorshift64 for cheap pseudo-randomness
    let mut next = || -> u64 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut buf = String::with_capacity(36);
    // 8-4-4-4-12 hex groups
    for (i, len) in [8u8, 4, 4, 4, 12].iter().enumerate() {
        if i > 0 {
            buf.push('-');
        }
        let mut remaining = *len as usize;
        while remaining > 0 {
            let v = next();
            // Each u64 gives up to 16 hex chars
            let hex = format!("{:016x}", v);
            let take = remaining.min(16);
            buf.push_str(&hex[..take]);
            remaining -= take;
        }
    }
    // Set version nibble to 4 (UUID v4) and variant to 8/9/a/b
    if buf.len() == 36 {
        // Version: position 14 → '4'
        buf.replace_range(14..15, "4");
        // Variant: position 19 → '8', '9', 'a', or 'b'
        let variant = match buf.as_bytes()[19] {
            b'0'..=b'3' => '8',
            b'4'..=b'7' => '9',
            b'8'..=b'9' | b'a'..=b'b' => 'a',
            _ => 'b',
        };
        buf.replace_range(19..20, &variant.to_string());
    }
    buf
}

/// Build a PROJECT CONTEXT string (name, root path, top-level entries).
/// Returns empty string if no active project.
pub fn project_context_string(state: &AppState) -> String {
    let proj = match state.active_project() {
        Some(p) => p,
        None => return String::new(),
    };
    let mut ctx = format!(
        "\nPROJECT CONTEXT\nName: {}\nRoot: {}\n",
        proj.name, proj.root_path
    );
    if let Ok(entries) = std::fs::read_dir(&proj.root_path) {
        let mut items: Vec<String> = entries
            .filter_map(|e| {
                let e = e.ok()?;
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    return None;
                }
                let suffix = if e.file_type().ok().is_some_and(|t| t.is_dir()) {
                    "/"
                } else {
                    ""
                };
                Some(format!("  {}{}", name, suffix))
            })
            .collect();
        items.sort();
        for item in items {
            ctx.push_str(&item);
            ctx.push('\n');
        }
    }
    ctx.truncate(ctx.trim_end().len());
    ctx
}
