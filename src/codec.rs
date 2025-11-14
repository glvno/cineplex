use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::cache;

/// Detect the video codec using ffprobe.
/// Note: This runs on the UI thread, so it may cause brief hangs for large files.
pub fn get_video_codec(path: &Path) -> Option<String> {
    eprintln!("Running ffprobe to detect codec...");
    let start = Instant::now();

    let result = match Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name")
        .arg("-of")
        .arg("default=noprint_wrappers=1")
        .arg(path)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .lines()
                    .find(|line| line.starts_with("codec_name="))
                    .map(|line| line.trim_start_matches("codec_name=").to_string())
            } else {
                eprintln!("ffprobe failed for {:?}", path);
                None
            }
        }
        Err(e) => {
            eprintln!("Failed to run ffprobe: {}", e);
            None
        }
    };

    let elapsed = start.elapsed();
    eprintln!(
        "ffprobe took {:.2}s, result: {:?}",
        elapsed.as_secs_f64(),
        result
    );
    result
}

/// Determine if a video file needs conversion based on its codec.
/// Only converts H.264 and MPEG2 which have known NV12 conversion issues.
pub fn should_convert(path: &Path) -> bool {
    match get_video_codec(path) {
        Some(codec) => {
            eprintln!("Detected codec: {}", codec);
            // Convert H.264 and files we know have issues
            matches!(codec.as_str(), "h264" | "mpeg2video")
        }
        None => {
            // Fallback: if codec detection fails, convert MOV and MP4 files as a safety measure
            // (they're more likely to have codec issues)
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let should_fallback_convert =
                matches!(ext, "mov" | "MOV" | "mp4" | "MP4" | "m4v" | "M4V");
            eprintln!(
                "Codec detection failed, fallback convert for .{}: {}",
                ext, should_fallback_convert
            );
            should_fallback_convert
        }
    }
}

/// Convert a video file in the background using ffmpeg.
/// Converts to VP9/WebM format with NV12 output.
pub fn convert_video_background(original_path: &Path, _video_id: usize) {
    // Get cache directory
    let cache_dir = match cache::get_cache_dir() {
        Some(dir) => {
            let _ = std::fs::create_dir_all(&dir);
            dir
        }
        None => return,
    };

    // Create a deterministic filename based on the original file path
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    original_path.hash(&mut hasher);
    let hash = hasher.finish();
    let converted_path = cache_dir.join(format!("converted_{:x}.webm", hash));
    let temp_path = cache_dir.join(format!("converted_{:x}.webm.tmp", hash));
    let marker_path = cache_dir.join(format!("converted_{:x}.webm.done", hash));

    eprintln!("Starting VP9 background conversion");
    eprintln!("Source: {:?}", original_path);
    eprintln!("Temp: {:?}", temp_path);
    eprintln!("Final: {:?}", converted_path);

    // Run ffmpeg conversion to temp file - VP9 with fast preset
    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(original_path)
        .arg("-c:v")
        .arg("libvpx-vp9")
        .arg("-preset")
        .arg("fast")
        .arg("-b:v")
        .arg("0")
        .arg("-crf")
        .arg("23")
        .arg("-c:a")
        .arg("libopus")
        .arg("-b:a")
        .arg("128k")
        .arg("-f")
        .arg("webm")
        .arg("-y")
        .arg(&temp_path)
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                eprintln!("VP9 conversion successful, moving temp to final");
                // Move temp file to final location
                if std::fs::rename(&temp_path, &converted_path).is_ok() {
                    eprintln!("Successfully renamed temp file to final path");
                    // Create marker file to signal completion
                    let _ = std::fs::write(&marker_path, b"done");
                    eprintln!("VP9 conversion complete for {:?}", original_path);
                } else {
                    eprintln!("Failed to rename temp file!");
                }
            } else {
                eprintln!("ffmpeg conversion failed!");
                let stderr = String::from_utf8_lossy(&out.stderr);
                eprintln!("ffmpeg stderr: {}", stderr);
                let _ = std::fs::remove_file(&temp_path);
            }
        }
        Err(e) => {
            eprintln!("Failed to execute ffmpeg: {}", e);
            let _ = std::fs::remove_file(&temp_path);
        }
    }
}
