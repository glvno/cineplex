use iced_video_player::Video;
use std::time::Instant;
use tempfile::TempDir;

/// Represents a single video instance in the player.
pub struct VideoInstance {
    pub id: usize,
    pub video: Video,
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
    // Cached position to avoid expensive position queries on every frame
    pub cached_position: f64,
    pub last_position_query: Instant,
}

/// Application state containing all videos and UI state.
pub struct App {
    pub videos: Vec<VideoInstance>,
    pub next_id: usize,
    pub grid_columns: usize,
    pub error: Option<String>,
    pub status: String,
}

impl Default for App {
    fn default() -> Self {
        App {
            videos: Vec::new(),
            next_id: 0,
            grid_columns: 2, // Default to 2 columns
            error: None,
            status: "Drop video files here to load them".to_string(),
        }
    }
}
