use iced::widget::image::Handle;
use iced_video_player::Video;
use std::time::Instant;
use tempfile::TempDir;

/// Represents a single video instance in the player.
pub struct VideoInstance {
    pub id: usize,
    pub video: Video,
    pub position: f64,
    pub dragging: bool,
    pub was_paused_before_drag: bool,
    pub hovered: bool,
    pub looping_enabled: bool,
    pub fullscreen: bool,
    pub _temp_dir: Option<TempDir>,
    // Framerate monitoring
    pub frame_count: u64,
    pub last_fps_time: Instant,
    pub current_fps: f64,
    pub native_fps: f64, // Native framerate of the video
    // UI fade tracking
    pub last_mouse_activity: Instant,
}

/// Represents a single photo instance in the player.
pub struct PhotoInstance {
    pub id: usize,
    pub handle: Handle,
    pub hovered: bool,
    pub fullscreen: bool,
    pub filename: String,
    // UI fade tracking
    pub last_mouse_activity: Instant,
}

/// Unified media item that can be either a video or a photo.
pub enum MediaItem {
    Video(VideoInstance),
    Photo(PhotoInstance),
}

impl MediaItem {
    pub fn id(&self) -> usize {
        match self {
            MediaItem::Video(v) => v.id,
            MediaItem::Photo(p) => p.id,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        match self {
            MediaItem::Video(v) => v.fullscreen,
            MediaItem::Photo(p) => p.fullscreen,
        }
    }
}

/// Application state containing all media and UI state.
pub struct App {
    pub media: Vec<MediaItem>,
    pub next_id: usize,
    pub grid_columns: usize,
    pub error: Option<String>,
    pub status: String,
    pub watchdog: crate::watchdog::Watchdog,
}

impl Default for App {
    fn default() -> Self {
        App {
            media: Vec::new(),
            next_id: 0,
            grid_columns: 2, // Default to 2 columns
            error: None,
            status: "Drop media files here to load them".to_string(),
            watchdog: crate::watchdog::Watchdog::spawn(),
        }
    }
}
