//! One-shot discovery and cached previews; no polling or resident image process.
use crate::{Result, backdrop, state, theme_render, theme_runtime};
use image::{DynamicImage, ImageFormat, ImageReader, imageops::FilterType};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Cursor,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WallpaperSettings {
    directory: String,
}

fn user_path(value: &str, home: &Path) -> Result<PathBuf> {
    let path = if value == "~" || value == "$HOME" {
        home.to_owned()
    } else if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("$HOME/"))
    {
        home.join(relative)
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() || value.contains('\0') {
        return Err(crate::invalid(
            "Wallpaper directory must be absolute or start with ~/",
        ));
    }
    Ok(path)
}

fn wallpaper_directory(home: &Path, config: &Path) -> Result<PathBuf> {
    let path = config.join("desktop-foundation/wallpapers.toml");
    match fs::read_to_string(&path) {
        Ok(text) => {
            let settings: WallpaperSettings = toml::from_str(&text)
                .map_err(|e| crate::invalid(format!("Invalid wallpaper settings: {e}")))?;
            return user_path(&settings.directory, home);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    // user-dirs.dirs is shell-shaped data. Never execute or expand it as shell.
    let pictures = fs::read_to_string(config.join("user-dirs.dirs"))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                let value = line.trim().strip_prefix("XDG_PICTURES_DIR=")?.trim();
                let value = value.strip_prefix('"')?.strip_suffix('"')?;
                if value.contains(['\\', '"', '`']) {
                    return None;
                }
                user_path(value, home).ok()
            })
        })
        .unwrap_or_else(|| home.join("Pictures"));
    Ok(pictures.join("Wallpapers"))
}

fn preview(path: &Path, cache: &Path) -> Result<PathBuf> {
    let meta = fs::metadata(path)?;
    let mut digest = Sha256::new();
    digest.update(path.as_os_str().as_bytes());
    digest.update(format!(
        "{}:{}:{}:{}:{}:480x288-rounded-v1",
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec()
    ));
    let output = cache.join(format!("{:x}.png", digest.finalize()));
    if output.is_symlink() {
        return Err(crate::invalid("Preview cache must not be a symlink"));
    }
    if output.is_file() && ImageReader::open(&output)?.into_dimensions().is_ok() {
        return Ok(output);
    }
    let mut image = backdrop::decode(path)?
        .resize_to_fill(480, 288, FilterType::Triangle)
        .to_rgba8();
    // Bake the rounded alpha edge once. QML can render an ordinary Image, which
    // also works on software renderers and needs no per-tile effect/layer.
    let radius = 26.0_f32;
    for y in 0..image.height() {
        for x in 0..image.width() {
            let dx =
                (radius - (x as f32 + 0.5).min(image.width() as f32 - x as f32 - 0.5)).max(0.0);
            let dy =
                (radius - (y as f32 + 0.5).min(image.height() as f32 - y as f32 - 0.5)).max(0.0);
            if dx > 0.0 && dy > 0.0 {
                let alpha = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
                image.get_pixel_mut(x, y).0[3] =
                    (image.get_pixel(x, y).0[3] as f32 * alpha).round() as u8;
            }
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|e| crate::invalid(e.to_string()))?;
    state::atomic(&output, &bytes.into_inner(), 0o644)?;
    Ok(output)
}
pub fn catalog(directory: &Path, active: Option<&Path>) -> Result<Value> {
    catalog_with_previews(directory, active, None)
}
pub fn catalog_with_previews(
    directory: &Path,
    active: Option<&Path>,
    cache: Option<&Path>,
) -> Result<Value> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(
                json!({"directory":directory,"available":false,"current":active,"images":[]}),
            );
        }
        Err(e) => return Err(e.into()),
    };
    if let Some(cache) = cache {
        state::private_directory(cache)?;
    }
    let mut images = Vec::new();
    let mut retained = HashSet::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ![
            "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "avif", "tga", "qoi", "pnm",
            "pbm", "pgm", "ppm", "pam", "ico", "hdr", "exr", "ff", "jxl", "heic", "heif",
        ]
        .contains(&extension.as_str())
        {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let dimensions = ImageReader::open(&path)
            .and_then(|r| r.with_guessed_format())
            .ok()
            .and_then(|r| r.into_dimensions().ok())
            .or_else(|| {
                ["jxl", "heic", "heif"]
                    .contains(&extension.as_str())
                    .then(|| {
                        backdrop::decode(&path)
                            .ok()
                            .map(|image| (image.width(), image.height()))
                    })
                    .flatten()
            });
        let Some((width, height)) = dimensions else {
            continue;
        };
        let resolved = match path.canonicalize() {
            Ok(path) => path,
            Err(_) => continue,
        };
        let image = if let Some(cache) = cache {
            match preview(&resolved, cache) {
                Ok(path) => {
                    retained.insert(path.clone());
                    path
                }
                Err(_) => continue,
            }
        } else {
            resolved.clone()
        };
        images.push(json!({"name":name,"path":resolved,"url":state::file_uri(&image)?,"width":width,"height":height,"selected":active.is_some_and(|active|active==resolved)}));
    }
    // Reproducible previews for replaced/deleted images need not accumulate.
    // Unknown files and symlinks are never cleanup candidates.
    if let Some(cache) = cache {
        for entry in fs::read_dir(cache)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.len() == 68
                && name.ends_with(".png")
                && name.as_bytes()[..64].iter().all(|c| c.is_ascii_hexdigit())
                && entry.file_type()?.is_file()
                && entry.metadata()?.uid() == unsafe { libc::getuid() }
                && !retained.contains(&path)
            {
                fs::remove_file(path)?;
            }
        }
    }
    images.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
            .then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
    });
    Ok(json!({"directory":directory,"available":true,"current":active,"images":images}))
}
pub fn list(root: &Path) -> Result<Value> {
    let home = theme_runtime::directory()?;
    let mut selected = None;
    if let Some(revision) = theme_runtime::current(&home)? {
        selected = theme_render::read(&revision.join("metadata.json"))?["input"]
            .as_str()
            .and_then(|input| Path::new(input).canonicalize().ok())
            .filter(|path| !path.starts_with(home.join("revisions")));
        let p = theme_render::read(&revision.join("semantic.json"))?;
        if selected.is_none()
            && let Some(source) = p["source"].as_str()
        {
            let source = PathBuf::from(source);
            selected = if source.is_absolute() {
                source
            } else {
                root.join(source)
            }
            .canonicalize()
            .ok();
        }
    }
    let cache =
        state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/wallpaper-previews");
    let directory =
        wallpaper_directory(&state::home()?, &state::xdg("XDG_CONFIG_HOME", ".config")?)?;
    catalog_with_previews(&directory, selected.as_deref(), Some(&cache))
}

#[cfg(test)]
mod directory_tests {
    use super::*;

    #[test]
    fn honors_override_then_xdg_pictures_then_default() {
        let base =
            std::env::temp_dir().join(format!("foundation-wallpaper-path-{}", std::process::id()));
        fs::create_dir_all(base.join("desktop-foundation")).unwrap();
        let home = Path::new("/home/example");
        assert_eq!(
            wallpaper_directory(home, &base).unwrap(),
            home.join("Pictures/Wallpapers")
        );
        fs::write(
            base.join("user-dirs.dirs"),
            "XDG_PICTURES_DIR=\"$HOME/My Pictures\"\n",
        )
        .unwrap();
        assert_eq!(
            wallpaper_directory(home, &base).unwrap(),
            home.join("My Pictures/Wallpapers")
        );
        let settings = base.join("desktop-foundation/wallpapers.toml");
        fs::write(&settings, "directory = '~/Art'\n").unwrap();
        assert_eq!(wallpaper_directory(home, &base).unwrap(), home.join("Art"));
        fs::write(&settings, "directory = 'relative'\n").unwrap();
        assert!(wallpaper_directory(home, &base).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
