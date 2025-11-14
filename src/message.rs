use std::path::PathBuf;
use iced::Event;

#[derive(Clone, Debug)]
pub enum Message {
    TogglePause(usize),
    ToggleLoop(usize),
    Seek(usize, f64),
    SeekRelease(usize),
    EndOfStream(usize),
    NewFrame(usize),
    RemoveVideo(usize),
    VideoHoverChanged(usize, bool),
    ToggleMute(usize),
    ToggleFullscreen(usize),
    IncreaseColumns,
    DecreaseColumns,
    BrowseFile,
    ClearCache,
    FileDropped(PathBuf),
    EventOccurred(Event),
    ConversionStarted(PathBuf, usize),
    ConversionComplete(PathBuf, PathBuf, usize), // original, converted, video_id
    ConversionFailed(PathBuf, String, usize),
}
