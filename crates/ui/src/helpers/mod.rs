// helpers/mod.rs -- Shared UI helper functions used across multiple UI modules.

mod diff;
mod formatting;
mod time;
mod todo;
mod tool_result;
mod ui_id;
mod widgets;

pub use diff::{DiffLine, lcs_diff_lines, simple_diff_lines};
pub use formatting::{append_rich_inline_to_job, parse_inline_formatting};
pub use time::format_time;
pub use todo::find_current_task_index;
pub use tool_result::{
    CODE_DISPLAY_MAX_LINES, extract_tool_body, extract_tool_summary, get_tool_body,
    parse_path_header, strip_exit_code_trailer,
};
pub use ui_id::{
    data, data_id, get_temp, get_temp_bool, next_id, set_temp, set_temp_bool, take_temp,
    take_temp_bool,
};
pub use widgets::{field_label, section_heading, todo_scroll_area, toolbar_separator};
