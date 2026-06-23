pub mod sanitize;
pub mod id;
pub mod tokens;
pub mod paths;
pub mod regex;
pub mod serde_defaults;
pub mod utils;

// Re-export all public items so external callers (`crate::helpers::gen_id`, etc.)
// continue to work unchanged.

pub use sanitize::sanitize_tool_calls;

pub use id::{generate_id, generate_session_id, unix_now, ID_COUNTER};

pub use tokens::{
    estimate_tokens, estimate_message_tokens, estimate_single_message_json_tokens,
    estimate_tools_tokens, estimate_full_request_tokens, is_cjk,
};

pub use paths::{
    blocked_error, resolve_path, resolve_path_write, resolve_path_cached,
    resolve_path_write_cached, is_blocked_path, LruPathCache,
};

pub use regex::matches_pattern;

pub use serde_defaults::{
    serialize_secret, deserialize_secret, default_context_tokens, default_handoff_percent,
    default_handoff_trigger_prompt_string, default_connection_drop_prompt_string,
    default_handoff_enabled, default_handoff_continuation_prompt_string, default_thinking_mode,
    default_reasoning_effort, default_temperature, default_top_p, default_max_output_tokens,
    default_max_output_tokens_thinking, default_stream_idle_timeout, default_request_timeout,
    default_tool_timeout, default_shell_timeout, default_shell_timeout_max, default_max_retries,
    default_max_retry_wait, default_ui_display_window, default_disk_read_delay_ms,
    default_web_rate_limit_ms, default_disk_write_rate_ms, default_supports_strict_tools,
};

pub use utils::{
    truncate_str, truncate_middle, provider_manifest, model_manifest,
    reasoning_efforts_for_provider, safe_model_defaults, model_or_safe, provider_ids,
    parse_thinking_api, sanitize_filename, unique_data_dir_name, update_full_estimate,
    sanitize_display_text, panic_msg, budget_fraction, usage_display, manifest,
};
