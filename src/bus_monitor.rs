//! GStreamer bus monitor for async operations.
//!
//! This module provides a subscription that monitors GStreamer bus messages
//! from all active video pipelines, primarily to detect when async seeks complete.

use gstreamer::prelude::*;
use iced::stream;
use iced::Subscription;
use std::time::Duration;

// Data structure containing all the info needed for the bus monitor subscription
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BusMonitorData {
    video_ids: Vec<usize>,
    videos: Vec<(usize, gstreamer::Pipeline)>,
}

/// Creates a subscription that monitors GStreamer bus messages for all videos.
pub fn bus_monitor_subscription(
    videos: &[(usize, gstreamer::Pipeline)],
) -> Subscription<crate::message::Message> {
    if videos.is_empty() {
        return Subscription::none();
    }

    let video_ids: Vec<usize> = videos.iter().map(|(id, _)| *id).collect();
    let videos: Vec<(usize, gstreamer::Pipeline)> = videos.to_vec();

    Subscription::run_with(
        BusMonitorData {
            video_ids: video_ids.clone(),
            videos: videos.clone(),
        },
        run_bus_monitor,
    )
}

fn run_bus_monitor(
    data: &BusMonitorData,
) -> futures::stream::BoxStream<'static, crate::message::Message> {
    use futures::StreamExt;

    let video_ids = data.video_ids.clone();
    let videos = data.videos.clone();

    stream::channel(100, move |mut output: futures::channel::mpsc::Sender<crate::message::Message>| async move {
        log::info!(
            "Bus monitor started, monitoring {} videos: {:?}",
            videos.len(),
            video_ids
        );

        loop {
            // Check all video buses for messages
            for (video_id, pipeline) in &videos {
                    if let Some(bus) = pipeline.bus() {
                        // Non-blocking check for messages (timeout = 0)
                        while let Some(msg) = bus.timed_pop(gstreamer::ClockTime::ZERO) {
                            use gstreamer::MessageView;

                            match msg.view() {
                                MessageView::AsyncDone(_) => {
                                    log::info!(
                                        "Bus monitor: ASYNC_DONE received for video_id={}",
                                        video_id
                                    );
                                    let _ = output.try_send(crate::message::Message::SeekComplete(*video_id));
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

            // Small delay before next poll (16ms = ~60Hz)
            tokio::time::sleep(Duration::from_millis(16)).await;
        }
    })
    .boxed()
}
