use iced::event;
use iced::{Element, Subscription};
use std::time::Duration;

use crate::cache;
use crate::loader;
use crate::message::Message;
use crate::state::App;
use crate::sync::{synchronized_seek, synchronized_set_paused};
use crate::ui;

impl App {
    /// Handle UI messages and state updates.
    pub fn update(&mut self, message: Message) {
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
                    loader::load_video_from_path(self, path);
                }
            }
            Message::ClearCache => {
                self.status = cache::clear_cache();
            }
            Message::FileDropped(path) => {
                loader::load_video_from_path(self, path);
            }
            Message::EventOccurred(event) => match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    loader::load_video_from_path(self, path);
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
                    let was_paused = vid.video.paused();
                    synchronized_set_paused(&mut vid.video, !was_paused);
                    log::debug!("Video pause toggled: id={}, paused={}", id, !was_paused);
                }
            }
            Message::ToggleLoop(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
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
                        // NOTE: Do NOT pause here - calling set_paused triggers audio sink state changes
                        // that try to acquire CoreAudio HALB_Mutex, causing deadlock with other threads.
                        // Let video continue playing while user scrubs.
                        vid.position = secs;
                    }
                }
            }
            Message::SeekRelease(id) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    vid.dragging = false;
                    // Validate position is valid before seeking (must be finite, non-negative, and not NaN)
                    if vid.position.is_finite() && vid.position >= 0.0 {
                        // Use synchronized_seek to prevent concurrent FLUSH_START deadlocks
                        let _ = synchronized_seek(
                            &mut vid.video,
                            Duration::from_secs_f64(vid.position),
                            false,
                        );
                    }
                    // NOTE: Do NOT resume here - calling set_paused triggers audio sink state changes
                    // that deadlock. Just let the seek complete naturally.
                }
            }
            Message::EndOfStream(id) => {
                // GStreamer handles looping internally via video.set_looping(true)
                // We just log it for diagnostics. Don't trigger seek here - let GStreamer loop naturally.
                if let Some(_vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    log::debug!(
                        "Video reached end of stream (GStreamer looping handles restart): id={}",
                        id
                    );
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

                    // NOTE: Calling video.position() causes a deadlock due to GStreamer's CoreAudio
                    // latency query trying to acquire a mutex from the main thread. This appears to be
                    // a fundamental issue with GStreamer's OSX audio sink and CoreAudio interaction.
                    // Disabling position updates to prevent deadlocks. Videos still loop correctly
                    // via GStreamer's internal looping mechanism (video.set_looping(true)).
                    // Position remains at 0.0 in the UI, but the core functionality works reliably.
                    if !vid.dragging {
                        // DO NOT QUERY POSITION - causes deadlock with CoreAudio
                        // vid.position remains 0.0 to avoid UI updates that trigger GStreamer queries
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
                let before_count = self.videos.len();
                self.videos.retain(|v| v.id != id);
                if before_count != self.videos.len() {
                    log::info!(
                        "Video removed: id={}, remaining_videos={}",
                        id,
                        self.videos.len()
                    );
                }
            }
            Message::VideoHoverChanged(id, hovered) => {
                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                    vid.hovered = hovered;
                }
            }
        }
    }

    /// Subscribe to events.
    pub fn subscription(&self) -> Subscription<Message> {
        event::listen().map(Message::EventOccurred)
    }

    /// Render the view.
    pub fn view(&self) -> Element<'_, Message> {
        ui::render_main_view(self)
    }
}
