use std::path::{Path, PathBuf};

pub fn extended_path(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let s = path.to_string_lossy();
        // Strip trailing slashes -- \\?\ paths must not end with a separator
        // or ReadDir/metadata will hang with "permission denied or invalid path".
        let s = s.trim_end_matches(['/', '\\']);
        if s.starts_with(r"\\?\") {
            return PathBuf::from(s);
        }
        if let Some(stripped) = s.strip_prefix(r"\\") {
            return PathBuf::from(format!(r"\\?\UNC\{}", stripped));
        }
        let base = PathBuf::from(s);
        let abs = if base.is_absolute() {
            base
        } else {
            std::env::current_dir().unwrap_or_default().join(base)
        };
        // Canonicalize to resolve the true on-disk casing.
        // \\?\ paths are case-sensitive on Windows, so "c:\github\autocode"
        // must become "C:\github\AutoCode" before we prepend the prefix.
        // canonicalize() returns a \\?\ path itself on Windows, so use it
        // directly when it succeeds; fall back for paths not yet on disk.
        if let Ok(canonical) = std::fs::canonicalize(&abs) {
            return canonical;
        }
        PathBuf::from(format!(r"\\?\{}", abs.to_string_lossy()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

pub fn read_to_string(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(extended_path(path))
}
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    std::fs::write(extended_path(path), contents)
}
pub fn metadata(path: &Path) -> std::io::Result<std::fs::Metadata> {
    std::fs::metadata(extended_path(path))
}
pub fn read_dir(path: &Path) -> std::io::Result<std::fs::ReadDir> {
    std::fs::read_dir(extended_path(path))
}
pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(extended_path(path))
}
pub fn remove_file(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(extended_path(path))
}
pub fn remove_dir(path: &Path) -> std::io::Result<()> {
    std::fs::remove_dir(extended_path(path))
}
pub fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(extended_path(from), extended_path(to))
}
pub fn is_dir(path: &Path) -> bool {
    extended_path(path).is_dir()
}

pub fn display_path(path: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{}", stripped))
        } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            path.to_path_buf()
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

#[cfg(target_os = "windows")]
pub fn write_cmd_script(path: &Path, content: &str) -> std::io::Result<()> {
    let mut bytes = Vec::with_capacity(content.len() + 64);
    bytes.extend_from_slice(b"@echo off\r\n");
    bytes.extend_from_slice(content.as_bytes());
    if !content.ends_with('\n') && !content.ends_with('\r') {
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"exit /b %errorlevel%\r\n");
    write(path, &bytes)
}

#[cfg(not(target_os = "windows"))]
pub fn write_cmd_script(path: &Path, content: &str) -> std::io::Result<()> {
    write(path, content)
}
