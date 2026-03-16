use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use iced::widget::image::Handle;
use iced_video_player::Video;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crate::state::{LoadResult, PhotoInstance, VideoInstance};

/// Supported video extensions (case-insensitive check performed separately).
const VIDEO_EXTENSIONS: &[&str] = &["mov", "mp4", "m4v", "mkv", "avi", "webm"];

/// Supported image extensions (case-insensitive check performed separately).
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "tif", "heic", "heif",
];

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

/// Check if a path is a supported media file (video or image).
pub fn is_supported_media_file(path: &PathBuf) -> bool {
    is_video_file(path) || is_image_file(path)
}

/// Spawn a background thread to load a media file asynchronously.
/// Results are sent back via the provided channel.
pub fn load_media_async(tx: mpsc::Sender<LoadResult>, path: PathBuf, id: usize) {
    std::thread::Builder::new()
        .name(format!("media-loader-{}", id))
        .spawn(move || {
            let result = if is_video_file(&path) {
                load_video_on_thread(&path, id)
            } else if is_image_file(&path) {
                load_photo_on_thread(&path, id)
            } else {
                LoadResult::Error(format!(
                    "Unsupported file type: {}",
                    path.extension()
                        .and_then(OsStr::to_str)
                        .unwrap_or("unknown")
                ))
            };
            let _ = tx.send(result);
        })
        .expect("Failed to spawn media loader thread");
}

/// Load a video on a background thread, returning a LoadResult.
fn load_video_on_thread(video_path: &PathBuf, video_id: usize) -> LoadResult {
    let url = match url::Url::from_file_path(video_path) {
        Ok(u) => u,
        Err(_) => return LoadResult::Error("Invalid video path".to_string()),
    };

    // Create pipeline with videoflip for automatic rotation based on metadata
    // videorate ensures a fixed framerate (needed for VFR content that reports 0 fps)
    // audio-sink=fakesink prevents CoreAudio mutex contention for muted videos.
    // When the user unmutes, the audio sink is swapped to autoaudiosink in app.rs.
    let pipeline_str = format!(
        "playbin uri=\"{}\" audio-sink=fakesink \
         video-sink=\"videoflip method=automatic ! videorate ! video/x-raw,framerate=30/1 ! \
         videoscale ! videoconvert ! \
         appsink name=iced_video drop=true caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
        url.as_str()
    );

    let video = match create_video_from_pipeline(&pipeline_str) {
        Ok(v) => v,
        Err(e) => return LoadResult::Error(format!("Failed to load video: {}", e)),
    };

    let native_fps = video.framerate();
    let duration = {
        let raw_duration = video.duration().as_secs_f64();
        log::info!(
            "Duration query for {}: raw_duration={:.2}s, is_finite={}",
            video_path.file_name().unwrap_or_default().to_string_lossy(),
            raw_duration,
            raw_duration.is_finite()
        );
        if raw_duration.is_finite() && raw_duration > 0.0 {
            raw_duration
        } else {
            log::warn!(
                "Invalid duration for {}, defaulting to 1.0s",
                video_path.file_name().unwrap_or_default().to_string_lossy()
            );
            1.0
        }
    };
    let now = Instant::now();

    let video_instance = VideoInstance {
        id: video_id,
        video,
        position: 0.0,
        duration,
        dragging: false,
        was_paused_before_drag: false,
        hovered: false,
        is_paused: false,
        is_looping: true,
        is_muted: true,
        fullscreen: false,
        _temp_dir: None,
        native_fps,
        last_mouse_activity: now,
        last_position_update: now,
        last_position_value: 0.0,
    };

    crate::gst_logger::log_video_created(video_id, &video_path.display().to_string());
    log::info!(
        "Video loaded (async): id={}, path={}, fps={}",
        video_id,
        video_path.display(),
        native_fps,
    );

    LoadResult::Video(video_instance)
}

/// Load a photo on a background thread, returning a LoadResult.
fn load_photo_on_thread(photo_path: &PathBuf, photo_id: usize) -> LoadResult {
    let filename = photo_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let handle = match load_image_with_orientation(photo_path) {
        Ok(h) => h,
        Err(e) => return LoadResult::Error(format!("Failed to load image: {}", e)),
    };

    let photo_instance = PhotoInstance {
        id: photo_id,
        handle,
        hovered: false,
        fullscreen: false,
        filename: filename.clone(),
        last_mouse_activity: Instant::now(),
    };

    log::info!(
        "Photo loaded (async): id={}, path={}",
        photo_id,
        photo_path.display(),
    );

    LoadResult::Photo(photo_instance)
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

    // Set mute/volume on the playbin BEFORE from_gst_pipeline starts playback,
    // otherwise audio briefly plays when loading many videos at once.
    pipeline.set_property("mute", true);
    pipeline.set_property("volume", 0.0f64);

    let mut video = Video::from_gst_pipeline(pipeline, video_sink, None)?;
    video.set_muted(true);
    video.set_volume(0.0);
    video.set_looping(true);

    Ok(video)
}
