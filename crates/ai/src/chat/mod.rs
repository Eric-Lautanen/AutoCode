// Re-export public API from submodules
pub use completion::{
    auto_continue, auto_execute, check_auto_handoff, handle_handoff, send_message, start_completion,
};
pub use errors::{fix_provider_params, is_transient_error, shorten_err};
pub use looping::apply_looping_window;
pub use polling::{update_all, update_runtime};
pub use runtime::{BlinkKind, ChatRuntime, NetworkStatus, ToolResult};
pub use session::{delete_session, ensure_session};
pub use session_ops::{
    abort_for_session, context_usage_info_for_session, format_context_usage,
    project_root_for_session, push_error, push_runtime, push_to_session,
    push_tool_results_to_state, recompute_estimate_from_disk, refresh_tool_tokens_cache,
    replay_to_message, tool_defs_tokens_for_session, trim_session_ram, update_session_estimate,
};
pub use tools::{
    ToolExecCtx, build_tool_meta, execute_tool_with_cache, file_tool_meta, kill_process,
};

mod completion;
mod errors;
mod looping;
mod polling;
mod runtime;
mod session;
mod session_ops;
mod tools;
