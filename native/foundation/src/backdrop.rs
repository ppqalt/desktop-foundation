//! Decode, downsample and blur once; no wallpaper worker or image subprocess.
use crate::{Result, invalid, state, theme_render};
use image::{DynamicImage, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

pub fn hash(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn decode(path: &Path) -> Result<DynamicImage> {
    let native = ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .map_err(|e| invalid(format!("Invalid wallpaper: {e}")));
    if native.is_err()
        && path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ["jxl", "heic", "heif"].contains(&ext.to_ascii_lowercase().as_str()))
    {
        return crate::media::first_frame(path);
    }
    native
}
/// Convert expanded image formats once for consumers with narrower decoder support.
/// The source remains authoritative for hashing, selection and rollback.
pub fn compatible_image(path: &Path) -> Result<PathBuf> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ![
        "avif", "tga", "qoi", "pnm", "pbm", "pgm", "ppm", "pam", "ico", "hdr", "exr", "ff", "jxl",
        "heic", "heif",
    ]
    .contains(&extension.as_str())
    {
        return Ok(path.to_owned());
    }
    let cache =
        state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/wallpaper-compatible");
    state::private_directory(&cache)?;
    let output = cache.join(format!("{}-v1.png", hash(path)?));
    if output.is_symlink() {
        return Err(invalid("Wallpaper conversion cache must not be a symlink"));
    }
    if output.is_file() && decode(&output).is_ok() {
        return Ok(output);
    }
    let mut bytes = Cursor::new(Vec::new());
    let source = decode(path)?;
    let source = match source {
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
            DynamicImage::ImageRgba8(source.to_rgba8())
        }
        source => source,
    };
    source
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|e| invalid(format!("Wallpaper conversion failed: {e}")))?;
    state::atomic(&output, &bytes.into_inner(), 0o600)?;
    Ok(output)
}
pub fn prepare(image: &Path, mode: &str) -> Result<PathBuf> {
    if !["fill", "fit", "center", "tile"].contains(&mode) {
        return Err(invalid("Invalid wallpaper scaling mode"));
    }
    let cache = state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/overview");
    state::private_directory(&cache)?;
    let sizing = if ["fill", "fit"].contains(&mode) {
        "scaled"
    } else {
        "original"
    };
    let key = format!(
        "{:x}",
        Sha256::digest(format!("{}:{sizing}:rust-blur24-v2", hash(image)?).as_bytes())
    );
    let output = cache.join(format!("{key}.png"));
    if output.is_symlink() {
        return Err(invalid("Overview cache must not be a symlink"));
    }
    if output.is_file() && decode(&output).is_ok() {
        return Ok(output);
    }
    let source = decode(image)?;
    let source = if sizing == "scaled" && (source.width() > 1920 || source.height() > 1920) {
        source.thumbnail(1920, 1920)
    } else {
        source
    };
    // Keep the legacy radius, now using the native image library's Gaussian blur.
    let blurred = DynamicImage::ImageRgb8(source.to_rgb8()).blur(24.0);
    let mut bytes = Cursor::new(Vec::new());
    blurred
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|e| invalid(e.to_string()))?;
    state::atomic(&output, &bytes.into_inner(), 0o644)?;
    Ok(output)
}
pub fn prepare_legacy(image: &Path, mode: &str) -> Result<()> {
    let image = prepare(image, mode)?;
    theme_render::write_json(
        &image.parent().expect("cache parent").join("backdrop.json"),
        &serde_json::json!({"image":state::file_uri(&image)?,"mode":mode}),
    )
}
