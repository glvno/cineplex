//! GStreamer operation logging and timing instrumentation.
//!
//! This module provides detailed logging for all GStreamer operations to help
//! detect and debug potential deadlocks and performance issues.

use std::time::{Duration, Instant};
use std::thread::ThreadId;

/// Log categories for filtering
enum LogCategory {
    PositionQuery,
    Seek,
    Pause,
}

impl LogCategory {
    fn as_str(&self) -> &'static str {
        match self {
            LogCategory::PositionQuery => "POSITION_QUERY",
            LogCategory::Seek => "SEEK",
            LogCategory::Pause => "PAUSE",
        }
    }
}

/// Log the start of a position query
pub fn log_position_query_start(video_id: usize, thread_id: ThreadId) -> Instant {
    log::trace!(
        "[{}] Video {} position query START (thread: {:?})",
        LogCategory::PositionQuery.as_str(),
        video_id,
        thread_id
    );
    Instant::now()
}

/// Log the completion of a position query with timing
pub fn log_position_query_complete(video_id: usize, position: Duration, start: Instant) {
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();

    if elapsed_ms > 100 {
        log::warn!(
            "[{}] Video {} position query SLOW: {}ms, position={}s",
            LogCategory::PositionQuery.as_str(),
            video_id,
            elapsed_ms,
            position.as_secs_f64()
        );
    } else if elapsed_ms > 10 {
        log::debug!(
            "[{}] Video {} position query: {}ms, position={}s",
            LogCategory::PositionQuery.as_str(),
            video_id,
            elapsed_ms,
            position.as_secs_f64()
        );
    } else {
        log::trace!(
            "[{}] Video {} position query: {}ms, position={}s",
            LogCategory::PositionQuery.as_str(),
            video_id,
            elapsed_ms,
            position.as_secs_f64()
        );
    }
}

/// Log the start of a seek operation
pub fn log_seek_start(video_id: usize, target: Duration, accurate: bool) -> Instant {
    log::info!(
        "[{}] Video {} seek START: target={}s, accurate={}",
        LogCategory::Seek.as_str(),
        video_id,
        target.as_secs_f64(),
        accurate
    );
    Instant::now()
}

/// Log the completion of a seek operation without querying position (to avoid blocking)
pub fn log_seek_complete_no_position(video_id: usize, start: Instant) {
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();

    if elapsed_ms > 2000 {
        log::error!(
            "[{}] Video {} seek DEADLOCK SUSPECTED: {}ms",
            LogCategory::Seek.as_str(),
            video_id,
            elapsed_ms
        );
    } else if elapsed_ms > 1000 {
        log::warn!(
            "[{}] Video {} seek SLOW: {}ms",
            LogCategory::Seek.as_str(),
            video_id,
            elapsed_ms
        );
    } else {
        log::info!(
            "[{}] Video {} seek COMPLETE: {}ms",
            LogCategory::Seek.as_str(),
            video_id,
            elapsed_ms
        );
    }
}

/// Log a seek error
pub fn log_seek_error(video_id: usize, error: &str, start: Instant) {
    let elapsed_ms = start.elapsed().as_millis();
    log::error!(
        "[{}] Video {} seek ERROR after {}ms: {}",
        LogCategory::Seek.as_str(),
        video_id,
        elapsed_ms,
        error
    );
}

/// Log the start of a pause toggle operation
pub fn log_pause_toggle_start(video_id: usize, paused: bool, thread_id: ThreadId) -> Instant {
    log::debug!(
        "[{}] Video {} pause toggle START: paused={} (thread: {:?})",
        LogCategory::Pause.as_str(),
        video_id,
        paused,
        thread_id
    );
    Instant::now()
}

/// Log the completion of a pause toggle operation
pub fn log_pause_toggle_complete(video_id: usize, paused: bool, start: Instant) {
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();

    if elapsed_ms > 2000 {
        log::error!(
            "[{}] Video {} pause toggle DEADLOCK SUSPECTED: paused={}, {}ms",
            LogCategory::Pause.as_str(),
            video_id,
            paused,
            elapsed_ms
        );
    } else if elapsed_ms > 500 {
        log::warn!(
            "[{}] Video {} pause toggle SLOW: paused={}, {}ms",
            LogCategory::Pause.as_str(),
            video_id,
            paused,
            elapsed_ms
        );
    } else {
        log::debug!(
            "[{}] Video {} pause toggle COMPLETE: paused={}, {}ms",
            LogCategory::Pause.as_str(),
            video_id,
            paused,
            elapsed_ms
        );
    }
}

/// Log video creation
pub fn log_video_created(video_id: usize, path: &str) {
    log::info!("Video created: id={}, path={}", video_id, path);
}

/// Log video destruction
pub fn log_video_destroyed(video_id: usize) {
    log::info!("Video destroyed: id={}", video_id);
}
