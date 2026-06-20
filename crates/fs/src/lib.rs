//! Filesystem tool implementations for AutoCode.
//!
//! Provides shell command execution (background threads with channel-based
//! output streaming), a gitignore-aware file explorer (list_dir, glob, grep),
//! file extraction from AI code-fence output, and glob matching utilities.

pub mod explorer;
pub mod helpers;
pub mod shell;
pub mod skills;