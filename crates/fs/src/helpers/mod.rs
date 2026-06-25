// helpers/ -- FS-crate helpers: file extraction from AI output, glob matching utilities.

mod extract;
mod glob_match;
mod levenshtein;

// Re-export public API so external consumers can use `autocode_fs::helpers::Foo`.
pub use extract::{extract_files, write_extracted_files};
pub use glob_match::{glob_match, glob_match_segment};
pub use levenshtein::levenshtein;
