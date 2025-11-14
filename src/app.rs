use iced::event;
use iced::{Element, Subscription};
use std::time::Duration;

use crate::cache;
use crate::loader;
use crate::message::Message;
use crate::state::App;
use crate::ui;

impl App {
    /// Handle UI messages and state updates.
    pub fn update(&mut self, message: Message) {
        // Check for completed conversions first
        loader::check_for_completed_conversions(self);

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
                self.conversion_cache.clear();
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

    /// Subscribe to events.
    pub fn subscription(&self) -> Subscription<Message> {
        event::listen().map(Message::EventOccurred)
    }

    /// Render the view.
    pub fn view(&self) -> Element<'_, Message> {
        ui::render_main_view(self)
    }
}
