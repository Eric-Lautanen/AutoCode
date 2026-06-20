use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::OnceLock;

pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

/// Find the skills directory by trying several locations in order.
/// Returns the first valid one, falling back to `project_root/skills` as default.
pub fn skills_dir(project_root: &Path) -> std::path::PathBuf {
    // 1. project_root/skills
    if project_root.as_os_str().is_empty() {
        let from_cwd = std::env::current_dir().ok().map(|d| d.join("skills"));
        if from_cwd.as_ref().is_some_and(|d| d.is_dir()) {
            return from_cwd.unwrap();
        }
    } else {
        let candidate = project_root.join("skills");
        if candidate.is_dir() {
            return candidate;
        }
    }
    // 2. CWD/skills
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("skills");
        if candidate.is_dir() {
            return candidate;
        }
    }
    // 3. exe/skills
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("skills");
        if candidate.is_dir() {
            return candidate;
        }
    }
    // Default: project_root/skills (even if it doesn't exist)
    project_root.join("skills")
}

pub fn list_skill_names(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names
}

pub fn list_skills_with_info(dir: &Path) -> &Vec<SkillInfo> {
    static CACHE: OnceLock<Vec<SkillInfo>> = OnceLock::new();
    CACHE.get_or_init(|| scan_skills(dir))
}

fn scan_skills(dir: &Path) -> Vec<SkillInfo> {
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                let description = extract_description(&path).unwrap_or_default();
                skills.push(SkillInfo {
                    name: stem.to_string(),
                    description,
                });
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Read the first 2 KB of a skill file and return a description:
/// 1. The YAML frontmatter `description` field (single-line or folded `>`), or
/// 2. The first `# Heading` if no frontmatter description exists.
fn extract_description(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file).take(2048);
    let mut lines = reader.lines().map_while(Result::ok).peekable();

    // Try YAML frontmatter
    let desc = lines
        .next_if(|l| l.trim() == "---")
        .and_then(|_| parse_yaml_description(&mut lines));
    if let Some(d) = desc {
        return Some(d);
    }

    // Fallback: first # Heading
    for line in lines {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return Some(heading.to_string());
        }
    }
    None
}

fn parse_yaml_description(
    lines: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> Option<String> {
    let mut desc = None;

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed == "---" {
            break;
        }

        if let Some(val) = trimmed.strip_prefix("description:") {
            let rest = val.trim();
            if rest.is_empty() || rest == ">" || rest == "|" {
                // Block scalar — collect subsequent indented lines
                let block_indent = line.len() - line.trim_start().len();
                let mut block = String::new();
                while let Some(peek) = lines.peek() {
                    let peek_indent = peek.len() - peek.trim_start().len();
                    if peek.trim().is_empty() {
                        lines.next();
                        block.push(' ');
                        continue;
                    }
                    if peek_indent <= block_indent {
                        break;
                    }
                    let cl = lines.next().unwrap();
                    if !block.is_empty() {
                        block.push(' ');
                    }
                    block.push_str(cl.trim());
                }
                if !block.is_empty() {
                    desc = Some(block);
                }
            } else {
                desc = Some(rest.to_string());
            }
            // Found description — skip remaining frontmatter
            for skip in lines.by_ref() {
                if skip.trim() == "---" {
                    break;
                }
            }
            break;
        }
    }

    desc
}

pub fn read_skill(dir: &Path, name: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(dir.join(format!("{}.md", name)))
}
