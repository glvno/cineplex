use std::path::PathBuf;

/// Clear all cached conversions from disk.
pub fn clear_cache() -> String {
    // Clear cache directory
    if let Ok(home) = std::env::var("HOME") {
        let cache_dir = PathBuf::from(home).join(".cineplex_cache");
        let _ = std::fs::remove_dir_all(&cache_dir);
        return "Cache cleared".to_string();
    }
    "Failed to clear cache".to_string()
}
