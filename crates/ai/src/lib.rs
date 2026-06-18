//! AI provider client and chat orchestration for AutoCode.
//!
//! This crate implements the chat loop: sending messages to AI providers,
//! streaming SSE responses, dispatching tool calls (18 tools), handling
//! retry/backoff logic, auto-continuation, and session management. The
//! HTTP client uses raw `TcpStream` + `rustls` with manual SSE parsing
//! (no async runtime). Supports OpenRouter, NVIDIA NIM, and any
//! OpenAI-compatible endpoint with per-model manifests.

pub mod chat;
pub mod helpers;
pub mod provider;
pub mod session;
pub mod thread_pool;
