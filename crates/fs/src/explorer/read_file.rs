// read_file.rs -- Read file contents with size limit.

use std::path::Path;

use autocode_core::utils::fsutil;

/// Read the contents of a file as a String (up to 512 KB).
pub fn read_file(path: &Path) -> Result<String, String> {
    let meta = fsutil::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 512 * 1024 {
        return Err(format!(
            "File too large to display (> 512 KB): {}",
            path.display()
        ));
    }
    fsutil::read_to_string(path).map_err(|e| e.to_string())
}
