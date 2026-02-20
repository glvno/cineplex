//! Position polling for videos.
//!
//! This module provides position update functionality that's decoupled from
//! frame rendering to avoid blocking the UI thread on every frame.

use iced::Subscription;
use std::time::Duration;

/// Create a subscription for position updates
/// This polls position at a fixed rate (10Hz) independent of frame rate
pub fn position_update_subscription() -> Subscription<crate::message::Message> {
    iced::time::every(Duration::from_millis(100))
        .map(|_| crate::message::Message::PositionTick)
}
