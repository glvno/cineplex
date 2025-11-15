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

/// Set pause state by directly calling set_paused.
///
/// NOTE: This is a temporary placeholder. The underlying issue is that GStreamer's OSX
/// audio sink deadlocks when set_state() is called from the main thread. The proper
/// fix would require either:
/// 1. Using async state changes via GStreamer's bus
/// 2. Moving state changes to a background thread (requires Audio to be Clone)
/// 3. Using a non-blocking pause mechanism
///
/// For now, we call set_paused directly and hope the timing works out.
pub fn synchronized_set_paused(video: &mut iced_video_player::Video, paused: bool) {
    video.set_paused(paused);
}
