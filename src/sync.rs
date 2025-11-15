use std::sync::Mutex;
use std::time::Duration;

/// Global lock to prevent concurrent GStreamer state transition deadlocks.
/// When multiple videos seek or change pause state simultaneously, GStreamer can deadlock
/// on internal mutex contention. This lock serializes all such operations across all videos.
pub static GSTREAMER_LOCK: Mutex<()> = Mutex::new(());

/// Helper to safely perform a seek with synchronization.
pub fn synchronized_seek(
    video: &mut iced_video_player::Video,
    duration: Duration,
    accurate: bool,
) -> Result<(), iced_video_player::Error> {
    let _guard = GSTREAMER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    video.seek(duration, accurate)
}

/// Helper to safely set pause state with synchronization.
pub fn synchronized_set_paused(video: &mut iced_video_player::Video, paused: bool) {
    let _guard = GSTREAMER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    video.set_paused(paused);
}
