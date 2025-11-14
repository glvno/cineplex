use iced_video_player::Video;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

use crate::codec;
use crate::state::{App, VideoInstance};

/// Load a video from a file path, handling conversion if necessary.
pub fn load_video_from_path(app: &mut App, video_path: PathBuf) {
    eprintln!("load_video_from_path called for: {:?}", video_path);
    app.status = "Loading video...".to_string();

    match std::fs::metadata(&video_path) {
        Ok(_) => {
            // Check if video needs conversion based on codec
            let should_convert = codec::should_convert(&video_path);
            eprintln!("should_convert result: {}", should_convert);
            if should_convert {
                eprintln!("Video needs conversion, checking cache...");

                // Check if we already converted this file
                if let Some(converted_path) = app.conversion_cache.get(&video_path) {
                    eprintln!("Cache hit! Using converted file");
                    load_converted_video(app, &video_path, converted_path.clone());
                } else if !app.converting.contains_key(&video_path) {
                    // Not already converting, start background conversion
                    eprintln!("Starting background VP9 conversion");
                    let video_id = app.next_id;
                    app.converting.insert(video_path.clone(), video_id);
                    app.status = format!(
                        "Converting video ({})...",
                        video_path.file_name().unwrap_or_default().to_string_lossy()
                    );

                    // Spawn background conversion thread
                    let original_path = video_path.clone();
                    thread::spawn(move || {
                        codec::convert_video_background(&original_path, video_id);
                    });
                } else {
                    // Already converting this file
                    app.status = format!(
                        "Video already converting... ({})",
                        video_path.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
            } else {
                eprintln!("Video will be loaded directly");
                load_direct_video(app, &video_path);
            }
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

/// Load a video that has been converted to a different format.
pub fn load_converted_video(app: &mut App, original_path: &PathBuf, converted_path: PathBuf) {
    match url::Url::from_file_path(&converted_path) {
        Ok(url) => match Video::new(&url) {
            Ok(mut video) => {
                video.set_looping(true);
                let native_fps = video.framerate();
                let now = Instant::now();
                let video_instance = VideoInstance {
                    id: app.next_id,
                    video,
                    path: original_path.clone(),
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
                    "Video loaded (converted): {}",
                    original_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
            }
            Err(e) => {
                app.error = Some(format!("Failed to load converted video: {}", e));
            }
        },
        Err(_) => {
            app.error = Some("Invalid converted video path".to_string());
        }
    }
}

/// Check for completed background conversions and load converted videos.
pub fn check_for_completed_conversions(app: &mut App) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let cache_dir = match std::env::var("HOME") {
        Ok(home) => std::path::PathBuf::from(home).join(".cineplex_cache"),
        Err(_) => return,
    };

    // Check each video that's being converted
    let paths_to_check: Vec<_> = app.converting.keys().cloned().collect();

    for original_path in paths_to_check {
        // Compute the marker file path
        let mut hasher = DefaultHasher::new();
        original_path.hash(&mut hasher);
        let hash = hasher.finish();
        let marker_path = cache_dir.join(format!("converted_{:x}.webm.done", hash));
        let converted_path = cache_dir.join(format!("converted_{:x}.webm", hash));

        // If the marker file exists, conversion is complete and file is ready
        if marker_path.exists() {
            eprintln!("Conversion completed for {:?}", original_path);
            app.converting.remove(&original_path);
            app.conversion_cache
                .insert(original_path.clone(), converted_path.clone());
            // Save the updated cache to persistent storage
            crate::cache::save_cache_metadata(&app.conversion_cache);
            load_converted_video(app, &original_path, converted_path);
            // Clean up marker file
            let _ = std::fs::remove_file(&marker_path);
        }
    }
}
