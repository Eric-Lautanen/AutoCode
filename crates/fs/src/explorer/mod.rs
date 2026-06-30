// explorer/ -- File system traversal for the file explorer panel.

mod comment;
mod fuzzy;
mod gitignore;
mod glob;
mod grep;
mod listing;
mod read_file;
mod tree;

// Re-export public API so external consumers can use `autocode_fs::explorer::Foo`.
pub use gitignore::find_project_root;
pub use glob::glob_files;
pub use grep::grep_files;
pub use listing::{FsEntry, list_dir, list_dir_all, merge_git_status};
pub use read_file::read_file;
pub use tree::project_tree;
