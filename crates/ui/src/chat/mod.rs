// chat/mod.rs -- Chat panel module.
// Session tab bar + message bubbles + markdown renderer with syntax
// highlighting + collapsible tool cards with diff views.

mod code_block;
mod diff_view;
mod input;
mod markdown;
mod messages;
mod panel;
mod session;
mod state;
mod tabs;
mod theme;
mod tool_result;

pub use panel::show;
pub use state::ChatPanelState;
pub use theme::ThemeColors;
