//! UI panels and widgets for AutoCode.
//!
//! Implements the egui-based user interface: chat panel with message bubbles
//! (markdown, diffs, reasoning, streaming, live shell output), settings window
//! (7 tabs), file explorer tree with preview, floating task list, toolbar
//! with project/session/provider pickers, and various UI helpers.

pub mod helpers;
pub mod ui_chat;
pub mod ui_explorer;
pub mod ui_settings;
pub mod ui_todo;
pub mod ui_toolbar;
