//! Dedicated background thread for GStreamer position queries.
//!
//! Position queries can block for 100ms+ when CoreAudio mutexes are held,
//! so we perform them in a background thread to avoid blocking the UI.

use gstreamer::prelude::*;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Message sent from position thread to main thread
#[derive(Debug, Clone)]
pub struct PositionUpdate {
    pub video_id: usize,
    pub position: f64,
}

/// Spawns a background thread that queries positions for all videos at 60Hz
pub fn spawn_position_thread(
    videos: Vec<(usize, gstreamer::Pipeline)>,
) -> mpsc::Receiver<PositionUpdate> {
    let (tx, rx) = mpsc::channel();

    thread::Builder::new()
        .name("gst-position-query".to_string())
        .spawn(move || {
            log::info!(
                "Position query thread started, monitoring {} videos",
                videos.len()
            );

            loop {
                // Query position for each video
                for (video_id, pipeline) in &videos {
                    let thread_id = std::thread::current().id();
                    let start = crate::gst_logger::log_position_query_start(*video_id, thread_id);

                    if let Some(position) = pipeline.query_position::<gstreamer::ClockTime>() {
                        // Convert nanoseconds to seconds with full precision
                        let position_secs = position.nseconds() as f64 / 1_000_000_000.0;

                        crate::gst_logger::log_position_query_complete(
                            *video_id,
                            Duration::from_secs_f64(position_secs),
                            start,
                        );

                        // Send update to main thread
                        if tx
                            .send(PositionUpdate {
                                video_id: *video_id,
                                position: position_secs,
                            })
                            .is_err()
                        {
                            log::warn!("Main thread disconnected, stopping position thread");
                            return;
                        }
                    }
                }

                // Run at 60Hz (every ~16.67ms)
                thread::sleep(Duration::from_millis(16));
            }
        })
        .expect("Failed to spawn position query thread");

    rx
}

