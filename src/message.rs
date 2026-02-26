use iced::Event;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Message {
    // Video-specific messages
    TogglePause(usize),
    ToggleLoop(usize),
    Seek(usize, f64),
    SeekRelease(usize),
    EndOfStream(usize),
    ToggleMute(usize),
    PositionTick,
    // Shared messages (work for both videos and photos)
    RemoveMedia(usize),
    MediaHoverChanged(usize, bool),
    MouseMoved(usize),
    ToggleFullscreen(usize),
    // UI fade timer
    UiFadeTick,
    // Grid controls
    IncreaseColumns,
    DecreaseColumns,
    // File loading
    BrowseFile,
    EventOccurred(Event),
    LoadInitialFiles(Vec<PathBuf>),
}
