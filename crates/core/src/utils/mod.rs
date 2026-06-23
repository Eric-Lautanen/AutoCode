pub mod extract;
pub mod fsutil;
pub mod sysinfo;

// Re-export all public items so existing call sites continue to work.
// Old paths: crate::fsutil::*, crate::extract::*, crate::sysinfo::*
pub use extract::{extract_ddg_results, extract_html_content, search_cache_get, search_cache_set};
pub use fsutil::{
    TEMP_FILES, create_dir_all, display_path, exe_dir, extended_path, is_dir, metadata, read_dir,
    read_to_string, remove_dir, remove_file, rename, set_exe_dir_for_test, track_temp_file,
    untrack_temp_file, write, write_cmd_script,
};
pub use sysinfo::{
    SysInfo, ToolProbeEntry, grep_note, grep_note_from, has_opengl, is_ready, seed_from_persisted,
    shell_tools_note, shell_tools_note_from, start_detect,
};
