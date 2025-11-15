use std::time::Duration;

/// Perform a seek operation without serialization.
/// Uses inaccurate seeks to reduce GStreamer mutex contention.
pub fn synchronized_seek(
    video: &mut iced_video_player::Video,
    duration: Duration,
    _accurate: bool,
) -> Result<(), iced_video_player::Error> {
    // Always use inaccurate seeks (false) to minimize GStreamer contention
    // Accurate seeks cause FLUSH_START events that can deadlock with other operations
    video.seek(duration, false)
}

/// Set pause state without additional synchronization.
pub fn synchronized_set_paused(video: &mut iced_video_player::Video, paused: bool) {
    video.set_paused(paused);
}
