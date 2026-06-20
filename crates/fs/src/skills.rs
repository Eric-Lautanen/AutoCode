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
                let description = extract_heading(&path).unwrap_or_default();
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

/// Read only the first 2 KiB to grab the heading — avoids slurping the whole file.
fn extract_heading(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file).take(2048);
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim().to_string();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return Some(heading.to_string());
        }
    }
    None
}

pub fn read_skill(dir: &Path, name: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(dir.join(format!("{}.md", name)))
}
