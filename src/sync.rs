use std::time::Duration;
use crate::gst_logger;

/// Perform a seek operation with timing instrumentation.
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
            let actual_position = video.position();
            gst_logger::log_seek_complete(video_id, actual_position, start);
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
