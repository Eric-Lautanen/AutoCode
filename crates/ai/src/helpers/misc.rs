use autocode_core::state::AppState;

/// Format the current UTC time as "YYYY-MM-DD HH:MM:SS" (e.g. "2026-08-02 14:30:45").
/// Injected into model-facing messages so the model can see the wall-clock time.
pub fn format_now_utc() -> String {
    let secs = autocode_core::helpers::unix_now() as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86400));
    let rem = secs.rem_euclid(86400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s)
}

/// Convert days since 1970-01-01 to (year, month, day) in the proleptic Gregorian
/// calendar (Howard Hinnant's civil-from-days algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Generate a random tool-call ID in UUID v4 format (e.g.
/// "6acbfb8e-63e6-4fd5-907b-ecc1b366f09c"). Used for synthetic bootstrap
/// messages that simulate a tool call/result pair.
pub fn gen_tool_call_id() -> String {
    let mut rng = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    // Mix in a monotonic counter so two IDs generated in the same nanosecond
    // on the same thread still differ.
    rng ^= autocode_core::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    project_context_for_project(state, state.active_project_id.as_deref())
}

/// Build a PROJECT CONTEXT string for a specific project id. Unlike
/// [`project_context_string`] this never falls back to the app-active
/// project, so background runtimes describe their own project.
pub fn project_context_for_project(state: &AppState, project_id: Option<&str>) -> String {
    let Some(pid) = project_id else {
        return String::new();
    };
    let proj = match state.projects.iter().find(|p| p.id == pid) {
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
