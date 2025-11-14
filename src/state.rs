use iced_video_player::Video;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;

use crate::cache;

/// Represents a single video instance in the player.
pub struct VideoInstance {
    pub id: usize,
    pub video: Video,
    pub path: PathBuf,
    pub position: f64,
    pub dragging: bool,
    pub hovered: bool,
    pub looping_enabled: bool,
    pub fullscreen: bool,
    pub _temp_dir: Option<TempDir>,
    // Framerate monitoring
    pub frame_count: u64,
    pub last_fps_time: Instant,
    pub current_fps: f64,
    pub native_fps: f64, // Native framerate of the video
    // Frame throttling for UI updates (max 30 FPS UI refreshes)
    pub last_ui_update: Instant,
    pub pending_position_update: bool,
}

/// Application state containing all videos and UI state.
pub struct App {
    pub videos: Vec<VideoInstance>,
    pub next_id: usize,
    pub grid_columns: usize,
    pub error: Option<String>,
    pub status: String,
    // Conversion state tracking
    pub converting: HashMap<PathBuf, usize>,        // original_path -> video_id (currently converting)
    pub conversion_cache: HashMap<PathBuf, PathBuf>, // original_path -> converted_path (cache hits)
}

impl Default for App {
    fn default() -> Self {
        let mut app = App {
            videos: Vec::new(),
            next_id: 0,
            grid_columns: 2, // Default to 2 columns
            error: None,
            status: "Drop video files here to load them".to_string(),
            converting: HashMap::new(),
            conversion_cache: HashMap::new(),
        };

        // Load persistent cache from disk on startup
        cache::load_persistent_cache(&mut app.conversion_cache);

        app
    }
}
