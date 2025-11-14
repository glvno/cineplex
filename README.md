# Cineplex

The world's least worst video multiplexer, written in Rust.

https://github.com/user-attachments/assets/41e884e9-8c23-474d-8b6a-930d4cc0c1ff


### Prerequisites

- Rust 2024 edition (or later)
- ffmpeg (for video conversion - see below)

### Usage

```bash
cargo run --release
```

### Known Issues

Due to an upstream bug, certain H.264 videos fail to render in spectacular fashion.

As a temporary workaround, affected videos are automatically converted to a working format, and the converted videos are cached in `$HOME/.cineplex_cache/`, which can be cleared at will with the "Clear cache" button in the bottom bar of Cineplex.

These conversions take an annoyingly long time, but they happen in the background and do not block playback of other videos.
