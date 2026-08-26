// helpers/timing.rs -- Opt-in performance logging behind AUTOCODE_TIMING=1.
// Emits `[timing]` lines on stderr (matching the house eprintln error style)
// so batch/request wall times can be captured without touching the UI.

use std::sync::OnceLock;

/// Whether timing output is enabled. Read once per process from the env.
pub fn timing_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("AUTOCODE_TIMING").is_ok_and(|v| v == "1"))
}

/// Emit a `[timing]` line when enabled. The message is built lazily so the
/// formatting cost is skipped entirely in normal runs.
pub fn log_timing(msg: impl FnOnce() -> String) {
    if timing_enabled() {
        eprintln!("[timing] {}", msg());
    }
}

/// Format a duration as a compact human-readable figure, e.g. "1.234s",
/// "12.5ms", "340us". Pure so the format is unit-testable.
pub fn format_duration(d: std::time::Duration) -> String {
    let micros = d.as_micros();
    if micros < 1_000 {
        format!("{}us", micros)
    } else if micros < 1_000_000 {
        format!("{:.1}ms", micros as f64 / 1_000.0)
    } else {
        format!("{:.3}s", micros as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_units() {
        assert_eq!(
            format_duration(std::time::Duration::from_micros(340)),
            "340us"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_millis(12)),
            "12.0ms"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_micros(12_567)),
            "12.6ms"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_millis(1234)),
            "1.234s"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_secs(61)),
            "61.000s"
        );
    }
}
