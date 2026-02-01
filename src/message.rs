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
    ToggleFullscreen(usize),
    // Grid controls
    IncreaseColumns,
    DecreaseColumns,
    // File loading
    BrowseFile,
    ClearCache,
    FileDropped(PathBuf),
    EventOccurred(Event),
}
