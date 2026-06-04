// debug.rs -- File-based debug logging for diagnosing drops/stalls.
// Writes to %TEMP%\autocode_debug.log (or /tmp/autocode_debug.log on Unix).
// Rotates when the file exceeds ~1 MB.

use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("autocode_debug.log")
}

static LOG: std::sync::LazyLock<Mutex<std::fs::File>> = std::sync::LazyLock::new(|| {
    let path = log_path();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("cannot open debug log");
    Mutex::new(file)
});

fn timestamp() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let secs = ms / 1000;
    let millis = ms % 1000;
    let total_secs = secs % 86400;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

fn rotate_log(path: &std::path::Path) {
    for i in (2..=5).rev() {
        let old = path.with_extension(format!("log.{}", i - 1));
        let new = path.with_extension(format!("log.{}", i));
        if old.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::rename(path, &backup);
}

fn write_log(s: &str) {
    if let Ok(mut f) = LOG.lock() {
        let _ = writeln!(f, "{} {}", timestamp(), s);
        let _ = f.flush();
        if let Ok(meta) = f.metadata()
            && meta.len() > 1024 * 1024
        {
            let path = log_path();
            drop(f);
            rotate_log(&path);
            if let Ok(new_file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                && let Ok(mut f) = LOG.lock()
            {
                *f = new_file;
            }
        }
    }
}

pub fn log_fmt(args: std::fmt::Arguments<'_>) {
    write_log(&args.to_string());
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::debug::log_fmt(format_args!($($arg)*))
    };
}

/// Called at app startup to mark log boundary.
pub fn init() {
    write_log("=== autocode debug log ===");
}

/// Log a panic payload (called from catch_unwind handlers).
pub fn panic_msg(panic_info: &Box<dyn std::any::Any + Send>) -> String {
    panic_info
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic_info.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap_or("unknown panic")
        .to_string()
}
