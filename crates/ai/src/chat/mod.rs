// Re-export public API from submodules
pub use agents::{cancel_agent, settle_agents_on_stop};
pub use completion::{
    auto_continue, auto_execute, cancel_queued_message, check_auto_handoff, handle_handoff,
    inject_queued_message_now, queue_message, send_message, start_completion, take_queued_message,
};
pub use errors::{fix_provider_params, shorten_err};
pub use looping::apply_looping_window;
pub use polling::{update_all, update_runtime};
pub use runtime::{
    AgentHandle, AgentOutcome, BlinkKind, ChatRuntime, NetworkStatus, QueuedMessage, ToolResult,
};
pub use session::{delete_session, ensure_session};
pub use session_ops::{
    abort_for_session, context_usage_info_for_session, format_context_usage,
    project_root_for_session, push_error, push_runtime, push_to_session,
    push_tool_results_to_state, replay_to_message, trim_session_ram,
};
pub use tools::{
    ToolExecCtx, build_tool_meta, execute_tool_with_cache, file_tool_meta, kill_process,
};

mod agents;
mod completion;
mod errors;
mod looping;
mod polling;
mod runtime;
mod session;
mod session_ops;
mod tools;
