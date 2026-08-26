// provider/ -- HTTP API client for AI providers.
// Uses only std::net + manual HTTP/HTTPS via a thin blocking wrapper.
// To avoid a heavy async runtime we spawn threads and use channels.

mod client;
mod http;
mod rate_limit;
mod thread_pool;
mod tool_defs;
mod types;
mod web;

// Re-export public API
pub use client::{ProviderClient, count_input_tokens, fetch_models};
pub use rate_limit::{api_rate_limit_record, api_rate_limit_reset, api_rate_limit_wait_ms};
pub use thread_pool::ThreadPool;
pub use tool_defs::tool_definitions;
pub use types::{
    ApiMessage, CompletionRequest, CompletionStream, ProviderEvent, ToolCall, ToolChoice,
};
pub use web::{native_get, native_post, render_via_chrome, set_web_rate_limit_ms};
