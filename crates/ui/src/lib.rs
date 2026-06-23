//! UI panels and widgets for AutoCode.
//!
//! Implements the egui-based user interface: chat panel with user message
//! bubbles and inline assistant/tool content (markdown, diffs, code blocks,
//! terminal output), settings window (6 tabs), file explorer tree with preview,
//! floating task list, toolbar with project/session/provider pickers,
//! and various UI helpers.

pub mod helpers;
pub mod theme;
pub mod ui_chat;
pub mod ui_explorer;
pub mod ui_project_tasks;
pub mod ui_settings;
pub mod ui_todo;
pub mod ui_todo_window;
pub mod ui_toolbar;
