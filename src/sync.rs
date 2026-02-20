use std::time::Duration;

/// Perform a seek operation.
pub fn synchronized_seek(
    video: &mut iced_video_player::Video,
    duration: Duration,
    accurate: bool,
) -> Result<(), iced_video_player::Error> {
    video.seek(duration, accurate)
}

/// Set pause state.
pub fn synchronized_set_paused(video: &mut iced_video_player::Video, paused: bool) {
    video.set_paused(paused);
}
