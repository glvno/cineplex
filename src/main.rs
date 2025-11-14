mod app;
mod cache;
mod loader;
mod message;
mod state;
mod ui;

use state::App;

fn main() -> iced::Result {
    iced::application("Cineplex", App::update, App::view)
        .subscription(App::subscription)
        .run()
}
