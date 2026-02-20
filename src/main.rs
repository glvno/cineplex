mod app;
mod bus_monitor;
mod bus_watcher;
mod gst_logger;
mod loader;
mod message;
mod position_poller;
mod position_thread;
mod state;
mod sync;
mod ui;
mod watchdog;

use iced::Task;
use message::Message;
use state::App;
use std::path::PathBuf;

fn main() -> iced::Result {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    // Register HEIC/HEIF decoder hooks for the image crate
    libheif_rs::integration::image::register_all_decoding_hooks();

    // Collect initial files from command-line arguments
    let initial_files = collect_initial_files();

    iced::application("Cineplex", App::update, App::view)
        .subscription(App::subscription)
        .run_with(move || {
            let task = if initial_files.is_empty() {
                Task::none()
            } else {
                Task::done(Message::LoadInitialFiles(initial_files))
            };
            (App::default(), task)
        })
}

/// Collect media files from command-line arguments.
/// If a directory is provided, returns all supported media files in that directory.
/// If individual files are provided, returns those files.
/// All paths are canonicalized to absolute paths.
fn collect_initial_files() -> Vec<PathBuf> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();

    for arg in args {
        let path = PathBuf::from(&arg);
        // Canonicalize to handle relative paths
        let path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => continue, // Skip invalid paths
        };

        if path.is_dir() {
            // Read directory and collect supported media files
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut dir_files: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && loader::is_supported_media_file(p))
                    .collect();
                // Sort alphabetically for predictable ordering
                dir_files.sort();
                files.extend(dir_files);
            }
        } else if path.is_file() && loader::is_supported_media_file(&path) {
            files.push(path);
        }
    }

    files
}
