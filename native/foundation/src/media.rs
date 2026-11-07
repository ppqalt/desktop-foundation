//! Bounded, on-demand media inspection and first-frame decoding.
use crate::{Result, invalid, process};
use serde_json::Value;
use std::{path::Path, time::Duration};

fn local_file(path: &Path) -> Result<String> {
    let path = path.canonicalize()?;
    if !path.is_file() {
        return Err(invalid("Wallpaper media must be a regular file"));
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("Wallpaper media path is not valid UTF-8"))
}

/// Inspect actual streams, independently of the file extension.
pub fn probe(path: &Path) -> Result<Value> {
    let file = local_file(path)?;
    let args = [
        "ffprobe",
        "-v",
        "error",
        "-protocol_whitelist",
        "file,pipe",
        "-show_entries",
        "stream=codec_type,codec_name,width,height:format=format_name,duration",
        "-of",
        "json",
        "-i",
        &file,
    ]
    .map(str::to_owned);
    let output = process::checked_capture(&args, Duration::from_secs(10), 1024 * 1024)?;
    let value: Value = serde_json::from_slice(&output.stdout)?;
    let streams = value["streams"]
        .as_array()
        .ok_or_else(|| invalid("Media probe did not return streams"))?;
    Ok(
        serde_json::json!({"streams":streams,"format":value["format"],"hasAudio":streams.iter().any(|s|s["codec_type"] == "audio"),"hasVideo":streams.iter().any(|s|s["codec_type"] == "video")}),
    )
}

/// Decode a single frame without creating a persistent player or audio stream.
pub fn first_frame(path: &Path) -> Result<image::DynamicImage> {
    let file = local_file(path)?;
    let args = [
        "ffmpeg",
        "-v",
        "error",
        "-nostdin",
        "-protocol_whitelist",
        "file,pipe",
        "-i",
        &file,
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-an",
        "-f",
        "image2pipe",
        "-vcodec",
        "png",
        "pipe:1",
    ]
    .map(str::to_owned);
    let output = process::checked_image(&args, Duration::from_secs(20))?;
    image::load_from_memory_with_format(&output.stdout, image::ImageFormat::Png)
        .map_err(|e| invalid(format!("Media frame decode failed: {e}")))
}
