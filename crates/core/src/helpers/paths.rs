/// Sentinel filenames used by `resolve_path` / `resolve_path_write` when
/// a path traversal attempt is blocked. The caller should detect these and
/// return a clear, actionable error rather than a generic "not found".
const READ_BLOCKED_SENTINEL: &str = "_path_traversal_blocked_";
const WRITE_BLOCKED_SENTINEL: &str = "_write_path_traversal_blocked_";

#[must_use]
pub fn blocked_error(raw_path: &str) -> String {
    format!(
        "{{\"error\":{},\"suggestion\":{}}}",
        serde_json::Value::String(format!(
            "Path traversal blocked for \"{raw_path}\" -- path escapes the project root"
        )),
        serde_json::Value::String("Use a path within the project directory".to_string()),
    )
}

fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut stack: Vec<std::path::Component<'_>> = Vec::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                if matches!(stack.last(), Some(std::path::Component::Normal(_))) {
                    stack.pop();
                }
            }
            std::path::Component::CurDir => {}
            other => stack.push(other),
        }
    }
    stack.into_iter().collect()
}

fn within_root(candidate: &std::path::Path, root: &std::path::Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

fn find_deepest_existing_ancestor(path: &std::path::Path) -> Option<std::path::PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).ok();
    }
    let mut a = path.parent();
    while let Some(p) = a {
        if p.exists() {
            return std::fs::canonicalize(p).ok();
        }
        a = p.parent();
    }
    None
}

/// Default maximum number of entries in the path cache.
const PATH_CACHE_MAX: usize = 500;

/// A single LRU cache for resolved file paths.
///
/// Uses a `HashMap` for O(1) lookups and a `VecDeque` to track insertion
/// order for proper least-recently-used eviction. When the capacity is
/// exceeded the oldest entry is removed.
///
/// This replaces the previous three separate caching mechanisms:
/// - `cache_insert` + `HashMap` (no real LRU eviction)
/// - `PathCacheTrait` + generic trait functions (unnecessary indirection)
/// - `PathCache` in `chat.rs` (duplicated LRU logic)
pub struct LruPathCache {
    map: std::collections::HashMap<String, std::path::PathBuf>,
    order: std::collections::VecDeque<String>,
    capacity: usize,
}

impl LruPathCache {
    pub fn new() -> Self {
        Self::with_capacity(PATH_CACHE_MAX)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    pub fn get(&self, key: &str) -> Option<&std::path::PathBuf> {
        self.map.get(key)
    }

    pub fn insert(&mut self, key: String, value: std::path::PathBuf) {
        // Replace existing entry without touching eviction or order.
        if self.map.remove(&key).is_some() {
            self.map.insert(key, value);
            return;
        }
        // Vacant: evict oldest if at capacity, then insert.
        if self.map.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.map.insert(key, value);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

impl Default for LruPathCache {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn resolve_path_cached(
    raw: &str,
    project_root: &str,
    cache: &mut LruPathCache,
    allow_escape: bool,
) -> std::path::PathBuf {
    let key = format!("r:{}:{}", project_root, raw);
    if let Some(p) = cache.get(&key) {
        return p.clone();
    }
    let p = resolve_path(raw, project_root, allow_escape);
    cache.insert(key, p.clone());
    p
}

#[must_use]
pub fn resolve_path_write_cached(
    raw: &str,
    project_root: &str,
    cache: &mut LruPathCache,
    allow_escape: bool,
) -> std::path::PathBuf {
    let key = format!("w:{}:{}", project_root, raw);
    if let Some(p) = cache.get(&key) {
        return p.clone();
    }
    let p = resolve_path_write(raw, project_root, allow_escape);
    cache.insert(key, p.clone());
    p
}

/// Check whether a path is blocked by traversal detection.
/// Returns true if the path contains `..` segments that would escape the project root.
#[must_use]
pub fn is_blocked_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == READ_BLOCKED_SENTINEL || n == WRITE_BLOCKED_SENTINEL)
}

/// Returns true if the path is safely within the project root (or is an
/// absolute path that the model explicitly requested — those are allowed
/// for reads but not for writes).
fn is_within_root(resolved: &std::path::Path, project_root: &str) -> bool {
    if let Ok(canonical_root) = std::fs::canonicalize(project_root) {
        let canonical_root = crate::utils::fsutil::display_path(&canonical_root);
        within_root(resolved, &canonical_root)
    } else {
        false
    }
}

pub fn resolve_path(raw: &str, project_root: &str, allow_escape: bool) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let (raw, project_root) = (
        &raw.replace('/', "\\") as &str,
        &project_root.replace('/', "\\") as &str,
    );
    let raw = raw.trim_end_matches(['.', '/', '\\']);
    let raw = if raw.is_empty() { "." } else { raw };
    let p = std::path::Path::new(raw);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::path::Path::new(project_root).join(p)
    };
    let resolved = std::fs::canonicalize(&joined)
        .map(|p| crate::utils::fsutil::display_path(&p))
        .unwrap_or_else(|_| crate::utils::fsutil::display_path(&joined));
    // Path traversal protection for relative paths:
    // If a relative path like "../../etc/passwd" was given, the canonicalized
    // result will escape the project root. We detect this and return the
    // project root instead (the tool execution layer will then get a
    // "not found" error, which is safer than silently accessing outside files).
    if !allow_escape && !p.is_absolute() {
        let resolved_path = std::path::Path::new(&resolved);
        if !is_within_root(resolved_path, project_root) {
            // Return the non-escaping original join target so the caller
            // gets a "file not found" rather than accessing outside files.
            return crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(
                &std::path::Path::new(project_root).join(READ_BLOCKED_SENTINEL),
            ));
        }
    }
    resolved
}

pub fn resolve_path_write(raw: &str, project_root: &str, allow_escape: bool) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let (raw, project_root) = (
        &raw.replace('/', "\\") as &str,
        &project_root.replace('/', "\\") as &str,
    );
    let raw = raw.trim_end_matches(['.', '/', '\\']);
    let raw = if raw.is_empty() { "." } else { raw };
    let p = std::path::Path::new(raw);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::path::Path::new(project_root).join(p)
    };

    // Determine the allowed root for Path-based containment checks.
    let root_for_comparison = if let Ok(canonical_root) = std::fs::canonicalize(project_root) {
        crate::utils::fsutil::display_path(&canonical_root)
    } else {
        crate::utils::fsutil::display_path(std::path::Path::new(project_root))
    };

    if !allow_escape {
        let check_target = find_deepest_existing_ancestor(&joined);

        if let Some(cp) = check_target {
            let cp = crate::utils::fsutil::display_path(&cp);
            if !within_root(&cp, &root_for_comparison) {
                return crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(
                    &std::path::Path::new(project_root).join(WRITE_BLOCKED_SENTINEL),
                ));
            }
        } else {
            let normalized = normalize_path(&joined);
            let normalized = crate::utils::fsutil::display_path(&normalized);
            if !within_root(&normalized, &root_for_comparison) {
                return crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(
                    &std::path::Path::new(project_root).join(WRITE_BLOCKED_SENTINEL),
                ));
            }
        }
    }

    if joined.exists() {
        std::fs::canonicalize(&joined)
            .map(|p| crate::utils::fsutil::display_path(&p))
            .unwrap_or_else(|_| crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(&joined)))
    } else {
        let parent = joined.parent();
        let filename = joined.file_name();
        match (parent, filename) {
            (Some(dir), Some(name)) => {
                let canonical_dir = if dir.exists() {
                    std::fs::canonicalize(dir)
                        .map(|p| crate::utils::fsutil::display_path(&p))
                        .unwrap_or_else(|_| {
                            crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(dir))
                        })
                } else {
                    crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(dir))
                };
                canonical_dir.join(name)
            }
            _ => crate::utils::fsutil::display_path(&crate::utils::fsutil::extended_path(&joined)),
        }
    }
}
