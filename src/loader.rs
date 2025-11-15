use iced_video_player::Video;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::state::{App, VideoInstance};

// Global lock to prevent simultaneous GStreamer pipeline initialization
// which causes FLUSH_START event deadlocks when loading multiple videos
static GSTREAMER_INIT_LOCK: std::sync::OnceLock<Arc<Mutex<()>>> = std::sync::OnceLock::new();

fn get_gstreamer_lock() -> Arc<Mutex<()>> {
    GSTREAMER_INIT_LOCK
        .get_or_init(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Load a video from a file path.
pub fn load_video_from_path(app: &mut App, video_path: PathBuf) {
    app.status = "Loading video...".to_string();

    match std::fs::metadata(&video_path) {
        Ok(_) => {
            // Acquire lock to prevent simultaneous GStreamer pipeline initialization
            let lock = get_gstreamer_lock();
            let _guard = lock.lock().unwrap();
            load_direct_video(app, &video_path);
        }
        Err(e) => {
            app.error = Some(format!("Video file not found: {}", e));
        }
    }
}

/// Load a video directly without conversion.
pub fn load_direct_video(app: &mut App, video_path: &PathBuf) {
    match url::Url::from_file_path(&video_path) {
        Ok(url) => match Video::new(&url) {
            Ok(mut video) => {
                video.set_looping(true);
                let native_fps = video.framerate();
                let now = Instant::now();
                let video_id = app.next_id;
                let video_instance = VideoInstance {
                    id: video_id,
                    video,
                    path: video_path.clone(),
                    position: 0.0,
                    dragging: false,
                    hovered: false,
                    looping_enabled: true,
                    fullscreen: false,
                    _temp_dir: None,
                    frame_count: 0,
                    last_fps_time: now,
                    current_fps: 0.0,
                    native_fps,
                    last_ui_update: now,
                    pending_position_update: false,
                };
                log::info!("Video loaded: id={}, path={}, fps={}, total_videos={}",
                          video_id, video_path.display(), native_fps, app.videos.len() + 1);
                app.videos.push(video_instance);
                app.next_id += 1;
                app.error = None;
                app.status = format!(
                    "Video loaded: {}",
                    video_path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            Err(e) => {
                app.error = Some(format!("Failed to load video: {}", e));
            }
        },
        Err(_) => {
            app.error = Some("Invalid video path".to_string());
        }
    }
}
