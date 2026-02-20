use std::time::Duration;
use crate::gst_logger;

/// Perform a seek operation with timing instrumentation.
///
/// IMPORTANT: Does NOT query position after seek to avoid blocking the UI thread.
/// Position will be updated by the background position thread.
pub fn synchronized_seek(
    video_id: usize,
    video: &mut iced_video_player::Video,
    duration: Duration,
    accurate: bool,
) -> Result<(), iced_video_player::Error> {
    let start = gst_logger::log_seek_start(video_id, duration, accurate);

    let result = video.seek(duration, accurate);

    match &result {
        Ok(_) => {
            // Don't query position here - it can block for 100ms+ on macOS CoreAudio
            // Position will be updated by background position thread
            gst_logger::log_seek_complete_no_position(video_id, start);
        }
        Err(e) => {
            gst_logger::log_seek_error(video_id, &e.to_string(), start);
        }
    }

    result
}

/// Set pause state with timing instrumentation.
pub fn synchronized_set_paused(video_id: usize, video: &mut iced_video_player::Video, paused: bool) {
    let thread_id = std::thread::current().id();
    let start = gst_logger::log_pause_toggle_start(video_id, paused, thread_id);

    video.set_paused(paused);

    gst_logger::log_pause_toggle_complete(video_id, paused, start);
}
