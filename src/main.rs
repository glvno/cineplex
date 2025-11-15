mod app;
mod cache;
mod loader;
mod message;
mod state;
mod ui;

use state::App;

fn main() -> iced::Result {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    iced::application("Cineplex", App::update, App::view)
        .subscription(App::subscription)
        .run()
}
