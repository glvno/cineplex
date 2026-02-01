use iced::widget::image::Handle;
use iced_video_player::Video;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Instant;

use crate::state::{App, MediaItem, PhotoInstance, VideoInstance};

/// Supported video extensions (case-insensitive check performed separately).
const VIDEO_EXTENSIONS: &[&str] = &["mov", "mp4", "m4v", "mkv", "avi", "webm"];

/// Supported image extensions (case-insensitive check performed separately).
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "tif"];

/// Determine if a path is a video file.
fn is_video_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Determine if a path is an image file.
fn is_image_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Load a media file (video or photo) from a file path.
pub fn load_media_from_path(app: &mut App, path: PathBuf) {
    app.status = "Loading...".to_string();

    match std::fs::metadata(&path) {
        Ok(_) => {
            if is_video_file(&path) {
                load_direct_video(app, &path);
            } else if is_image_file(&path) {
                load_photo(app, &path);
            } else {
                app.error = Some(format!(
                    "Unsupported file type: {}",
                    path.extension()
                        .and_then(OsStr::to_str)
                        .unwrap_or("unknown")
                ));
            }
        }
        Err(e) => {
            app.error = Some(format!("File not found: {}", e));
        }
    }
}

/// Load a photo from a file path.
fn load_photo(app: &mut App, photo_path: &PathBuf) {
    let photo_id = app.next_id;
    let filename = photo_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let handle = Handle::from_path(photo_path);

    let photo_instance = PhotoInstance {
        id: photo_id,
        handle,
        hovered: false,
        fullscreen: false,
        filename: filename.clone(),
    };

    log::info!(
        "Photo loaded: id={}, path={}, total_media={}",
        photo_id,
        photo_path.display(),
        app.media.len() + 1
    );

    app.media.push(MediaItem::Photo(photo_instance));
    app.next_id += 1;
    app.error = None;
    app.status = format!("Photo loaded: {}", filename);
}

/// Load a video directly without conversion.
fn load_direct_video(app: &mut App, video_path: &PathBuf) {
    match url::Url::from_file_path(video_path) {
        Ok(url) => match Video::new(&url) {
            Ok(mut video) => {
                video.set_muted(true);
                video.set_volume(0.0);
                video.set_looping(true);
                let native_fps = video.framerate();
                let now = Instant::now();
                let video_id = app.next_id;

                let video_instance = VideoInstance {
                    id: video_id,
                    video,
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
                log::info!(
                    "Video loaded: id={}, path={}, fps={}, total_media={}",
                    video_id,
                    video_path.display(),
                    native_fps,
                    app.media.len() + 1
                );
                app.media.push(MediaItem::Video(video_instance));
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
