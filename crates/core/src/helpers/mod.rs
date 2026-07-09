pub mod id;
pub mod paths;
pub mod regex;
pub mod sanitize;
pub mod serde_defaults;
pub mod tokens;
pub mod utils;

// Re-export all public items so external callers (`crate::helpers::gen_id`, etc.)
// continue to work unchanged.

pub use sanitize::sanitize_tool_calls;

pub use id::{ID_COUNTER, generate_id, generate_session_id, unix_now};

pub use tokens::{
    compute_request_estimate, estimate_full_request_tokens, estimate_message_tokens,
    estimate_single_message_json_tokens, estimate_tokens, estimate_tokens_json,
    estimate_tools_tokens, is_cjk,
};

pub use paths::{
    LruPathCache, blocked_error, is_blocked_path, resolve_path, resolve_path_cached,
    resolve_path_write, resolve_path_write_cached,
};

pub use regex::{has_regex_meta, matches_pattern};

pub use serde_defaults::{
    default_context_tokens, default_disk_read_delay_ms, default_disk_write_rate_ms,
    default_handoff_continuation_prompt_string, default_handoff_enabled, default_handoff_percent,
    default_handoff_trigger_prompt_string, default_loop_warning_prompt_string,
    default_max_output_tokens, default_max_output_tokens_thinking, default_max_retries,
    default_max_retry_wait, default_reasoning_effort, default_request_timeout,
    default_shell_timeout, default_shell_timeout_max, default_stream_idle_timeout,
    default_supports_strict_tools, default_temperature, default_thinking_mode,
    default_tool_timeout, default_top_p, default_ui_display_window, default_web_rate_limit_ms,
    deserialize_secret, serialize_secret,
};

pub use utils::{
    budget_fraction, manifest, model_manifest, model_or_safe, panic_msg, parse_thinking_api,
    provider_ids, provider_manifest, reasoning_efforts_for_provider, safe_model_defaults,
    sanitize_display_text, sanitize_filename, truncate_middle, truncate_str, unique_data_dir_name,
    update_full_estimate, usage_display,
};
