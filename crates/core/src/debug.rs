// debug.rs — Debug logging (no-op). All I/O removed.

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}

pub fn init() {}

pub fn panic_msg(panic_info: &Box<dyn std::any::Any + Send>) -> String {
    panic_info
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic_info.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap_or("unknown panic")
        .to_string()
}
