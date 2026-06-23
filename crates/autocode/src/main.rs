// autocode - Autonomous AI Coding Assistant

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    if let Err(e) = autocode_ui::run() {
        eprintln!("[fatal] AutoCode exited with error: {}", e);
    }
}
