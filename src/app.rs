use iced::event;
use iced::time;
use iced::{Element, Subscription};
use std::time::{Duration, Instant};

use crate::loader;
use crate::message::Message;
use crate::state::{App, MediaItem};
use crate::sync::{synchronized_seek, synchronized_set_paused};
use crate::ui;

impl App {
    /// Find a video by ID.
    fn find_video_mut(&mut self, id: usize) -> Option<&mut crate::state::VideoInstance> {
        self.media.iter_mut().find_map(|m| match m {
            MediaItem::Video(v) if v.id == id => Some(v),
            _ => None,
        })
    }

    /// Handle UI messages and state updates.
    pub fn update(&mut self, message: Message) {
        // Signal watchdog that UI thread is alive
        self.watchdog.heartbeat();

        match message {
            Message::BrowseFile => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Media",
                        &[
                            "mov", "MOV", "mp4", "MP4", "m4v", "M4V", "mkv", "MKV", "avi", "AVI",
                            "webm", "WEBM", "jpg", "JPG", "jpeg", "JPEG", "png", "PNG", "gif",
                            "GIF", "bmp", "BMP", "webp", "WEBP", "tiff", "TIFF", "tif", "TIF",
                        ],
                    )
                    .pick_file()
                {
                    loader::load_media_from_path(self, path);
                }
            }
            Message::FileDropped(path) => {
                loader::load_media_from_path(self, path);
            }
            Message::EventOccurred(event) => match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    loader::load_media_from_path(self, path);
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
                if let Some(vid) = self.find_video_mut(id) {
                    let was_paused = vid.video.paused();
                    synchronized_set_paused(id, &mut vid.video, !was_paused);
                    log::debug!("Video pause toggled: id={}, paused={}", id, !was_paused);
                }
            }
            Message::ToggleLoop(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    let new_looping_state = !vid.video.looping();
                    vid.video.set_looping(new_looping_state);
                    vid.looping_enabled = new_looping_state;
                    log::debug!(
                        "Video looping toggled: id={}, looping={}",
                        id,
                        new_looping_state
                    );
                }
            }
            Message::ToggleMute(id) => {
                if let Some(vid) = self.find_video_mut(id) {
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
                if let Some(item) = self.media.iter_mut().find(|m| m.id() == id) {
                    match item {
                        MediaItem::Video(v) => v.fullscreen = !v.fullscreen,
                        MediaItem::Photo(p) => p.fullscreen = !p.fullscreen,
                    }
                }
            }
            Message::Seek(id, secs) => {
                if let Some(vid) = self.find_video_mut(id) {
                    // Validate secs is a valid number
                    if secs.is_finite() && secs >= 0.0 {
                        vid.dragging = true;
                        vid.was_paused_before_drag = vid.video.paused();
                        // Update position without pausing (reduces synchronous GStreamer calls)
                        vid.position = secs;
                    }
                }
            }
            Message::SeekRelease(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    vid.dragging = false;
                    // Validate position is valid before seeking (must be finite, non-negative, and not NaN)
                    if vid.position.is_finite() && vid.position >= 0.0 {
                        // Perform accurate seek without pause/unpause to reduce synchronous calls
                        let _ = synchronized_seek(
                            id,
                            &mut vid.video,
                            Duration::from_secs_f64(vid.position),
                            true,
                        );
                    }
                    // Note: Video continues playing during/after seek unless user paused it
                }
            }
            Message::EndOfStream(id) => {
                // GStreamer handles looping internally via video.set_looping(true)
                // We just log it for diagnostics. Don't trigger seek here - let GStreamer loop naturally.
                if self.find_video_mut(id).is_some() {
                    log::debug!(
                        "Video reached end of stream (GStreamer looping handles restart): id={}",
                        id
                    );
                }
            }
            Message::NewFrame(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    // Update FPS counter (recalculate every second for efficiency)
                    vid.frame_count += 1;
                    let elapsed = vid.last_fps_time.elapsed();
                    if elapsed.as_secs_f64() >= 1.0 {
                        vid.current_fps = vid.frame_count as f64 / elapsed.as_secs_f64();
                        vid.frame_count = 0;
                        vid.last_fps_time = std::time::Instant::now();
                    }

                    // Position is now updated via PositionTick subscription
                    // to decouple it from frame rendering and reduce blocking
                }
            }
            Message::PositionTick => {
                // Update positions for all videos (unless user is dragging)
                // This runs at 10Hz independent of frame rate
                for item in &mut self.media {
                    if let MediaItem::Video(vid) = item {
                        if !vid.dragging {
                            let thread_id = std::thread::current().id();
                            let start = crate::gst_logger::log_position_query_start(vid.id, thread_id);
                            let position = vid.video.position();
                            crate::gst_logger::log_position_query_complete(vid.id, position, start);
                            vid.position = position.as_secs_f64();
                        }
                    }
                }
            }
            Message::RemoveMedia(id) => {
                let before_count = self.media.len();
                // Log video destruction before removing
                if let Some(item) = self.media.iter().find(|m| m.id() == id) {
                    if matches!(item, MediaItem::Video(_)) {
                        crate::gst_logger::log_video_destroyed(id);
                    }
                }
                self.media.retain(|m| m.id() != id);
                if before_count != self.media.len() {
                    log::info!(
                        "Media removed: id={}, remaining_media={}",
                        id,
                        self.media.len()
                    );
                }
            }
            Message::MediaHoverChanged(id, hovered) => {
                if let Some(item) = self.media.iter_mut().find(|m| m.id() == id) {
                    let now = Instant::now();
                    match item {
                        MediaItem::Video(v) => {
                            v.hovered = hovered;
                            if hovered {
                                v.last_mouse_activity = now;
                            }
                        }
                        MediaItem::Photo(p) => {
                            p.hovered = hovered;
                            if hovered {
                                p.last_mouse_activity = now;
                            }
                        }
                    }
                }
            }
            Message::MouseMoved(id) => {
                if let Some(item) = self.media.iter_mut().find(|m| m.id() == id) {
                    let now = Instant::now();
                    match item {
                        MediaItem::Video(v) => v.last_mouse_activity = now,
                        MediaItem::Photo(p) => p.last_mouse_activity = now,
                    }
                }
            }
            Message::UiFadeTick => {
                // Just triggers a re-render; opacity is computed in view
            }
            Message::LoadInitialFiles(paths) => {
                for path in paths {
                    loader::load_media_from_path(self, path);
                }
            }
        }
    }

    /// Subscribe to events.
    pub fn subscription(&self) -> Subscription<Message> {
        // Only tick when there's media that might need fading
        let has_hovered_media = self.media.iter().any(|m| match m {
            MediaItem::Video(v) => v.hovered,
            MediaItem::Photo(p) => p.hovered,
        });

        // Only poll positions when there are videos
        let has_videos = self.media.iter().any(|m| matches!(m, MediaItem::Video(_)));

        let mut subscriptions = vec![event::listen().map(Message::EventOccurred)];

        if has_hovered_media {
            subscriptions.push(time::every(Duration::from_millis(100)).map(|_| Message::UiFadeTick));
        }

        if has_videos {
            subscriptions.push(crate::position_poller::position_update_subscription());
        }

        Subscription::batch(subscriptions)
    }

    /// Render the view.
    pub fn view(&self) -> Element<'_, Message> {
        ui::render_main_view(self)
    }
}
