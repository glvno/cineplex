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
                    // Toggle pause state using cached value only
                    // NEVER query video.paused() - it can deadlock on macOS CoreAudio
                    let new_paused = !vid.is_paused;
                    synchronized_set_paused(id, &mut vid.video, new_paused);
                    vid.is_paused = new_paused; // Update cache
                    log::debug!("Pause toggled: video_id={}, paused={}", id, new_paused);
                }
            }
            Message::ToggleLoop(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    // Use cached state instead of querying GStreamer (which can block)
                    let new_looping = !vid.is_looping;
                    vid.video.set_looping(new_looping);
                    vid.is_looping = new_looping; // Update cache
                    vid.looping_enabled = new_looping; // Update legacy field
                    log::debug!(
                        "Video looping toggled: id={}, looping={}",
                        id,
                        new_looping
                    );
                }
            }
            Message::ToggleMute(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    // Use cached state instead of querying GStreamer (which can block)
                    let new_muted = !vid.is_muted;
                    if new_muted {
                        // Mute: set volume to 0 and mute
                        vid.video.set_volume(0.0);
                        vid.video.set_muted(true);
                    } else {
                        // Unmute: restore volume to 1.0 and unmute
                        vid.video.set_volume(1.0);
                        vid.video.set_muted(false);
                    }
                    vid.is_muted = new_muted; // Update cache
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
                        // Use cached state instead of querying GStreamer (which can block)
                        vid.was_paused_before_drag = vid.is_paused;
                        // Just update UI position while dragging
                        vid.position = secs;
                    }
                }
            }
            Message::SeekRelease(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    vid.dragging = false;
                    // Validate position is valid before seeking
                    if vid.position.is_finite() && vid.position >= 0.0 {
                        let target_pos = vid.position;
                        log::info!(
                            "Seeking: video_id={}, target={:.2}s",
                            id,
                            target_pos
                        );
                        let _ = synchronized_seek(
                            id,
                            &mut vid.video,
                            Duration::from_secs_f64(target_pos),
                            true,
                        );
                    }
                }
            }
            Message::SeekComplete(id) => {
                // No longer needed - seeks are processed synchronously
                log::debug!("SeekComplete message received for video_id={} (ignored)", id);
            }
            Message::EndOfStream(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    log::info!(
                        "EOS: video_id={}, is_looping={}, position={:.2}s/{:.2}s",
                        id,
                        vid.is_looping,
                        vid.position,
                        vid.duration
                    );

                    // Update duration to actual observed length if different
                    if vid.position > vid.duration && vid.position.is_finite() {
                        log::warn!(
                            "Duration fix: video_id={}, was={:.2}s, actual={:.2}s",
                            id,
                            vid.duration,
                            vid.position
                        );
                        vid.duration = vid.position;
                    }

                    // If looping is enabled, seek to start and force playing state.
                    // After EOS, GStreamer may not auto-resume after a seek, so we
                    // always call set_paused(false) regardless of cached state.
                    if vid.is_looping {
                        log::info!("Loop restart: video_id={}", id);
                        let _ = synchronized_seek(
                            id,
                            &mut vid.video,
                            Duration::from_secs(0),
                            false,
                        );
                        synchronized_set_paused(id, &mut vid.video, false);
                        vid.is_paused = false;
                    }
                }
            }
            Message::NewFrame(_id) => {
                // No longer used - removed on_new_frame callback to prevent
                // layout invalidation warnings caused by excessive view() calls
                // FPS is now displayed using native_fps instead of calculated FPS
            }
            Message::PositionTick => {
                // Process position updates from background thread
                // Only process on PositionTick to avoid excessive re-renders
                let mut position_updates = Vec::new();
                if let Some(ref rx) = self.position_thread_rx {
                    while let Ok(update) = rx.try_recv() {
                        position_updates.push(update);
                    }
                }

                let now = Instant::now();

                // Apply position updates and detect stalls
                for update in position_updates {
                    if let Some(vid) = self.find_video_mut(update.video_id) {
                        // Track when position actually *changes* for stall detection.
                        // A stuck video still responds to position queries (same value),
                        // so we must check the value, not just whether we got an update.
                        if (update.position - vid.last_position_value).abs() > 0.01 {
                            vid.last_position_update = now;
                            vid.last_position_value = update.position;
                        }

                        // Only update position if user isn't dragging
                        if !vid.dragging {
                            // Only update if the displayed value changed (whole seconds)
                            let old_display_pos = vid.position as u64;
                            let new_display_pos = update.position as u64;
                            if old_display_pos != new_display_pos || (vid.position - update.position).abs() > 1.0 {
                                vid.position = update.position;
                            }

                            // If position exceeds cached duration, the duration query was wrong
                            // Expand duration to accommodate the actual playback length
                            if update.position > vid.duration && update.position.is_finite() {
                                let old_duration = vid.duration;
                                vid.duration = (update.position * 1.1).max(vid.duration + 5.0); // Add 10% buffer or 5s, whichever is larger
                                log::warn!(
                                    "Duration correction: video_id={}, was={:.2}s, observed={:.2}s, now={:.2}s",
                                    update.video_id,
                                    old_duration,
                                    update.position,
                                    vid.duration
                                );
                            }
                        }
                    }
                }

                // Detect stalled videos and recover ONE per cycle (every ~1s).
                // Recovering all at once triggers CoreAudio deadlock; one at a time is safe.
                self.stall_check_counter += 1;
                if self.stall_check_counter >= 10 {
                    self.stall_check_counter = 0;

                    // Find first stalled video and recover it
                    let mut stalled_id = None;
                    for item in &self.media {
                        if let MediaItem::Video(vid) = item {
                            if !vid.is_paused && vid.is_looping {
                                let time_since_update = now.duration_since(vid.last_position_update).as_secs_f64();
                                if time_since_update > 3.0 {
                                    log::warn!(
                                        "Stalled video: video_id={}, position={:.2}s, last_update={:.1}s ago",
                                        vid.id,
                                        vid.last_position_value,
                                        time_since_update
                                    );
                                    stalled_id = Some(vid.id);
                                    break; // Only recover one per cycle
                                }
                            }
                        }
                    }

                    if let Some(id) = stalled_id {
                        if let Some(vid) = self.find_video_mut(id) {
                            log::warn!("Recovering stalled video_id={}", id);
                            // Seek to current position and force playing state,
                            // same approach as loop restart
                            let pos = vid.last_position_value;
                            let _ = synchronized_seek(
                                id,
                                &mut vid.video,
                                Duration::from_secs_f64(pos),
                                false,
                            );
                            synchronized_set_paused(id, &mut vid.video, false);
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
                // Just triggers fade animations - FPS now uses native_fps
                // No state validation - NEVER query GStreamer state on main thread (causes deadlocks)
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
        // Check if we need UI updates (fade or FPS tracking)
        let has_hovered_media = self.media.iter().any(|m| match m {
            MediaItem::Video(v) => v.hovered,
            MediaItem::Photo(p) => p.hovered,
        });

        let has_videos = self.media.iter().any(|m| matches!(m, MediaItem::Video(_)));

        let mut subscriptions = vec![event::listen().map(Message::EventOccurred)];

        // Always tick when there are videos (for FPS updates) or hovered media (for fade)
        if has_videos || has_hovered_media {
            subscriptions.push(time::every(Duration::from_millis(100)).map(|_| Message::UiFadeTick));
        }

        if has_videos {
            subscriptions.push(crate::position_poller::position_update_subscription());
            // Note: bus_monitor subscription removed - now using bus_watcher background thread
        }

        Subscription::batch(subscriptions)
    }

    /// Render the view.
    pub fn view(&self) -> Element<'_, Message> {
        ui::render_main_view(self)
    }
}
