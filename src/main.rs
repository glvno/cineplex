mod app;
mod cache;
mod loader;
mod message;
mod state;
mod sync;
mod ui;

use state::App;

fn main() -> iced::Result {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    // Register HEIC/HEIF decoder hooks for the image crate
    libheif_rs::integration::image::register_all_decoding_hooks();

    iced::application("Cineplex", App::update, App::view)
        .subscription(App::subscription)
        .run()
}
