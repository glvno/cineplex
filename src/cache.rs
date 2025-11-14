use std::collections::HashMap;
use std::path::PathBuf;

/// Get the path to cache metadata file.
fn get_cache_metadata_path() -> Option<PathBuf> {
    match std::env::var("HOME") {
        Ok(home) => Some(
            PathBuf::from(home)
                .join(".cineplex_cache")
                .join("cache_metadata.json"),
        ),
        Err(_) => None,
    }
}

/// Load cached conversion entries from persistent storage.
pub fn load_persistent_cache(cache: &mut HashMap<PathBuf, PathBuf>) {
    if let Some(metadata_path) = get_cache_metadata_path() {
        if let Ok(content) = std::fs::read_to_string(&metadata_path) {
            if let Ok(entries) = serde_json::from_str::<Vec<(String, String)>>(&content) {
                for (original_str, converted_str) in entries {
                    let original_path = PathBuf::from(&original_str);
                    let converted_path = PathBuf::from(&converted_str);

                    // Only add to cache if the converted file still exists
                    if converted_path.exists() {
                        cache.insert(original_path, converted_path);
                    }
                }
                let count = cache.len();
                eprintln!("Loaded {} cached conversions from persistent storage", count);
            }
        }
    }
}

/// Save cache metadata to persistent storage.
pub fn save_cache_metadata(cache: &HashMap<PathBuf, PathBuf>) {
    if let Ok(home) = std::env::var("HOME") {
        let cache_dir = PathBuf::from(home).join(".cineplex_cache");
        let _ = std::fs::create_dir_all(&cache_dir);

        if let Some(metadata_path) = get_cache_metadata_path() {
            let entries: Vec<(String, String)> = cache
                .iter()
                .map(|(k, v)| (k.to_string_lossy().to_string(), v.to_string_lossy().to_string()))
                .collect();

            if let Ok(json) = serde_json::to_string(&entries) {
                let _ = std::fs::write(&metadata_path, json);
            }
        }
    }
}

/// Clear all cached conversions from memory and disk.
pub fn clear_cache() -> String {
    // Clear cache directory
    if let Ok(home) = std::env::var("HOME") {
        let cache_dir = PathBuf::from(home).join(".cineplex_cache");
        let _ = std::fs::remove_dir_all(&cache_dir);
        eprintln!("Cache cleared successfully");
        return "Cache cleared".to_string();
    }
    "Failed to clear cache".to_string()
}

/// Get the cache directory path.
pub fn get_cache_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".cineplex_cache"))
}
