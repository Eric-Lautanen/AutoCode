//! Core types, utilities, and foundational infrastructure for AutoCode.
//!
//! This crate provides the canonical persistent application state
//! (`AppState`, `Project`, `Session`, `ChatMessage`), a tiny regex engine,
//! token estimation (heuristic + tiktoken), filesystem path utilities with
//! Windows `\\?\` extended-length path support, HTML scraping helpers,
//! cross-platform system-info detection (OS/CPU/GPU/RAM/tools), session
//! persistence (atomic JSON + JSONL), a dark-theme color palette, and
//! debug logging.

pub mod extract;
pub mod fsutil;
pub mod helpers;
pub mod session_storage;
pub mod state;
pub mod sysinfo;
pub mod tokenizer;
pub mod theme;
