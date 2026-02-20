//! Dedicated background thread for GStreamer bus monitoring.
//!
//! This is more reliable than subscriptions because the thread stays alive
//! and continuously monitors the bus without interruption.

use gstreamer::prelude::*;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Message sent from bus watcher thread to main thread
#[derive(Debug, Clone)]
pub enum BusEvent {
    SeekComplete(usize),
}

/// Spawns a background thread that monitors GStreamer buses for all videos
pub fn spawn_bus_watcher(
    videos: Vec<(usize, gstreamer::Pipeline)>,
) -> mpsc::Receiver<BusEvent> {
    let (tx, rx) = mpsc::channel();

    thread::Builder::new()
        .name("gst-bus-watcher".to_string())
        .spawn(move || {
            log::info!("Bus watcher thread started, monitoring {} videos", videos.len());

            loop {
                // Check all video buses for messages
                for (video_id, pipeline) in &videos {
                    if let Some(bus) = pipeline.bus() {
                        // Non-blocking check for messages
                        while let Some(msg) = bus.timed_pop(gstreamer::ClockTime::ZERO) {
                            use gstreamer::MessageView;

                            match msg.view() {
                                MessageView::AsyncDone(_) => {
                                    log::info!(
                                        "Bus watcher: ASYNC_DONE received for video_id={}",
                                        video_id
                                    );
                                    // Send event to main thread
                                    if tx.send(BusEvent::SeekComplete(*video_id)).is_err() {
                                        log::warn!("Main thread disconnected, stopping bus watcher");
                                        return;
                                    }
                                }
                                MessageView::Error(err) => {
                                    log::error!(
                                        "GStreamer error on video_id={}: {} (debug: {:?})",
                                        video_id,
                                        err.error(),
                                        err.debug()
                                    );
                                }
                                MessageView::Warning(warn) => {
                                    log::warn!(
                                        "GStreamer warning on video_id={}: {} (debug: {:?})",
                                        video_id,
                                        warn.error(),
                                        warn.debug()
                                    );
                                }
                                MessageView::Eos(_) => {
                                    log::debug!("GStreamer EOS on video_id={}", video_id);
                                }
                                _ => {
                                    // Ignore other messages
                                }
                            }
                        }
                    }
                }

                // Small delay to avoid busy-waiting (60Hz polling)
                thread::sleep(Duration::from_millis(16));
            }
        })
        .expect("Failed to spawn bus watcher thread");

    rx
}
