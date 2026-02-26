use iced::widget::image::Handle;
use iced_video_player::Video;
use std::sync::mpsc;
use std::time::Instant;
use tempfile::TempDir;

/// Represents a single video instance in the player.
pub struct VideoInstance {
    pub id: usize,
    pub video: Video,
    pub position: f64,
    pub duration: f64, // Cached duration to avoid blocking GStreamer queries during rendering
    pub dragging: bool,
    pub was_paused_before_drag: bool,
    pub hovered: bool,
    // Cached GStreamer state (NEVER query video.paused/looping/muted on main thread - use these!)
    pub is_paused: bool,
    pub is_looping: bool,
    pub is_muted: bool,
    pub fullscreen: bool,
    pub _temp_dir: Option<TempDir>,
    pub native_fps: f64, // Native framerate of the video
    // UI fade tracking
    pub last_mouse_activity: Instant,
    // Stall detection: tracks when position last *changed* value, not just when queried
    pub last_position_update: Instant,
    pub last_position_value: f64,
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
    pub position_thread_rx: Option<mpsc::Receiver<crate::position_thread::PositionUpdate>>,
    pub stall_check_counter: u32,
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
            position_thread_rx: None,
            stall_check_counter: 0,
        }
    }
}
