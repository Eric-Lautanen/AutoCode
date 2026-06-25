// grep.rs -- File content search with fuzzy suggestion fallback.

use std::path::Path;

use super::fuzzy::{fuzzy_suggest, fuzzy_suggest_single_file, walk_files};
use super::gitignore::{Gitignore, find_project_root};
use autocode_core::helpers::has_regex_meta;
use autocode_core::utils::fsutil;

pub fn grep_files(
    search_path: &Path,
    pattern: &str,
    file_glob: &str,
    case_sensitive: bool,
    max_results: usize,
) -> String {
    let project_root = find_project_root(search_path).unwrap_or_else(|| search_path.to_path_buf());
    let project_root = autocode_core::utils::fsutil::extended_path(&project_root);
    let search_path = &autocode_core::utils::fsutil::extended_path(search_path);

    if search_path.is_file() {
        let mut results: Vec<String> = Vec::new();
        let content = search_file_for_pattern(
            search_path,
            pattern,
            case_sensitive,
            max_results,
            &mut results,
        );
        if results.is_empty() {
            let mut msg = format!(
                "No matches for \"{}\" in {}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display()
            );
            if !has_regex_meta(pattern)
                && let Some(content) = content
            {
                let suggestions = fuzzy_suggest_single_file(&content, pattern, case_sensitive);
                if !suggestions.is_empty() {
                    msg.push_str(&format!(
                        ". Try grep again with one of: {}",
                        suggestions
                            .iter()
                            .map(|s| format!("\"{}\"", s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            msg
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display(),
                results.len(),
                results.join("\n")
            )
        }
    } else {
        let gitignore_path = project_root.join(".gitignore");
        let gitignore = gitignore_path
            .exists()
            .then(|| Gitignore::load(&gitignore_path));

        let mut results: Vec<String> = Vec::new();
        let grep_params = GrepParams {
            pattern,
            file_glob,
            case_sensitive,
            max_results,
        };
        grep_walk(
            search_path,
            search_path,
            &project_root,
            &grep_params,
            &mut results,
            gitignore.as_ref(),
        );

        if results.is_empty() {
            let mut msg = format!(
                "No matches for \"{}\" in {}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display()
            );
            if !has_regex_meta(pattern) {
                let suggestions = fuzzy_suggest(
                    search_path,
                    &project_root,
                    pattern,
                    file_glob,
                    case_sensitive,
                    gitignore.as_ref(),
                );
                if !suggestions.is_empty() {
                    msg.push_str(&format!(
                        ". Try grep again with one of: {}",
                        suggestions
                            .iter()
                            .map(|s| format!("\"{}\"", s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            msg
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display(),
                results.len(),
                results.join("\n")
            )
        }
    }
}

struct GrepParams<'a> {
    pattern: &'a str,
    file_glob: &'a str,
    case_sensitive: bool,
    max_results: usize,
}

/// Search a single file for a pattern and add results to the vector.
/// Returns the file content so callers can reuse it for fuzzy suggestions.
fn search_file_for_pattern(
    path: &Path,
    pattern: &str,
    case_sensitive: bool,
    max_results: usize,
    results: &mut Vec<String>,
) -> Option<String> {
    // Skip files larger than 1 MB.
    if let Ok(meta) = fsutil::metadata(path)
        && meta.len() > 1024 * 1024
    {
        return None;
    }

    let content = match fsutil::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Skip files that look binary.
    if content.contains('\0') {
        return Some(content);
    }

    let pattern_lower = if !case_sensitive {
        Some(pattern.to_lowercase())
    } else {
        None
    };

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    for (i, line) in content.lines().enumerate() {
        if results.len() >= max_results {
            break;
        }
        let (search_line, search_pattern) = if let Some(ref pl) = pattern_lower {
            (&line.to_lowercase() as &str, pl.as_str())
        } else {
            (line, pattern)
        };
        if autocode_core::helpers::matches_pattern(search_pattern, search_line, false) {
            results.push(format!("{}:{}: {}", file_name, i + 1, line));
        }
    }

    Some(content)
}

fn grep_walk(
    dir: &Path,
    search_root: &Path,
    project_root: &Path,
    params: &GrepParams,
    results: &mut Vec<String>,
    gitignore: Option<&Gitignore>,
) {
    walk_files(
        dir,
        search_root,
        project_root,
        params.file_glob,
        gitignore,
        &mut |_path: &Path, rel_raw: &str, content: &str| {
            if results.len() >= params.max_results {
                return;
            }

            let pattern_lower = if !params.case_sensitive {
                Some(params.pattern.to_lowercase())
            } else {
                None
            };

            for (i, line) in content.lines().enumerate() {
                if results.len() >= params.max_results {
                    break;
                }
                let (search_line, search_pattern) = if let Some(ref pl) = pattern_lower {
                    (&line.to_lowercase() as &str, pl.as_str())
                } else {
                    (line, params.pattern)
                };
                if autocode_core::helpers::matches_pattern(search_pattern, search_line, false) {
                    results.push(format!("{}:{}: {}", rel_raw, i + 1, line));
                }
            }
        },
    );
}
