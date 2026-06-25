// time.rs -- Time formatting helpers.

pub fn format_time(ts: u64) -> String {
    let secs = ts % 86400;
    format!(
        "{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
