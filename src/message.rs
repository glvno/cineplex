use iced::Event;
use std::path::PathBuf;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Message {
    // Video-specific messages
    TogglePause(usize),
    ToggleLoop(usize),
    Seek(usize, f64),
    SeekRelease(usize),
    EndOfStream(usize),
    NewFrame(usize),
    ToggleMute(usize),
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
    FileDropped(PathBuf),
    EventOccurred(Event),
    LoadInitialFiles(Vec<PathBuf>),
}
