//! One-shot Matugen palettes mapped onto the existing graphite semantics.
//! Publication/reloads and their transaction lock remain with the caller.
use crate::{Result, invalid, process, state};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

pub const VERSION: &str = "graphite-v1";
const METADATA_LIMIT: u64 = 64 * 1024;
const ROLES: [&str; 15] = [
    "background",
    "elevated",
    "foreground",
    "muted",
    "subtle",
    "border",
    "accent",
    "selected",
    "selectionBorder",
    "iconTile",
    "hover",
    "error",
    "accentStrong",
    "windowActive",
    "windowInactive",
];
const SURFACES: [&str; 9] = [
    "background",
    "elevated",
    "iconTile",
    "hover",
    "selected",
    "border",
    "selectionBorder",
    "windowActive",
    "windowInactive",
];
const TEXT_SURFACES: [&str; 3] = ["background", "elevated", "selected"];
const FILLED_SURFACES: [&str; 5] = ["background", "elevated", "iconTile", "hover", "selected"];
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
pub struct Generation {
    pub palette: Value,
    pub cached: bool,
}

fn color<'a>(palette: &'a Value, role: &str) -> Result<&'a str> {
    palette[role]
        .as_str()
        .ok_or_else(|| invalid(format!("Palette color is missing: {role}")))
}

fn hex_color(value: &str, digits: usize) -> bool {
    value.len() == digits + 1
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn rgb(color: &str) -> Result<[f64; 3]> {
    if !hex_color(color, 6) {
        return Err(invalid("Invalid palette color"));
    }
    let mut result = [0.; 3];
    for (index, component) in result.iter_mut().enumerate() {
        let offset = index * 2 + 1;
        *component = u8::from_str_radix(&color[offset..offset + 2], 16)
            .map_err(|_| invalid("Invalid palette color"))? as f64
            / 255.;
    }
    Ok(result)
}

fn hexcolor(values: [f64; 3]) -> String {
    let [r, g, b] = values.map(|value| (value.clamp(0., 1.) * 255.).round_ties_even() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

// Keep the Python colorsys HLS evaluation order, including ties-to-even color
// quantization, so existing graphite-v1 caches and renderers retain their colors.
fn rgb_to_hls([r, g, b]: [f64; 3]) -> [f64; 3] {
    let maximum = r.max(g).max(b);
    let minimum = r.min(g).min(b);
    let lightness = (minimum + maximum) / 2.;
    if minimum == maximum {
        return [0., lightness, 0.];
    }
    let range = maximum - minimum;
    let saturation = if lightness <= 0.5 {
        range / (maximum + minimum)
    } else {
        range / (2. - maximum - minimum)
    };
    let rc = (maximum - r) / range;
    let gc = (maximum - g) / range;
    let bc = (maximum - b) / range;
    let hue = if r == maximum {
        bc - gc
    } else if g == maximum {
        2. + rc - bc
    } else {
        4. + gc - rc
    };
    [(hue / 6.).rem_euclid(1.), lightness, saturation]
}

fn hls_to_rgb([hue, lightness, saturation]: [f64; 3]) -> [f64; 3] {
    if saturation == 0. {
        return [lightness; 3];
    }
    let m2 = if lightness <= 0.5 {
        lightness * (1. + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let m1 = 2. * lightness - m2;
    let component = |hue: f64| {
        let hue = hue.rem_euclid(1.);
        if hue < 1. / 6. {
            m1 + (m2 - m1) * hue * 6.
        } else if hue < 0.5 {
            m2
        } else if hue < 2. / 3. {
            m1 + (m2 - m1) * (2. / 3. - hue) * 6.
        } else {
            m1
        }
    };
    [
        component(hue + 1. / 3.),
        component(hue),
        component(hue - 1. / 3.),
    ]
}

fn mix(base: &str, tint: &str, amount: f64, saturation: Option<f64>) -> Result<String> {
    let a = rgb(base)?;
    let b = rgb(tint)?;
    let mut values = std::array::from_fn(|index| a[index] * (1. - amount) + b[index] * amount);
    if let Some(cap) = saturation {
        let [h, l, s] = rgb_to_hls(values);
        values = hls_to_rgb([h, l, s.min(cap)]);
    }
    Ok(hexcolor(values))
}

fn luminance(color: &str) -> Result<f64> {
    let values = rgb(color)?.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    Ok(values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722)
}

fn contrast(a: &str, b: &str) -> Result<f64> {
    let x = luminance(a)?;
    let y = luminance(b)?;
    Ok((x.max(y) + 0.05) / (x.min(y) + 0.05))
}

fn minimum_contrast(palette: &Value, role: &str) -> Result<f64> {
    TEXT_SURFACES
        .iter()
        .try_fold(f64::INFINITY, |minimum, surface| {
            Ok(minimum.min(contrast(color(palette, role)?, color(palette, surface)?)?))
        })
}

fn read_json(path: &Path) -> Result<Value> {
    // Metadata is small; wallpaper hashing below deliberately has no size cap.
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid(format!(
            "Palette metadata must be a regular file: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?
        .take(METADATA_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_LIMIT {
        return Err(invalid("Palette metadata is too large"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn validate_roles(palette: &Value) -> Result<()> {
    if !palette.is_object() {
        return Err(invalid("Palette must be a JSON object"));
    }
    for role in ROLES {
        rgb(color(palette, role)?)?;
    }
    let source = palette["source"]
        .as_str()
        .ok_or_else(|| invalid("Palette source is missing"))?;
    if source.is_empty() || source.contains('\0') {
        return Err(invalid("Invalid palette source"));
    }
    if !hex_color(color(palette, "scrim")?, 8) {
        return Err(invalid("Invalid palette scrim"));
    }
    Ok(())
}

fn baseline(root: &Path) -> Result<Value> {
    let palette = read_json(&root.join("theme/fallback/semantic.json"))?;
    validate_roles(&palette)?;
    Ok(palette)
}

fn validate_constraints(palette: &Value) -> Result<()> {
    for role in SURFACES {
        let [_, lightness, saturation] = rgb_to_hls(rgb(color(palette, role)?)?);
        let lightness_limit = if FILLED_SURFACES.contains(&role) {
            0.37
        } else {
            0.65
        };
        if lightness >= lightness_limit || saturation >= 0.26 {
            return Err(invalid(format!(
                "Graphite surface constraint failed: {role}"
            )));
        }
    }
    for (role, required) in [
        ("foreground", 7.),
        ("muted", 3.),
        ("accent", 4.5),
        ("accentStrong", 4.5),
    ] {
        if minimum_contrast(palette, role)? < required {
            return Err(invalid(format!(
                "Palette contrast constraint failed: {role}"
            )));
        }
    }
    Ok(())
}

fn validate_cached(palette: &Value, fallback: &Value, digest: &str) -> Result<()> {
    validate_roles(palette)?;
    if palette["policy"] != VERSION {
        return Err(invalid(
            "Cached palette policy mismatch; current theme left intact",
        ));
    }
    if palette["wallpaperHash"] != digest {
        return Err(invalid(
            "Cached palette wallpaper hash mismatch; current theme left intact",
        ));
    }
    for role in ["foreground", "muted", "subtle", "error", "scrim"] {
        if palette[role] != fallback[role] {
            return Err(invalid(format!(
                "Cached neutral palette role differs: {role}"
            )));
        }
    }
    validate_constraints(palette)
}

pub fn derive(root: &Path, material: &Value, source: &str, digest: &str) -> Result<Value> {
    let mut palette = baseline(root)?;
    let primary = color(material, "primary")?;
    let secondary = color(material, "secondary")?;
    // Validate before updating even an in-memory output; malformed colors cannot
    // become zero/black fallbacks or partially published theme files.
    rgb(primary)?;
    rgb(secondary)?;
    palette["source"] = json!(source);
    palette["wallpaperHash"] = json!(digest);
    palette["policy"] = json!(VERSION);
    for (role, base, amount, cap) in [
        ("background", "#171b22", 0.06, 0.20),
        ("elevated", "#222832", 0.10, 0.20),
        ("iconTile", "#172029", 0.12, 0.22),
        ("hover", "#222b38", 0.12, 0.22),
        ("selected", "#293649", 0.17, 0.24),
        ("border", "#38414e", 0.18, 0.22),
        ("selectionBorder", "#485a73", 0.30, 0.25),
        ("windowActive", "#485669", 0.28, 0.25),
        ("windowInactive", "#2b323d", 0.10, 0.20),
    ] {
        palette[role] = json!(mix(base, primary, amount, Some(cap))?);
    }
    palette["accent"] = json!(primary);
    palette["accentStrong"] = json!(mix(primary, secondary, 0.20, None)?);
    for role in ["accent", "accentStrong"] {
        while minimum_contrast(&palette, role)? < 4.5 {
            let old = color(&palette, role)?;
            let new = mix(old, "#ffffff", 0.10, None)?;
            if new == old {
                return Err(invalid("Could not satisfy accent contrast"));
            }
            palette[role] = json!(new);
        }
    }
    validate_roles(&palette)?;
    validate_constraints(&palette)?;
    Ok(palette)
}

fn material_colors(output: &Value) -> Result<Value> {
    let colors = output["colors"]
        .as_object()
        .ok_or_else(|| invalid("Matugen output has no colors object"))?;
    let mut material = serde_json::Map::new();
    for role in ["primary", "secondary"] {
        let value = if let Some(dark) = colors.get("dark").filter(|dark| !dark.is_null()) {
            dark.get(role).and_then(Value::as_str)
        } else {
            colors
                .get(role)
                .and_then(|v| v.get("dark"))
                .and_then(|v| v.get("color"))
                .and_then(Value::as_str)
        }
        .ok_or_else(|| invalid(format!("Matugen dark color is missing: {role}")))?;
        rgb(value)?;
        material.insert(role.to_owned(), json!(value));
    }
    Ok(Value::Object(material))
}

fn executable() -> Result<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("matugen"))
        .find(|path| {
            path.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
        .ok_or_else(|| invalid("matugen is missing; current theme was left intact"))
}

// Device/inode/size and both timestamps detect writes and atomic replacements
// during this request; this is not a resident filesystem watcher.
#[derive(PartialEq, Eq)]
struct ImageStamp(u64, u64, u64, i64, i64, i64, i64);
impl ImageStamp {
    fn from(metadata: &fs::Metadata) -> Self {
        Self(
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    }
}

fn unchanged_image(path: &Path, stamp: &ImageStamp) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || ImageStamp::from(&metadata) != *stamp {
        return Err(invalid(
            "Wallpaper changed during theme generation; current theme left intact",
        ));
    }
    Ok(())
}

fn file_hash(path: &Path) -> Result<(String, ImageStamp)> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err(invalid("Wallpaper must be a regular file"));
    }
    let stamp = ImageStamp::from(&metadata);
    // Canonical image paths may originally have been symlinks. Refuse a raced
    // replacement with a symlink/FIFO rather than blocking before Matugen starts.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || ImageStamp::from(&opened) != stamp {
        return Err(invalid(
            "Wallpaper changed before hashing; current theme left intact",
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        hasher.update(&buffer[..length]);
    }
    if ImageStamp::from(&file.metadata()?) != stamp {
        return Err(invalid(
            "Wallpaper changed during hashing; current theme left intact",
        ));
    }
    unchanged_image(path, &stamp)?;
    Ok((format!("{:x}", hasher.finalize()), stamp))
}

fn text_path(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("Theme paths must be UTF-8"))
}

struct MatugenConfig(PathBuf);
impl MatugenConfig {
    fn create(parent: &Path) -> Result<Self> {
        loop {
            let directory = parent.join(format!(
                ".matugen-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let temporary = Self(directory);
                    state::atomic(
                        &temporary.0.join("config.toml"),
                        b"[config]\n[templates]\n",
                        0o600,
                    )?;
                    return Ok(temporary);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}
impl Drop for MatugenConfig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn generate(root: &Path, image: &Path) -> Result<Generation> {
    let executable = executable()?;
    let image = if image == Path::new("~") {
        state::home()?
    } else if let Ok(suffix) = image.strip_prefix("~/") {
        state::home()?.join(suffix)
    } else {
        image.to_owned()
    }
    .canonicalize()?;
    let root = root.canonicalize()?;
    let (digest, stamp) = file_hash(&image)?;
    let source = text_path(image.strip_prefix(&root).unwrap_or(&image))?;
    let fallback = baseline(&root)?;
    let cache_base = state::xdg("XDG_CACHE_HOME", ".cache")?;
    if fs::symlink_metadata(&cache_base).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(invalid("Refusing symlink theme cache directory"));
    }
    let managed = cache_base.join("desktop-foundation");
    state::private_directory(&managed)?;
    let cache = managed.join("themes");
    state::private_directory(&cache)?;
    let target = cache.join(format!("{digest}-{VERSION}.json"));
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            let mut palette = read_json(&target)?;
            validate_cached(&palette, &fallback, &digest)?;
            palette["source"] = json!(source);
            unchanged_image(&image, &stamp)?;
            return Ok(Generation {
                palette,
                cached: true,
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    // No caller templates, hooks, polling or persistent Matugen process. The
    // private config is dropped on success, invalid JSON, child failure/timeout.
    let temporary = MatugenConfig::create(&cache)?;
    let args = vec![
        text_path(&executable)?,
        "--config".into(),
        text_path(&temporary.0.join("config.toml"))?,
        "--dry-run".into(),
        "--mode".into(),
        "dark".into(),
        "--json".into(),
        "hex".into(),
        "--source-color-index".into(),
        "0".into(),
        "image".into(),
        text_path(&image)?,
    ];
    let output = process::checked(&args, None, Duration::from_secs(60), true)?;
    let material = material_colors(&serde_json::from_slice(&output.stdout)?)?;
    let palette = derive(&root, &material, &source, &digest)?;
    let mut content = serde_json::to_vec_pretty(&palette)?;
    content.push(b'\n');
    unchanged_image(&image, &stamp)?;
    state::atomic(&target, &content, 0o600)?;
    Ok(Generation {
        palette,
        cached: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_quantization_matches_python_ties_and_hls_edges() {
        assert_eq!(hexcolor([0.5 / 255., 1.5 / 255., 2.5 / 255.]), "#000202");
        assert_eq!(hexcolor([-1., 2., 1.]), "#00ffff");
        assert_eq!(rgb_to_hls([0.25; 3]), [0., 0.25, 0.]);
        assert_eq!(hls_to_rgb([0.8, 0.25, 0.]), [0.25; 3]);
        assert_eq!(
            mix("#171b22", "#98ccf9", 0.06, Some(0.20)).unwrap(),
            "#1f262f"
        );
        for [r, g, b] in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0.1, 0.3, 0.8]] {
            let result = hls_to_rgb(rgb_to_hls([r, g, b]));
            for (actual, expected) in result.into_iter().zip([r, g, b]) {
                assert!((actual - expected).abs() < 1e-14);
            }
        }
        for bad in ["ffffff", "#ff", "#ffffffff", "#f0ffgg", "#f0fƒff"] {
            assert!(rgb(bad).is_err());
        }
    }

    #[test]
    fn matugen_modes_are_explicit_and_malformed_colors_are_rejected() {
        let old = json!({"colors":{"dark":{"primary":"#98ccf9","secondary":"#cdc2d0"}}});
        let modern = json!({"colors":{
            "primary":{"dark":{"color":"#98ccf9"},"light":{"color":"#000000"}},
            "secondary":{"dark":{"color":"#cdc2d0"}}
        }});
        assert_eq!(
            material_colors(&old).unwrap(),
            material_colors(&modern).unwrap()
        );
        assert_eq!(material_colors(&modern).unwrap()["primary"], "#98ccf9");
        for bad in [
            json!({}),
            json!({"colors":[]}),
            json!({"colors":{"dark":{"primary":"invalid","secondary":"#ffffff"}}}),
            json!({"colors":{"primary":{"light":{"color":"#ffffff"}}}}),
        ] {
            assert!(material_colors(&bad).is_err());
        }
    }

    #[test]
    fn cached_palettes_require_matching_metadata_and_graphite_constraints() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fallback = baseline(&root).unwrap();
        let palette = derive(
            &root,
            &json!({"primary":"#ffb599","secondary":"#cdc2d0"}),
            "test",
            "hash",
        )
        .unwrap();
        validate_cached(&palette, &fallback, "hash").unwrap();
        for (role, value) in [
            ("policy", json!("other")),
            ("wallpaperHash", json!("other")),
            ("background", json!("#0000ff")),
            ("accent", json!("#000000")),
            ("foreground", json!("#ffffff")),
            ("muted", json!(false)),
            ("scrim", json!("#ffffff")),
            ("source", json!(null)),
        ] {
            let mut bad = palette.clone();
            bad[role] = value;
            assert!(
                validate_cached(&bad, &fallback, "hash").is_err(),
                "accepted bad {role}"
            );
        }
    }
}
