use crate::state::SecretString;
use serde::Deserialize;

// -- Serde helpers for SecretString --------------------------------------------

pub fn serialize_secret<S: serde::Serializer>(val: &SecretString, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(val.as_str())
}

pub fn deserialize_secret<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<SecretString, D::Error> {
    Ok(SecretString::new(String::deserialize(d)?))
}

// -- Default value functions for serde -----------------------------------------

pub fn default_context_tokens() -> u32 {
    128_000
}
pub fn default_handoff_percent() -> u8 {
    80
}
pub fn default_handoff_trigger_prompt_string() -> String {
    crate::state::DEFAULT_HANDOFF_TRIGGER_PROMPT.to_string()
}
pub fn default_handoff_enabled() -> bool {
    true
}
pub fn default_use_headless_chrome() -> bool {
    true
}
pub fn default_handoff_continuation_prompt_string() -> String {
    crate::state::DEFAULT_HANDOFF_CONTINUATION_PROMPT.to_string()
}
pub fn default_loop_warning_prompt_string() -> String {
    crate::state::DEFAULT_LOOP_WARNING_PROMPT.to_string()
}
pub fn default_thinking_mode() -> bool {
    false
}
pub fn default_reasoning_effort() -> String {
    "high".into()
}
pub fn default_temperature() -> f32 {
    0.2
}
pub fn default_top_p() -> f32 {
    1.0
}
pub fn default_max_output_tokens() -> u32 {
    16384
}
pub fn default_max_output_tokens_thinking() -> u32 {
    32768
}
pub fn default_stream_idle_timeout() -> u64 {
    120
}
pub fn default_request_timeout() -> u64 {
    300
}
pub fn default_tool_timeout() -> u64 {
    300
}
pub fn default_shell_timeout() -> u64 {
    300
}
pub fn default_shell_timeout_max() -> u64 {
    600
}
pub fn default_max_retries() -> u8 {
    3
}
pub fn default_max_retry_wait() -> u64 {
    900
}
pub fn default_ui_display_window() -> usize {
    50
}
pub fn default_disk_read_delay_ms() -> u64 {
    300
}
pub fn default_web_rate_limit_ms() -> u64 {
    1500
}
pub fn default_disk_write_rate_ms() -> u64 {
    300
}
pub fn default_supports_strict_tools() -> bool {
    true
}
