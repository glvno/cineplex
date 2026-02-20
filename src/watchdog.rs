//! Watchdog thread for detecting UI thread deadlocks.
//!
//! This module provides a background watchdog that monitors the main UI thread
//! for freezes and deadlocks by checking heartbeat signals.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Watchdog for detecting deadlocks in the UI thread
pub struct Watchdog {
    last_heartbeat: Arc<AtomicU64>,
}

impl Watchdog {
    /// Spawn a new watchdog thread
    pub fn spawn() -> Self {
        let last_heartbeat = Arc::new(AtomicU64::new(current_timestamp_ms()));
        let heartbeat_clone = last_heartbeat.clone();

        thread::Builder::new()
            .name("watchdog".to_string())
            .spawn(move || {
                log::debug!("Watchdog thread started");

                loop {
                    thread::sleep(Duration::from_secs(3));

                    let last = heartbeat_clone.load(Ordering::Relaxed);
                    let now = current_timestamp_ms();
                    let elapsed = now.saturating_sub(last);

                    if elapsed > 5_000 {
                        // 5 seconds without heartbeat
                        log::error!(
                            "DEADLOCK DETECTED: UI thread hasn't responded in {}ms",
                            elapsed
                        );
                        log::error!("Application may be frozen. Check GStreamer operations.");

                        // Continue monitoring to report ongoing deadlock duration
                        if elapsed > 30_000 {
                            // 30 seconds
                            log::error!(
                                "CRITICAL: UI thread frozen for {}s. Application is likely deadlocked.",
                                elapsed / 1000
                            );
                        }
                    } else if elapsed > 2_000 {
                        // 2 seconds - warning
                        log::warn!(
                            "UI thread slow: {}ms since last heartbeat (warning threshold: 2000ms)",
                            elapsed
                        );
                    } else {
                        log::trace!("Watchdog heartbeat OK ({}ms)", elapsed);
                    }
                }
            })
            .expect("Failed to spawn watchdog thread");

        log::info!("Watchdog initialized (deadlock detection enabled)");

        Watchdog { last_heartbeat }
    }

    /// Signal that the UI thread is alive
    pub fn heartbeat(&self) {
        self.last_heartbeat
            .store(current_timestamp_ms(), Ordering::Relaxed);
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
