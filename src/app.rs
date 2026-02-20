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
                    synchronized_set_paused(&mut vid.video, !was_paused);
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
                        // Pause during scrubbing for smoother experience
                        synchronized_set_paused(&mut vid.video, true);
                        vid.position = secs;
                    }
                }
            }
            Message::SeekRelease(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    let was_paused = vid.was_paused_before_drag;
                    vid.dragging = false;
                    // Validate position is valid before seeking (must be finite, non-negative, and not NaN)
                    if vid.position.is_finite() && vid.position >= 0.0 {
                        // Perform accurate seek
                        let _ = synchronized_seek(
                            &mut vid.video,
                            Duration::from_secs_f64(vid.position),
                            true,
                        );
                    }
                    // Resume playback if it wasn't paused before dragging
                    if !was_paused {
                        synchronized_set_paused(&mut vid.video, false);
                    }
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

                    // Update position from video (unless user is dragging the scrubber)
                    if !vid.dragging {
                        vid.position = vid.video.position().as_secs_f64();
                    }
                }
            }
            Message::RemoveMedia(id) => {
                let before_count = self.media.len();
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

        if has_hovered_media {
            Subscription::batch([
                event::listen().map(Message::EventOccurred),
                time::every(Duration::from_millis(100)).map(|_| Message::UiFadeTick),
            ])
        } else {
            event::listen().map(Message::EventOccurred)
        }
    }

    /// Render the view.
    pub fn view(&self) -> Element<'_, Message> {
        ui::render_main_view(self)
    }
}
