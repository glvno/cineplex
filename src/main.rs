use iced::event;
use iced::widget::text::Shaping;
use iced::widget::{button, center, column, container, mouse_area, row, slider, stack, text};
use iced::{Color, Element, Length, Subscription, Theme, alignment};
use iced_video_player::{Video, VideoPlayer};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

fn main() -> iced::Result {
    iced::application("Cineplex", App::update, App::view)
        .subscription(App::subscription)
        .run()
}

#[derive(Clone, Debug)]
enum Message {
    TogglePause(usize),
    ToggleLoop(usize),
    Seek(usize, f64),
    SeekRelease(usize),
    EndOfStream(usize),
    NewFrame(usize),
    RemoveVideo(usize),
    VideoHoverChanged(usize, bool),
    ToggleMute(usize),
    ToggleFullscreen(usize),
    IncreaseColumns,
    DecreaseColumns,
    BrowseFile,
    ClearCache,
    FileDropped(PathBuf),
    EventOccurred(iced::Event),
    ConversionStarted(PathBuf, usize),
    ConversionComplete(PathBuf, PathBuf, usize), // original, converted, video_id
    ConversionFailed(PathBuf, String, usize),
}

struct VideoInstance {
    id: usize,
    video: Video,
    path: PathBuf,
    position: f64,
    dragging: bool,
    hovered: bool,
    looping_enabled: bool,
    fullscreen: bool,
    _temp_dir: Option<TempDir>,
    // Framerate monitoring
    frame_count: u64,
    last_fps_time: std::time::Instant,
    current_fps: f64,
    native_fps: f64, // Native framerate of the video
    // Frame throttling for UI updates (max 30 FPS UI refreshes)
    last_ui_update: std::time::Instant,
    pending_position_update: bool,
}

struct App {
    videos: Vec<VideoInstance>,
    next_id: usize,
    grid_columns: usize,
    error: Option<String>,
    status: String,
    // Conversion state tracking
    converting: HashMap<PathBuf, usize>, // original_path -> video_id (currently converting)
    conversion_cache: HashMap<PathBuf, PathBuf>, // original_path -> converted_path (cache hits)
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
        app.load_persistent_cache();

        app
    }
}

impl App {
    fn get_cache_metadata_path() -> Option<PathBuf> {
        match std::env::var("HOME") {
            Ok(home) => Some(PathBuf::from(home).join(".cineplex_cache").join("cache_metadata.json")),
            Err(_) => None,
        }
    }

    fn load_persistent_cache(&mut self) {
        if let Some(metadata_path) = Self::get_cache_metadata_path() {
            if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                if let Ok(entries) = serde_json::from_str::<Vec<(String, String)>>(&content) {
                    for (original_str, converted_str) in entries {
                        let original_path = PathBuf::from(&original_str);
                        let converted_path = PathBuf::from(&converted_str);

                        // Only add to cache if the converted file still exists
                        if converted_path.exists() {
                            self.conversion_cache.insert(original_path, converted_path);
                        }
                    }
                    let count = self.conversion_cache.len();
                    eprintln!("Loaded {} cached conversions from persistent storage", count);
                }
            }
        }
    }

    fn save_cache_metadata(&self) {
        if let Some(cache_dir) = std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".cineplex_cache")) {
            let _ = std::fs::create_dir_all(&cache_dir);

            if let Some(metadata_path) = Self::get_cache_metadata_path() {
                let entries: Vec<(String, String)> = self.conversion_cache
                    .iter()
                    .map(|(k, v)| (k.to_string_lossy().to_string(), v.to_string_lossy().to_string()))
                    .collect();

                if let Ok(json) = serde_json::to_string(&entries) {
                    let _ = std::fs::write(&metadata_path, json);
                }
            }
        }
    }

    fn clear_cache_impl(&mut self) {
        // Clear in-memory cache
        self.conversion_cache.clear();

        // Delete cache directory
        if let Ok(home) = std::env::var("HOME") {
            let cache_dir = PathBuf::from(home).join(".cineplex_cache");
            let _ = std::fs::remove_dir_all(&cache_dir);
            eprintln!("Cache cleared successfully");
            self.status = "Cache cleared".to_string();
        }
    }

    fn get_video_codec(path: &Path) -> Option<String> {
        // Use ffprobe to detect the codec
        // Note: This runs on the UI thread, so it may cause brief hangs for large files
        eprintln!("Running ffprobe to detect codec...");
        let start = std::time::Instant::now();

        let result = match std::process::Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=codec_name")
            .arg("-of")
            .arg("default=noprint_wrappers=1")
            .arg(path)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    stdout
                        .lines()
                        .find(|line| line.starts_with("codec_name="))
                        .map(|line| line.trim_start_matches("codec_name=").to_string())
                } else {
                    eprintln!("ffprobe failed for {:?}", path);
                    None
                }
            }
            Err(e) => {
                eprintln!("Failed to run ffprobe: {}", e);
                None
            }
        };

        let elapsed = start.elapsed();
        eprintln!(
            "ffprobe took {:.2}s, result: {:?}",
            elapsed.as_secs_f64(),
            result
        );
        result
    }

    fn should_convert(path: &Path) -> bool {
        // Only convert H.264 files and MOV files (which often contain H.264)
        // VP9, AV1, and other modern codecs typically decode to proper NV12
        match Self::get_video_codec(path) {
            Some(codec) => {
                eprintln!("Detected codec: {}", codec);
                // Convert H.264 and files we know have issues
                matches!(codec.as_str(), "h264" | "mpeg2video")
            }
            None => {
                // Fallback: if codec detection fails, convert MOV and MP4 files as a safety measure
                // (they're more likely to have codec issues)
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let should_fallback_convert =
                    matches!(ext, "mov" | "MOV" | "mp4" | "MP4" | "m4v" | "M4V");
                eprintln!(
                    "Codec detection failed, fallback convert for .{}: {}",
                    ext, should_fallback_convert
                );
                should_fallback_convert
            }
        }
    }

    fn safe_duration(video: &Video) -> f64 {
        // Safely get duration, returning a default value if invalid
        let duration = video.duration().as_secs_f64();
        if duration.is_finite() && duration > 0.0 {
            duration
        } else {
            1.0 // Default to 1 second if invalid (prevents slider from breaking)
        }
    }

    fn get_fps_display(fps: f64) -> String {
        format!("{:.1} FPS", fps)
    }

    fn get_fps_color(current_fps: f64, native_fps: f64) -> Color {
        // Color based on performance relative to native framerate
        // Green: within 5% of native (excellent)
        // Yellow: 85-95% of native (good)
        // Orange: 70-85% of native (acceptable)
        // Red: below 70% of native (poor)

        let threshold_excellent = native_fps * 0.95;
        let threshold_good = native_fps * 0.85;
        let threshold_acceptable = native_fps * 0.70;

        if current_fps >= threshold_excellent {
            Color::from_rgb8(0, 255, 0) // Green - excellent (95%+)
        } else if current_fps >= threshold_good {
            Color::from_rgb8(255, 255, 0) // Yellow - good (85-95%)
        } else if current_fps >= threshold_acceptable {
            Color::from_rgb8(255, 165, 0) // Orange - acceptable (70-85%)
        } else {
            Color::from_rgb8(255, 0, 0) // Red - poor (<70%)
        }
    }

    fn load_video_from_path(&mut self, video_path: PathBuf) {
        eprintln!("load_video_from_path called for: {:?}", video_path);
        self.status = "Loading video...".to_string();

        match std::fs::metadata(&video_path) {
            Ok(_) => {
                // Check if video needs conversion based on codec
                let should_convert = Self::should_convert(&video_path);
                eprintln!("should_convert result: {}", should_convert);
                if should_convert {
                    eprintln!("Video needs conversion, checking cache...");

                    // Check if we already converted this file
                    if let Some(converted_path) = self.conversion_cache.get(&video_path) {
                        eprintln!("Cache hit! Using converted file");
                        self.load_converted_video(&video_path, converted_path.clone());
                    } else if !self.converting.contains_key(&video_path) {
                        // Not already converting, start background conversion
                        eprintln!("Starting background VP9 conversion");
                        let video_id = self.next_id;
                        self.converting.insert(video_path.clone(), video_id);
                        self.status = format!(
                            "Converting video ({})...",
                            video_path.file_name().unwrap_or_default().to_string_lossy()
                        );

                        // Spawn background conversion thread
                        let original_path = video_path.clone();
                        thread::spawn(move || {
                            Self::convert_video_background(&original_path, video_id);
                        });
                    } else {
                        // Already converting this file
                        self.status = format!(
                            "Video already converting... ({})",
                            video_path.file_name().unwrap_or_default().to_string_lossy()
                        );
                    }
                } else {
                    eprintln!("Video will be loaded directly");
                    self.load_direct_video(&video_path);
                }
            }
            Err(e) => {
                self.error = Some(format!("Video file not found: {}", e));
            }
        }
    }

    fn load_direct_video(&mut self, video_path: &PathBuf) {
        // Try to load video directly without conversion
        match url::Url::from_file_path(&video_path) {
            Ok(url) => match Video::new(&url) {
                Ok(mut video) => {
                    video.set_looping(true);
                    let native_fps = video.framerate();
                    let now = std::time::Instant::now();
                    let video_instance = VideoInstance {
                        id: self.next_id,
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
                    self.videos.push(video_instance);
                    self.next_id += 1;
                    self.error = None;
                    self.status = format!(
                        "Video loaded: {}",
                        video_path.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
                Err(e) => {
                    self.error = Some(format!("Failed to load video: {}", e));
                }
            },
            Err(_) => {
                self.error = Some("Invalid video path".to_string());
            }
        }
    }

    fn load_converted_video(&mut self, original_path: &PathBuf, converted_path: PathBuf) {
        match url::Url::from_file_path(&converted_path) {
            Ok(url) => match Video::new(&url) {
                Ok(mut video) => {
                    video.set_looping(true);
                    let native_fps = video.framerate();
                    let now = std::time::Instant::now();
                    let video_instance = VideoInstance {
                        id: self.next_id,
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
                    self.videos.push(video_instance);
                    self.next_id += 1;
                    self.error = None;
                    self.status = format!(
                        "Video loaded (converted): {}",
                        original_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    );
                }
                Err(e) => {
                    self.error = Some(format!("Failed to load converted video: {}", e));
                }
            },
            Err(_) => {
                self.error = Some("Invalid converted video path".to_string());
            }
        }
    }

    fn convert_video_background(original_path: &Path, _video_id: usize) {
        // Get cache directory
        let cache_dir = match std::env::var("HOME") {
            Ok(home) => {
                let cache_path = PathBuf::from(home).join(".cineplex_cache");
                let _ = std::fs::create_dir_all(&cache_path); // Create if not exists
                cache_path
            }
            Err(_) => return,
        };

        // Create a deterministic filename based on the original file path
        let mut hasher = DefaultHasher::new();
        original_path.hash(&mut hasher);
        let hash = hasher.finish();
        let converted_path = cache_dir.join(format!("converted_{:x}.webm", hash));
        let temp_path = cache_dir.join(format!("converted_{:x}.webm.tmp", hash));
        let marker_path = cache_dir.join(format!("converted_{:x}.webm.done", hash));

        eprintln!("Starting VP9 background conversion");
        eprintln!("Source: {:?}", original_path);
        eprintln!("Temp: {:?}", temp_path);
        eprintln!("Final: {:?}", converted_path);

        // Run ffmpeg conversion to temp file - VP9 with fast preset
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(original_path)
            .arg("-c:v")
            .arg("libvpx-vp9")
            .arg("-preset")
            .arg("fast")
            .arg("-b:v")
            .arg("0")
            .arg("-crf")
            .arg("23")
            .arg("-c:a")
            .arg("libopus")
            .arg("-b:a")
            .arg("128k")
            .arg("-f")
            .arg("webm")
            .arg("-y")
            .arg(&temp_path)
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    eprintln!("VP9 conversion successful, moving temp to final");
                    // Move temp file to final location
                    if std::fs::rename(&temp_path, &converted_path).is_ok() {
                        eprintln!("Successfully renamed temp file to final path");
                        // Create marker file to signal completion
                        let _ = std::fs::write(&marker_path, b"done");
                        eprintln!("VP9 conversion complete for {:?}", original_path);
                    } else {
                        eprintln!("Failed to rename temp file!");
                    }
                } else {
                    eprintln!("ffmpeg conversion failed!");
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    eprintln!("ffmpeg stderr: {}", stderr);
                    let _ = std::fs::remove_file(&temp_path);
                }
            }
            Err(e) => {
                eprintln!("Failed to execute ffmpeg: {}", e);
                let _ = std::fs::remove_file(&temp_path);
            }
        }
    }

    fn check_for_completed_conversions(&mut self) {
        // Check if any background conversions have completed
        let cache_dir = match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(".cineplex_cache"),
            Err(_) => return,
        };

        // Check each video that's being converted
        let paths_to_check: Vec<_> = self.converting.keys().cloned().collect();

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
                self.converting.remove(&original_path);
                self.conversion_cache
                    .insert(original_path.clone(), converted_path.clone());
                // Save the updated cache to persistent storage
                self.save_cache_metadata();
                self.load_converted_video(&original_path, converted_path);
                // Clean up marker file
                let _ = std::fs::remove_file(&marker_path);
            }
        }
    }

    fn update(&mut self, message: Message) {
        // Check for completed conversions first
        self.check_for_completed_conversions();

        match message {
            Message::BrowseFile => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Videos",
                        &[
                            "mov", "MOV", "mp4", "MP4", "m4v", "M4V", "mkv", "MKV", "avi", "AVI",
                            "webm", "WEBM",
                        ],
                    )
                    .pick_file()
                {
                    self.load_video_from_path(path);
                }
            }
            Message::ClearCache => {
                self.clear_cache_impl();
            }
            Message::FileDropped(path) => {
                self.load_video_from_path(path);
            }
            Message::EventOccurred(event) => match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    self.load_video_from_path(path);
                }
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(key),
                    ..
                }) => match key {
                    iced::keyboard::key::Named::ArrowRight
                    | iced::keyboard::key::Named::ArrowUp => {
                        if self.grid_columns < 10 {
                            self.grid_columns += 1;
                        }
                    }
                    iced::keyboard::key::Named::ArrowLeft
                    | iced::keyboard::key::Named::ArrowDown => {
                        if self.grid_columns > 1 {
                            self.grid_columns -= 1;
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            Message::IncreaseColumns => {
                if self.grid_columns < 10 {
                    self.grid_columns += 1;
                }
            }
            Message::DecreaseColumns => {
                if self.grid_columns > 1 {
                    self.grid_columns -= 1;
                }
            }
            Message::TogglePause(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    vid.video.set_paused(!vid.video.paused());
                }
            }
            Message::ToggleLoop(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    let new_looping_state = !vid.video.looping();
                    vid.video.set_looping(new_looping_state);
                    vid.looping_enabled = new_looping_state;
                }
            }
            Message::ToggleMute(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    let current_muted = vid.video.muted();
                    if current_muted {
                        // Unmute: restore volume to 1.0 and unmute
                        vid.video.set_volume(1.0);
                        vid.video.set_muted(false);
                    } else {
                        // Mute: set volume to 0 and mute
                        vid.video.set_volume(0.0);
                        vid.video.set_muted(true);
                    }
                }
            }
            Message::ToggleFullscreen(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    vid.fullscreen = !vid.fullscreen;
                }
            }
            Message::Seek(id, secs) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    // Validate secs is a valid number
                    if secs.is_finite() && secs >= 0.0 {
                        vid.dragging = true;
                        vid.video.set_paused(true);
                        vid.position = secs;
                    }
                }
            }
            Message::SeekRelease(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    vid.dragging = false;
                    // Validate position is valid before seeking (must be finite, non-negative, and not NaN)
                    if vid.position.is_finite() && vid.position >= 0.0 {
                        let _ = vid.video.seek(Duration::from_secs_f64(vid.position), true);
                    }
                    vid.video.set_paused(false);
                }
            }
            Message::EndOfStream(id) => {
                // Only loop if the looping_enabled flag is set
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    if vid.looping_enabled {
                        // Seek back to start and continue playing
                        vid.position = 0.0;
                        let _ = vid.video.seek(Duration::ZERO, true);
                        vid.video.set_paused(false);
                    }
                }
            }
            Message::NewFrame(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    // Update FPS counter (recalculate every second for efficiency)
                    vid.frame_count += 1;
                    let elapsed = vid.last_fps_time.elapsed();
                    if elapsed.as_secs_f64() >= 1.0 {
                        vid.current_fps = vid.frame_count as f64 / elapsed.as_secs_f64();
                        vid.frame_count = 0;
                        vid.last_fps_time = std::time::Instant::now();
                    }

                    if !vid.dragging {
                        let pos = vid.video.position().as_secs_f64();
                        // Only update position if it's a valid number
                        if pos.is_finite() && pos >= 0.0 {
                            vid.position = pos;
                        }
                    }

                    // Throttle UI updates to 30 FPS max (~33ms between redraws)
                    // Store that there's a pending update even if we skip the redraw
                    vid.pending_position_update = true;
                    let ui_update_elapsed = vid.last_ui_update.elapsed().as_millis() as u32;
                    if ui_update_elapsed < 33 {
                        // Skip redraw - return early to suppress view() rebuild
                        return;
                    }
                    vid.last_ui_update = std::time::Instant::now();
                }
            }
            Message::RemoveVideo(id) => {
                self.videos.retain(|v| v.id != id);
            }
            Message::VideoHoverChanged(id, hovered) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    vid.hovered = hovered;
                }
            }
            Message::ConversionStarted(path, _id) => {
                eprintln!("Conversion started for: {:?}", path);
            }
            Message::ConversionComplete(_original, converted, _id) => {
                eprintln!("Conversion complete: {:?}", converted);
            }
            Message::ConversionFailed(path, error, _id) => {
                eprintln!("Conversion failed for {:?}: {}", path, error);
                self.error = Some(format!("Conversion failed: {}", error));
                self.converting.remove(&path);
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen().map(Message::EventOccurred)
    }

    fn create_video_cell<'a>(&self, vid: &'a VideoInstance) -> Element<'a, Message> {
        let video_player = container(
            VideoPlayer::new(&vid.video)
                .on_end_of_stream(Message::EndOfStream(vid.id))
                .on_new_frame(Message::NewFrame(vid.id)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

        let mut stack_content = stack![video_player];

        // Add overlay controls when hovered
        if vid.hovered {
            let overlay = container(column![
                // Top bar with FPS and close button
                row![
                    {
                        let fps_text = Self::get_fps_display(vid.current_fps);
                        let fps_color = Self::get_fps_color(vid.current_fps, vid.native_fps);
                        text(fps_text)
                            .size(14)
                            .shaping(Shaping::Basic)
                            .color(fps_color)
                    },
                    container("").width(Length::Fill),
                    button(text("X").size(20))
                        .on_press(Message::RemoveVideo(vid.id))
                        .padding(5)
                        .width(Length::Shrink)
                        .height(Length::Shrink)
                ]
                .padding(10),
                // Center spacer
                container("").height(Length::Fill),
                // Bottom controls
                column![
                    // Seek slider
                    slider(
                        0.0..=Self::safe_duration(&vid.video),
                        vid.position,
                        move |pos| Message::Seek(vid.id, pos)
                    )
                    .step(0.1)
                    .on_release(Message::SeekRelease(vid.id)),
                    // Control buttons
                    row![
                        button(text(if vid.video.paused() { ">" } else { "||" }).size(12))
                            .on_press(Message::TogglePause(vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        button(text(if vid.video.looping() { "↻" } else { "→" }).size(12))
                            .on_press(Message::ToggleLoop(vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        button(text(if vid.video.muted() { "M" } else { "~" }).size(12))
                            .on_press(Message::ToggleMute(vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        button(text(if vid.fullscreen { "V" } else { "F" }).size(12))
                            .on_press(Message::ToggleFullscreen(vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        text(format!(
                            "{}:{:02}",
                            vid.position as u64 / 60,
                            vid.position as u64 % 60
                        ))
                        .size(12)
                    ]
                    .spacing(5)
                    .align_y(alignment::Vertical::Center)
                    .width(Length::Shrink)
                ]
                .spacing(5)
                .padding(10)
            ])
            .style(|_theme: &Theme| container::Style {
                background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill);

            stack_content = stack_content.push(overlay);
        }

        mouse_area(stack_content)
            .on_enter(Message::VideoHoverChanged(vid.id, true))
            .on_exit(Message::VideoHoverChanged(vid.id, false))
            .into()
    }

    fn view(&self) -> Element<'_, Message> {
        if let Some(error) = &self.error {
            return center(column![
                text("Error Loading Video").size(32),
                text(error.clone()),
                text("").size(10),
                text(self.status.clone()).size(12),
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        }

        if self.videos.is_empty() {
            return center(
                column![
                    text("Drag & Drop Video Here").size(48),
                    text("or click browse to load videos").size(16),
                    button(text("[Browse Files]").size(18))
                        .padding(10)
                        .on_press(Message::BrowseFile),
                    text("").size(10),
                    text(self.status.clone()).size(12),
                ]
                .spacing(20),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        }

        // Check if any video is in fullscreen mode
        if let Some(fullscreen_vid) = self.videos.iter().find(|v| v.fullscreen) {
            // Render fullscreen video with overlay
            let video_player = container(
                VideoPlayer::new(&fullscreen_vid.video)
                    .on_end_of_stream(Message::EndOfStream(fullscreen_vid.id))
                    .on_new_frame(Message::NewFrame(fullscreen_vid.id)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill);

            let overlay = container(column![
                // Top bar with FPS and close button
                row![
                    {
                        let fps_text = Self::get_fps_display(fullscreen_vid.current_fps);
                        let fps_color = Self::get_fps_color(fullscreen_vid.current_fps, fullscreen_vid.native_fps);
                        text(fps_text)
                            .size(14)
                            .shaping(Shaping::Basic)
                            .color(fps_color)
                    },
                    container("").width(Length::Fill),
                    button(text("X").size(20))
                        .on_press(Message::ToggleFullscreen(fullscreen_vid.id))
                        .padding(5)
                        .width(Length::Shrink)
                        .height(Length::Shrink)
                ]
                .padding(10),
                // Center spacer
                container("").height(Length::Fill),
                // Bottom controls
                column![
                    // Seek slider
                    slider(
                        0.0..=Self::safe_duration(&fullscreen_vid.video),
                        fullscreen_vid.position,
                        move |pos| Message::Seek(fullscreen_vid.id, pos)
                    )
                    .step(0.1)
                    .on_release(Message::SeekRelease(fullscreen_vid.id)),
                    // Control buttons
                    row![
                        button(text(if fullscreen_vid.video.paused() { ">" } else { "||" }).size(12))
                            .on_press(Message::TogglePause(fullscreen_vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        button(text(if fullscreen_vid.video.looping() { "↻" } else { "→" }).size(12))
                            .on_press(Message::ToggleLoop(fullscreen_vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        button(text(if fullscreen_vid.video.muted() { "M" } else { "~" }).size(12))
                            .on_press(Message::ToggleMute(fullscreen_vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        button(text("V").size(12))
                            .on_press(Message::ToggleFullscreen(fullscreen_vid.id))
                            .padding(8)
                            .width(Length::Shrink)
                            .height(Length::Shrink),
                        text(format!(
                            "{}:{:02}",
                            fullscreen_vid.position as u64 / 60,
                            fullscreen_vid.position as u64 % 60
                        ))
                        .size(12)
                    ]
                    .spacing(5)
                    .align_y(alignment::Vertical::Center)
                    .width(Length::Shrink)
                ]
                .spacing(5)
                .padding(10)
            ])
            .style(|_theme: &Theme| container::Style {
                background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill);

            let fullscreen_stack = stack![video_player, overlay];

            return mouse_area(fullscreen_stack)
                .on_enter(Message::VideoHoverChanged(fullscreen_vid.id, true))
                .on_exit(Message::VideoHoverChanged(fullscreen_vid.id, false))
                .into();
        }

        // Create grid with video cells using custom column count
        let grid: Element<'_, Message> = if self.videos.len() == 1 {
            // Single video: full screen
            self.create_video_cell(&self.videos[0])
        } else {
            // Multiple videos: use custom column count
            let mut rows: Vec<Element<'_, Message>> = Vec::new();

            for chunk in self.videos.chunks(self.grid_columns) {
                let row_content: Vec<Element<'_, Message>> = chunk
                    .iter()
                    .map(|vid| self.create_video_cell(vid))
                    .collect();

                rows.push(
                    row(row_content)
                        .spacing(5)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                );
            }

            column(rows)
                .spacing(5)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };

        // Main layout with grid and controls
        let controls = container(
            row![
                button(text("<").size(16))
                    .on_press(Message::DecreaseColumns)
                    .padding(5),
                text(format!("Grid: {} columns", self.grid_columns)).size(14),
                button(text(">").size(16))
                    .on_press(Message::IncreaseColumns)
                    .padding(5),
                container("").width(Length::Fill),
                button(text("[Browse]").size(14))
                    .on_press(Message::BrowseFile)
                    .padding(5),
                button(text("[Clear Cache]").size(12))
                    .on_press(Message::ClearCache)
                    .padding(5),
                text(format!("{} videos", self.videos.len())).size(12),
            ]
            .spacing(10)
            .align_y(alignment::Vertical::Center),
        )
        .padding(5)
        .width(Length::Fill);

        column![grid, controls]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
