use iced_video_player::Video;
use std::path::PathBuf;
use std::time::Instant;

use crate::state::{App, VideoInstance};

/// Load a video from a file path.
pub fn load_video_from_path(app: &mut App, video_path: PathBuf) {
    app.status = "Loading video...".to_string();

    match std::fs::metadata(&video_path) {
        Ok(_) => {
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
                let video_instance = VideoInstance {
                    id: app.next_id,
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
