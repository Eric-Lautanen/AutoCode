// chat/mod.rs -- Chat panel module.
// Session tab bar + message bubbles + markdown renderer with syntax
// highlighting + collapsible tool cards with diff views.

mod attachments;
mod code_block;
mod diff_view;
mod input;
mod live;
mod markdown;
mod messages;
mod panel;
mod session;
mod state;
mod tabs;
mod theme;
mod tool_result;

pub(crate) use attachments::stage_paths;

pub use panel::show;
pub use state::ChatPanelState;
pub use theme::ThemeColors;

pub(crate) use live::show_live_turn;
pub(crate) use messages::{show_assistant_content, show_user_bubble};
pub(crate) use theme::theme;
