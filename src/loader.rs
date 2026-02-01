use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use iced::widget::image::Handle;
use iced_video_player::Video;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Instant;

use crate::state::{App, MediaItem, PhotoInstance, VideoInstance};

/// Supported video extensions (case-insensitive check performed separately).
const VIDEO_EXTENSIONS: &[&str] = &["mov", "mp4", "m4v", "mkv", "avi", "webm"];

/// Supported image extensions (case-insensitive check performed separately).
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "tif", "heic", "heif"];

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

/// Load a photo from a file path with proper EXIF orientation handling.
fn load_photo(app: &mut App, photo_path: &PathBuf) {
    let photo_id = app.next_id;
    let filename = photo_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Load image with EXIF orientation support
    let handle = match load_image_with_orientation(photo_path) {
        Ok(h) => h,
        Err(e) => {
            app.error = Some(format!("Failed to load image: {}", e));
            return;
        }
    };

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

/// Load an image file and apply EXIF orientation correction.
fn load_image_with_orientation(path: &PathBuf) -> Result<Handle, Box<dyn std::error::Error>> {
    let mut decoder = ImageReader::open(path)?.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.into_raw();

    Ok(Handle::from_rgba(width, height, pixels))
}

/// Load a video with automatic rotation correction from metadata.
fn load_direct_video(app: &mut App, video_path: &PathBuf) {
    let url = match url::Url::from_file_path(video_path) {
        Ok(u) => u,
        Err(_) => {
            app.error = Some("Invalid video path".to_string());
            return;
        }
    };

    // Create pipeline with videoflip for automatic rotation based on metadata
    let pipeline_str = format!(
        "playbin uri=\"{}\" \
         video-sink=\"videoflip method=automatic ! videoscale ! videoconvert ! \
         appsink name=iced_video drop=true caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
        url.as_str()
    );

    let video = match create_video_from_pipeline(&pipeline_str) {
        Ok(v) => v,
        Err(e) => {
            app.error = Some(format!("Failed to load video: {}", e));
            return;
        }
    };

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

/// Create a Video from a custom GStreamer pipeline string.
fn create_video_from_pipeline(pipeline_str: &str) -> Result<Video, Box<dyn std::error::Error>> {
    gst::init()?;

    let pipeline = gst::parse::launch(pipeline_str)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "Failed to cast to Pipeline")?;

    // Get the video sink from playbin and extract the appsink
    let video_sink: gst::Element = pipeline.property("video-sink");
    let pad = video_sink.pads().first().cloned().ok_or("No pads found")?;
    let pad = pad
        .dynamic_cast::<gst::GhostPad>()
        .map_err(|_| "Failed to cast to GhostPad")?;
    let bin = pad
        .parent_element()
        .ok_or("No parent element")?
        .downcast::<gst::Bin>()
        .map_err(|_| "Failed to cast to Bin")?;
    let video_sink = bin
        .by_name("iced_video")
        .ok_or("Could not find iced_video appsink")?;
    let video_sink = video_sink
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "Failed to cast to AppSink")?;

    let mut video = Video::from_gst_pipeline(pipeline, video_sink, None)?;
    video.set_muted(true);
    video.set_volume(0.0);
    video.set_looping(true);

    Ok(video)
}
