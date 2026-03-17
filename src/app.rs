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
    /// Returns the ID of the keyboard shortcut target:
    /// - In fullscreen mode: the fullscreen media's ID
    /// - In grid mode: the hovered media's ID (if any)
    fn shortcut_target_id(&self) -> Option<usize> {
        if let Some(item) = self.media.iter().find(|m| m.is_fullscreen()) {
            return Some(item.id());
        }
        self.media.iter().find_map(|m| match m {
            MediaItem::Video(v) if v.hovered => Some(v.id),
            MediaItem::Photo(p) if p.hovered => Some(p.id),
            _ => None,
        })
    }

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
                    let id = self.next_id;
                    self.next_id += 1;
                    self.loading_count += 1;
                    self.status = "Loading...".to_string();
                    loader::load_media_async(self.load_tx.clone(), path, id);
                }
            }
            Message::EventOccurred(event) => match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.loading_count += 1;
                    self.status = "Loading...".to_string();
                    loader::load_media_async(self.load_tx.clone(), path, id);
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
                    iced::keyboard::key::Named::Space => {
                        if let Some(id) = self.shortcut_target_id() {
                            if let Some(vid) = self.find_video_mut(id) {
                                let new_paused = !vid.video.paused();
                                synchronized_set_paused(id, &vid.video, new_paused);
                            }
                        }
                    }
                    _ => {}
                },
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Character(ch),
                    ..
                }) => {
                    if let Some(id) = self.shortcut_target_id() {
                        match ch.as_str() {
                            "f" => {
                                if let Some(item) = self.media.iter_mut().find(|m| m.id() == id) {
                                    match item {
                                        MediaItem::Video(v) => v.fullscreen = !v.fullscreen,
                                        MediaItem::Photo(p) => p.fullscreen = !p.fullscreen,
                                    }
                                }
                            }
                            "m" => {
                                if let Some(vid) = self.find_video_mut(id) {
                                    let enabled = vid.video.audio_enabled();
                                    let _ = vid.video.set_audio_enabled(!enabled);
                                }
                            }
                            "l" => {
                                if let Some(vid) = self.find_video_mut(id) {
                                    vid.video.set_looping(!vid.video.looping());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => {
                    if let Some(source_id) = self.drag_source_id.take() {
                        if let Some((target_id, insert_before)) = self.drag_target.take() {
                            // Find indices by ID
                            let source_idx = self.media.iter().position(|m| m.id() == source_id);
                            let target_idx = self.media.iter().position(|m| m.id() == target_id);
                            if let (Some(si), Some(ti)) = (source_idx, target_idx) {
                                let item = self.media.remove(si);
                                let adjusted = if si < ti { ti - 1 } else { ti };
                                let insert_idx = if insert_before {
                                    adjusted
                                } else {
                                    adjusted + 1
                                };
                                let insert_idx = insert_idx.min(self.media.len());
                                self.media.insert(insert_idx, item);
                            }
                        }
                    }
                    self.drag_target = None;
                }
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    self.window_width = size.width;
                }
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
                    let new_paused = !vid.video.paused();
                    synchronized_set_paused(id, &vid.video, new_paused);
                    log::debug!("Pause toggled: video_id={}, paused={}", id, new_paused);
                }
            }
            Message::ToggleLoop(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    vid.video.set_looping(!vid.video.looping());
                }
            }
            Message::ToggleMute(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    let enabled = vid.video.audio_enabled();
                    let _ = vid.video.set_audio_enabled(!enabled);
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
                        log::info!("Seeking: video_id={}, target={:.2}s", id, target_pos);
                        let _ = synchronized_seek(
                            id,
                            &vid.video,
                            Duration::from_secs_f64(target_pos),
                            true,
                        );
                    }
                }
            }
            Message::EndOfStream(id) => {
                if let Some(vid) = self.find_video_mut(id) {
                    log::info!(
                        "EOS: video_id={}, is_looping={}, position={:.2}s/{:.2}s",
                        id,
                        vid.video.looping(),
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
                    if vid.video.looping() {
                        log::info!("Loop restart: video_id={}", id);
                        let _ =
                            synchronized_seek(id, &vid.video, Duration::from_secs(0), false);
                        synchronized_set_paused(id, &vid.video, false);
                    }
                }
            }
            Message::UiFadeTick => {
                // Update position from video's background worker thread (non-blocking)
                for item in &mut self.media {
                    if let MediaItem::Video(vid) = item {
                        if !vid.dragging {
                            let pos = vid.video.cached_position().as_secs_f64();

                            // Only update if the displayed value changed meaningfully
                            let old_display_pos = vid.position as u64;
                            let new_display_pos = pos as u64;
                            if old_display_pos != new_display_pos
                                || (vid.position - pos).abs() > 1.0
                            {
                                vid.position = pos;
                            }

                            // If position exceeds cached duration, expand it
                            if pos > vid.duration && pos.is_finite() {
                                let old_duration = vid.duration;
                                vid.duration = (pos * 1.1).max(vid.duration + 5.0);
                                log::warn!(
                                    "Duration correction: video_id={}, was={:.2}s, observed={:.2}s, now={:.2}s",
                                    vid.id,
                                    old_duration,
                                    pos,
                                    vid.duration
                                );
                            }
                        }
                    }
                }

                // Detect stalled videos and recover ONE per cycle (every ~1s).
                self.stall_check_counter += 1;
                if self.stall_check_counter >= 10 {
                    self.stall_check_counter = 0;

                    // Find first stalled video and recover it
                    let mut stalled_id = None;
                    for item in &self.media {
                        if let MediaItem::Video(vid) = item {
                            if !vid.video.paused() && vid.video.looping() {
                                let stall_time = vid.video.time_since_position_change();
                                if stall_time > Duration::from_secs(3) {
                                    log::warn!(
                                        "Stalled video: video_id={}, position={:.2}s, stalled={:.1}s",
                                        vid.id,
                                        vid.position,
                                        stall_time.as_secs_f64()
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
                            let pos = vid.video.cached_position();
                            let _ = synchronized_seek(id, &vid.video, pos, false);
                            synchronized_set_paused(id, &vid.video, false);
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
            Message::DragStart(id) => {
                self.drag_source_id = Some(id);
                self.drag_target = None;
            }
            Message::MouseMoved(id, point) => {
                // Always update mouse activity for UI fade
                if let Some(item) = self.media.iter_mut().find(|m| m.id() == id) {
                    let now = Instant::now();
                    match item {
                        MediaItem::Video(v) => v.last_mouse_activity = now,
                        MediaItem::Photo(p) => p.last_mouse_activity = now,
                    }
                }
                // Update drag target if dragging over a different cell
                if let Some(source_id) = self.drag_source_id {
                    if id != source_id {
                        let cell_width = self.window_width / self.grid_columns as f32;
                        let insert_before = point.x < cell_width / 2.0;
                        self.drag_target = Some((id, insert_before));
                    } else {
                        self.drag_target = None;
                    }
                }
            }
            Message::LoadInitialFiles(paths) => {
                for path in paths {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.loading_count += 1;
                    loader::load_media_async(self.load_tx.clone(), path, id);
                }
                if self.loading_count > 0 {
                    self.status = format!("Loading {} files...", self.loading_count);
                }
            }
            Message::CheckLoadedMedia => {
                while let Ok(result) = self.load_rx.try_recv() {
                    self.loading_count = self.loading_count.saturating_sub(1);
                    match result {
                        crate::state::LoadResult::Video(video_instance) => {
                            let vid_id = video_instance.id;
                            let fps = video_instance.native_fps;
                            self.media.push(MediaItem::Video(video_instance));
                            log::info!(
                                "Video ready: id={}, fps={}, total_media={}",
                                vid_id,
                                fps,
                                self.media.len()
                            );
                            self.error = None;
                        }
                        crate::state::LoadResult::Photo(photo_instance) => {
                            let photo_id = photo_instance.id;
                            let filename = photo_instance.filename.clone();
                            self.media.push(MediaItem::Photo(photo_instance));
                            log::info!(
                                "Photo ready: id={}, name={}, total_media={}",
                                photo_id,
                                filename,
                                self.media.len()
                            );
                            self.error = None;
                        }
                        crate::state::LoadResult::Error(e) => {
                            log::error!("Media load error: {}", e);
                            self.error = Some(e);
                        }
                    }
                }

                if self.loading_count > 0 {
                    self.status = format!("Loading {} file{}...", self.loading_count, if self.loading_count == 1 { "" } else { "s" });
                } else if !self.media.is_empty() {
                    self.status = format!("{} media loaded", self.media.len());
                }
            }
        }
    }

    /// Subscribe to events.
    pub fn subscription(&self) -> Subscription<Message> {
        let has_hovered_media = self.media.iter().any(|m| match m {
            MediaItem::Video(v) => v.hovered,
            MediaItem::Photo(p) => p.hovered,
        });

        let has_videos = self.media.iter().any(|m| matches!(m, MediaItem::Video(_)));

        let mut subscriptions = vec![event::listen().map(Message::EventOccurred)];

        // Tick for UI fade + position updates (100ms when videos or hovered media present)
        if has_videos || has_hovered_media {
            subscriptions
                .push(time::every(Duration::from_millis(100)).map(|_| Message::UiFadeTick));
        }

        // Poll for loaded media results when background loads are in-flight
        if self.loading_count > 0 {
            subscriptions.push(
                time::every(Duration::from_millis(100)).map(|_| Message::CheckLoadedMedia),
            );
        }

        Subscription::batch(subscriptions)
    }

    /// Render the view.
    pub fn view(&self) -> Element<'_, Message> {
        ui::render_main_view(self)
    }
}
