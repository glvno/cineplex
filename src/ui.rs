use iced::widget::text::Shaping;
use iced::widget::{
    button, center, column, container, image, mouse_area, row, slider, stack, text,
};
use iced::{alignment, Color, Element, Length, Theme};
use iced_video_player::{Video, VideoPlayer};

use crate::message::Message;
use crate::state::{App, MediaItem, PhotoInstance, VideoInstance};

/// Get the safe duration of a video, handling invalid values.
pub fn safe_duration(video: &Video) -> f64 {
    let duration = video.duration().as_secs_f64();
    if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        1.0 // Default to 1 second if invalid (prevents slider from breaking)
    }
}

/// Format FPS for display.
pub fn get_fps_display(fps: f64) -> String {
    format!("{:.1} FPS", fps)
}

/// Get the color for FPS display based on performance.
pub fn get_fps_color(current_fps: f64, native_fps: f64) -> Color {
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

/// Create a media cell (routes to video or photo cell based on type).
pub fn create_media_cell<'a>(app: &'a App, item: &'a MediaItem) -> Element<'a, Message> {
    match item {
        MediaItem::Video(vid) => create_video_cell(app, vid),
        MediaItem::Photo(photo) => create_photo_cell(app, photo),
    }
}

/// Create a video cell with player and overlay controls.
pub fn create_video_cell<'a>(_app: &'a App, vid: &'a VideoInstance) -> Element<'a, Message> {
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
        let overlay = build_video_overlay(vid);
        stack_content = stack_content.push(overlay);
    }

    mouse_area(stack_content)
        .on_enter(Message::MediaHoverChanged(vid.id, true))
        .on_exit(Message::MediaHoverChanged(vid.id, false))
        .into()
}

/// Create a photo cell with image and overlay controls.
pub fn create_photo_cell<'a>(_app: &'a App, photo: &'a PhotoInstance) -> Element<'a, Message> {
    let photo_view = container(image(&photo.handle).content_fit(iced::ContentFit::Contain))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    let mut stack_content = stack![photo_view];

    // Add overlay controls when hovered
    if photo.hovered {
        let overlay = build_photo_overlay(photo);
        stack_content = stack_content.push(overlay);
    }

    mouse_area(stack_content)
        .on_enter(Message::MediaHoverChanged(photo.id, true))
        .on_exit(Message::MediaHoverChanged(photo.id, false))
        .into()
}

/// Build the overlay controls for a photo.
fn build_photo_overlay<'a>(photo: &'a PhotoInstance) -> Element<'a, Message> {
    let overlay = container(
        column![
            // Top bar with filename and close button
            row![
                text(&photo.filename).size(14).color(Color::WHITE),
                container("").width(Length::Fill),
                button(text("X").size(20))
                    .on_press(Message::RemoveMedia(photo.id))
                    .padding(5)
                    .width(Length::Shrink)
                    .height(Length::Shrink)
            ]
            .padding(10),
            // Center spacer
            container("").height(Length::Fill),
            // Bottom controls (just fullscreen for photos)
            row![button(text(if photo.fullscreen { "V" } else { "F" }).size(12))
                .on_press(Message::ToggleFullscreen(photo.id))
                .padding(8)
                .width(Length::Shrink)
                .height(Length::Shrink),]
            .spacing(5)
            .align_y(alignment::Vertical::Center)
            .padding(10)
        ],
    )
    .style(|_theme: &Theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill);

    overlay.into()
}

/// Build the overlay controls for a video.
fn build_video_overlay<'a>(vid: &'a VideoInstance) -> Element<'a, Message> {
    let overlay = container(
        column![
            // Top bar with FPS and close button
            row![
                {
                    let fps_text = get_fps_display(vid.current_fps);
                    let fps_color = get_fps_color(vid.current_fps, vid.native_fps);
                    text(fps_text)
                        .size(14)
                        .shaping(Shaping::Basic)
                        .color(fps_color)
                },
                container("").width(Length::Fill),
                button(text("X").size(20))
                    .on_press(Message::RemoveMedia(vid.id))
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
                slider(0.0..=safe_duration(&vid.video), vid.position, move |pos| {
                    Message::Seek(vid.id, pos)
                })
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
        ]
    )
    .style(|_theme: &Theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill);

    overlay.into()
}

/// Render the main view.
pub fn render_main_view(app: &App) -> Element<'_, Message> {
    // Error state
    if let Some(error) = &app.error {
        return center(column![
            text("Error Loading Media").size(32),
            text(error.clone()),
            text("").size(10),
            text(app.status.clone()).size(12),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    // Empty state
    if app.media.is_empty() {
        return center(
            column![
                text("Drag & Drop Media Here").size(48),
                text("or click browse to load videos and photos").size(16),
                button(text("[Browse Files]").size(18))
                    .padding(10)
                    .on_press(Message::BrowseFile),
                text("").size(10),
                text(app.status.clone()).size(12),
            ]
            .spacing(20),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    // Fullscreen mode
    if let Some(fullscreen_item) = app.media.iter().find(|m| m.is_fullscreen()) {
        return render_fullscreen_view(app, fullscreen_item);
    }

    // Grid mode - create media cells
    let grid: Element<'_, Message> = if app.media.len() == 1 {
        // Single item: full screen
        create_media_cell(app, &app.media[0])
    } else {
        // Multiple items: use custom column count
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        for chunk in app.media.chunks(app.grid_columns) {
            let row_content: Vec<Element<'_, Message>> = chunk
                .iter()
                .map(|item| create_media_cell(app, item))
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

    // Bottom control bar
    let controls = render_controls_bar(app);

    column![grid, controls]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Render the fullscreen view for a media item.
fn render_fullscreen_view<'a>(app: &'a App, item: &'a MediaItem) -> Element<'a, Message> {
    match item {
        MediaItem::Video(vid) => render_fullscreen_video(app, vid),
        MediaItem::Photo(photo) => render_fullscreen_photo(app, photo),
    }
}

/// Render the fullscreen view for a single video.
fn render_fullscreen_video<'a>(
    _app: &'a App,
    fullscreen_vid: &'a VideoInstance,
) -> Element<'a, Message> {
    let video_player = container(
        VideoPlayer::new(&fullscreen_vid.video)
            .on_end_of_stream(Message::EndOfStream(fullscreen_vid.id))
            .on_new_frame(Message::NewFrame(fullscreen_vid.id)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    let overlay = container(
        column![
            // Top bar with FPS and close button
            row![
                {
                    let fps_text = get_fps_display(fullscreen_vid.current_fps);
                    let fps_color =
                        get_fps_color(fullscreen_vid.current_fps, fullscreen_vid.native_fps);
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
                    0.0..=safe_duration(&fullscreen_vid.video),
                    fullscreen_vid.position,
                    move |pos| Message::Seek(fullscreen_vid.id, pos)
                )
                .step(0.1)
                .on_release(Message::SeekRelease(fullscreen_vid.id)),
                // Control buttons
                row![
                    button(
                        text(if fullscreen_vid.video.paused() {
                            ">"
                        } else {
                            "||"
                        })
                        .size(12)
                    )
                    .on_press(Message::TogglePause(fullscreen_vid.id))
                    .padding(8)
                    .width(Length::Shrink)
                    .height(Length::Shrink),
                    button(
                        text(if fullscreen_vid.video.looping() {
                            "↻"
                        } else {
                            "→"
                        })
                        .size(12)
                    )
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
        ],
    )
    .style(|_theme: &Theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let fullscreen_stack = stack![video_player, overlay];

    mouse_area(fullscreen_stack)
        .on_enter(Message::MediaHoverChanged(fullscreen_vid.id, true))
        .on_exit(Message::MediaHoverChanged(fullscreen_vid.id, false))
        .into()
}

/// Render the fullscreen view for a single photo.
fn render_fullscreen_photo<'a>(_app: &'a App, photo: &'a PhotoInstance) -> Element<'a, Message> {
    let photo_view = container(image(&photo.handle).content_fit(iced::ContentFit::Contain))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    let overlay = container(
        column![
            // Top bar with filename and close button
            row![
                text(&photo.filename).size(14).color(Color::WHITE),
                container("").width(Length::Fill),
                button(text("X").size(20))
                    .on_press(Message::ToggleFullscreen(photo.id))
                    .padding(5)
                    .width(Length::Shrink)
                    .height(Length::Shrink)
            ]
            .padding(10),
            // Center spacer
            container("").height(Length::Fill),
            // Bottom controls
            row![button(text("V").size(12))
                .on_press(Message::ToggleFullscreen(photo.id))
                .padding(8)
                .width(Length::Shrink)
                .height(Length::Shrink),]
            .spacing(5)
            .align_y(alignment::Vertical::Center)
            .padding(10)
        ],
    )
    .style(|_theme: &Theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let fullscreen_stack = stack![photo_view, overlay];

    mouse_area(fullscreen_stack)
        .on_enter(Message::MediaHoverChanged(photo.id, true))
        .on_exit(Message::MediaHoverChanged(photo.id, false))
        .into()
}

/// Render the bottom control bar.
fn render_controls_bar<'a>(app: &'a App) -> Element<'a, Message> {
    let video_count = app
        .media
        .iter()
        .filter(|m| matches!(m, MediaItem::Video(_)))
        .count();
    let photo_count = app
        .media
        .iter()
        .filter(|m| matches!(m, MediaItem::Photo(_)))
        .count();

    let count_text = match (video_count, photo_count) {
        (0, 0) => "No media".to_string(),
        (v, 0) => format!("{} video{}", v, if v == 1 { "" } else { "s" }),
        (0, p) => format!("{} photo{}", p, if p == 1 { "" } else { "s" }),
        (v, p) => format!(
            "{} video{}, {} photo{}",
            v,
            if v == 1 { "" } else { "s" },
            p,
            if p == 1 { "" } else { "s" }
        ),
    };

    container(
        row![
            button(text("<").size(16))
                .on_press(Message::DecreaseColumns)
                .padding(5),
            text(format!("Grid: {} columns", app.grid_columns)).size(14),
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
            text(count_text).size(12),
        ]
        .spacing(10)
        .align_y(alignment::Vertical::Center),
    )
    .padding(5)
    .width(Length::Fill)
    .into()
}
