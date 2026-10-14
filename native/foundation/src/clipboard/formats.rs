use super::{Format, MAX_BYTES, MAX_TOTAL, Result, TEXT, invalid};
use std::{collections::BTreeMap, ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

pub(super) fn plain_mime(mime: &str) -> bool {
    matches!(mime, "UTF8_STRING" | "TEXT" | "STRING")
        || mime.eq_ignore_ascii_case("text/plain")
        || mime.eq_ignore_ascii_case(TEXT)
}

pub(super) fn rich_mime(mime: &str) -> bool {
    matches!(mime, "text/html" | "text/rtf" | "application/rtf")
}

pub(super) fn file_mime(mime: &str) -> bool {
    matches!(mime, "text/uri-list" | "x-special/gnome-copied-files")
}

pub fn supported_mime(mime: &str) -> bool {
    plain_mime(mime)
        || rich_mime(mime)
        || file_mime(mime)
        || matches!(
            mime,
            "image/png"
                | "image/jpeg"
                | "image/gif"
                | "image/webp"
                | "application/x-kde-cutselection"
        )
}

pub(super) fn image_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

/// Decode only local file references. The clipboard stores references, never
/// traverses directories or reads the referred-to file contents.
pub(super) fn local_path(uri: &str) -> Result<PathBuf> {
    let rest = uri
        .strip_prefix("file://")
        .ok_or_else(|| invalid("Only local file clipboard references are supported"))?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        rest.strip_prefix("localhost/")
            .map(|_| &rest["localhost".len()..])
            .ok_or_else(|| invalid("Remote clipboard file references are not supported"))?
    };
    if path.contains(['?', '#']) || path.bytes().any(|b| b.is_ascii_control()) {
        return Err(invalid("Invalid clipboard file URI"));
    }
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let pair = bytes
                .get(i + 1..i + 3)
                .ok_or_else(|| invalid("Invalid clipboard URI escape"))?;
            let digit = |b: u8| (b as char).to_digit(16).map(|v| v as u8);
            let a = digit(pair[0]).ok_or_else(|| invalid("Invalid clipboard URI escape"))?;
            let b = digit(pair[1]).ok_or_else(|| invalid("Invalid clipboard URI escape"))?;
            decoded.push(a * 16 + b);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    if decoded.contains(&0) {
        return Err(invalid("NUL in clipboard file URI"));
    }
    let path = PathBuf::from(OsString::from_vec(decoded));
    if !path.is_absolute() {
        return Err(invalid("Clipboard file path is not absolute"));
    }
    Ok(path)
}

pub(super) fn file_uris(format: &Format) -> Result<Vec<String>> {
    let text =
        std::str::from_utf8(&format.data).map_err(|_| invalid("Invalid file clipboard UTF-8"))?;
    let mut lines = text.lines();
    if format.mime == "x-special/gnome-copied-files"
        && !matches!(lines.next(), Some("copy" | "cut"))
    {
        return Err(invalid("Invalid GNOME file clipboard operation"));
    }
    let uris: Vec<_> = lines
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(str::to_owned)
        .collect();
    if uris.is_empty() || uris.len() > 4096 {
        return Err(invalid("Invalid clipboard file count"));
    }
    for uri in &uris {
        local_path(uri)?;
    }
    Ok(uris)
}

pub(super) fn files(formats: &[Format]) -> Result<Vec<PathBuf>> {
    let mut values: Option<Vec<PathBuf>> = None;
    for format in formats.iter().filter(|f| file_mime(&f.mime)) {
        let current = file_uris(format)?
            .iter()
            .map(|s| local_path(s))
            .collect::<Result<Vec<_>>>()?;
        if let Some(previous) = &values {
            if previous != &current {
                return Err(invalid("Conflicting clipboard file lists"));
            }
        } else {
            values = Some(current);
        }
    }
    Ok(values.unwrap_or_default())
}

pub(super) fn validate(formats: &[Format]) -> Result<()> {
    if formats.is_empty() || formats.len() > 32 {
        return Err(invalid("Invalid clipboard format count"));
    }
    let mut total = 0usize;
    let mut names = std::collections::HashSet::new();
    for format in formats {
        if !supported_mime(&format.mime)
            || !names.insert(&format.mime)
            || format.data.is_empty()
            || format.data.len() > MAX_BYTES
        {
            return Err(invalid("Invalid or oversized clipboard format"));
        }
        total = total
            .checked_add(format.data.len())
            .ok_or_else(|| invalid("Clipboard bundle is too large"))?;
        if total > MAX_TOTAL {
            return Err(invalid("Clipboard bundle is too large"));
        }
        if (plain_mime(&format.mime) || format.mime == "text/html")
            && std::str::from_utf8(&format.data).is_err()
        {
            return Err(invalid("Invalid clipboard UTF-8"));
        }
        if format.mime.starts_with("image/")
            && image_mime(&format.data) != Some(format.mime.as_str())
        {
            return Err(invalid("Clipboard image type does not match its payload"));
        }
        if file_mime(&format.mime) {
            file_uris(format)?;
        }
        if format.mime == "application/x-kde-cutselection"
            && !matches!(format.data.as_slice(), b"0" | b"1" | [0] | [1])
        {
            return Err(invalid("Invalid KDE clipboard operation"));
        }
    }
    let paths = files(formats)?;
    if paths.is_empty()
        && formats
            .iter()
            .any(|f| f.mime == "application/x-kde-cutselection")
    {
        return Err(invalid("Clipboard operation has no file list"));
    }
    Ok(())
}

pub(super) fn normalized(formats: &[Format]) -> Result<Vec<Format>> {
    validate(formats)?;
    let mut map = BTreeMap::new();
    for format in formats {
        let mut format = format.clone();
        if format.mime == "x-special/gnome-copied-files" {
            format.data = format!("copy\n{}\n", file_uris(&format)?.join("\n")).into_bytes();
        } else if format.mime == "application/x-kde-cutselection" {
            format.data = b"0".to_vec();
        }
        map.insert(format.mime.clone(), format);
    }
    let mut result: Vec<_> = map.into_values().collect();
    // A plain fallback makes HTML/RTF-only offers usable in terminals and the
    // UI. It is data, never executable markup or a rendered HTML document.
    if !result.iter().any(|f| plain_mime(&f.mime))
        && let Some(rich) = result.iter().find(|f| rich_mime(&f.mime))
    {
        let text = rich_text(rich);
        if !text.is_empty() && text.len() <= MAX_BYTES {
            let total: usize = result.iter().map(|f| f.data.len()).sum();
            if total + text.len() <= MAX_TOTAL {
                result.push(Format {
                    mime: TEXT.into(),
                    data: text.into_bytes(),
                });
                result.sort_by(|a, b| a.mime.cmp(&b.mime));
            }
        }
    }
    validate(&result)?;
    Ok(result)
}

fn entity(text: &str) -> Option<char> {
    match text {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        "nbsp" => Some(' '),
        _ => text
            .strip_prefix("#x")
            .or_else(|| text.strip_prefix("#X"))
            .and_then(|s| u32::from_str_radix(s, 16).ok())
            .or_else(|| text.strip_prefix('#')?.parse().ok())
            .and_then(char::from_u32),
    }
}

fn html_text(text: &str) -> String {
    let mut out = String::new();
    let mut position = 0;
    let mut hidden: Option<&str> = None;
    while position < text.len() {
        let tail = &text[position..];
        if tail.starts_with('<') {
            let mut quote = None;
            let mut end = None;
            for (i, c) in tail.char_indices().skip(1) {
                match (quote, c) {
                    (Some(q), c) if c == q => quote = None,
                    (None, '\'' | '"') => quote = Some(c),
                    (None, '>') => {
                        end = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            let Some(end) = end else {
                break;
            };
            let tag = tail[1..end].trim().to_ascii_lowercase();
            let name = tag
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches('/');
            if matches!(name, "script" | "style") {
                hidden = Some(if name == "script" { "script" } else { "style" });
            }
            if hidden.is_some_and(|h| name == format!("/{h}")) {
                hidden = None;
            }
            if hidden.is_none()
                && matches!(
                    name,
                    "br" | "p"
                        | "/p"
                        | "div"
                        | "/div"
                        | "li"
                        | "/li"
                        | "h1"
                        | "/h1"
                        | "h2"
                        | "/h2"
                        | "tr"
                        | "/tr"
                )
                && !out.ends_with('\n')
            {
                out.push('\n');
            }
            position += end + 1;
            continue;
        }
        let c = tail.chars().next().unwrap();
        if hidden.is_none() {
            if c == '&'
                && let Some(end) = tail.find(';').filter(|n| *n <= 16)
                && let Some(decoded) = entity(&tail[1..end])
            {
                out.push(decoded);
                position += end + 1;
                continue;
            }
            if c.is_whitespace() {
                if !out.ends_with([' ', '\n']) {
                    out.push(' ');
                }
            } else {
                out.push(c);
            }
        }
        position += c.len_utf8();
    }
    out.trim().to_owned()
}

fn rtf_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut hidden = false;
    let mut groups = Vec::new();
    let mut i = 0usize;
    let mut fallback = 1usize;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                if groups.len() >= 1024 {
                    break;
                }
                groups.push((hidden, fallback));
                i += 1;
            }
            '}' => {
                if let Some((h, f)) = groups.pop() {
                    hidden = h;
                    fallback = f;
                }
                i += 1;
            }
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                if matches!(chars[i], '\\' | '{' | '}') {
                    if !hidden {
                        out.push(chars[i]);
                    }
                    i += 1;
                    continue;
                }
                if chars[i] == '*' {
                    hidden = true;
                    i += 1;
                    continue;
                }
                if chars[i] == '\'' {
                    if i + 2 < chars.len() {
                        let hex: String = chars[i + 1..i + 3].iter().collect();
                        if !hidden && let Ok(value) = u8::from_str_radix(&hex, 16) {
                            out.push(value as char);
                        }
                        i += 3;
                    } else {
                        break;
                    }
                    continue;
                }
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i == start {
                    if !hidden && chars[i] == '~' {
                        out.push(' ');
                    }
                    i += 1;
                    continue;
                }
                let word: String = chars[start..i].iter().collect();
                let number_start = i;
                if i < chars.len() && chars[i] == '-' {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let number: Option<i32> = chars[number_start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .ok();
                if i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                if matches!(
                    word.as_str(),
                    "fonttbl"
                        | "colortbl"
                        | "stylesheet"
                        | "info"
                        | "pict"
                        | "object"
                        | "header"
                        | "footer"
                ) {
                    hidden = true;
                }
                if word == "uc" {
                    fallback = number.unwrap_or(1).clamp(0, 16) as usize;
                }
                if !hidden {
                    match word.as_str() {
                        "par" | "line" => out.push('\n'),
                        "tab" => out.push('\t'),
                        "u" => {
                            if let Some(n) = number {
                                if let Some(c) = char::from_u32((n as u16) as u32) {
                                    out.push(c);
                                }
                                i = (i + fallback).min(chars.len());
                            }
                        }
                        _ => {}
                    }
                }
            }
            c => {
                if !hidden && !matches!(c, '\r' | '\n') {
                    out.push(c);
                }
                i += 1;
            }
        }
    }
    out.trim().to_owned()
}

pub(super) fn rich_text(format: &Format) -> String {
    if format.mime == "text/html" {
        html_text(std::str::from_utf8(&format.data).unwrap_or_default())
    } else {
        rtf_text(&format.data)
    }
}

pub(super) fn plain_text(formats: &[Format]) -> Option<String> {
    formats
        .iter()
        .find(|f| f.mime.eq_ignore_ascii_case(TEXT))
        .or_else(|| formats.iter().find(|f| plain_mime(&f.mime)))
        .and_then(|f| std::str::from_utf8(&f.data).ok())
        .map(str::to_owned)
        .or_else(|| formats.iter().find(|f| rich_mime(&f.mime)).map(rich_text))
}

pub(super) fn primary(formats: &[Format]) -> &Format {
    formats
        .iter()
        .find(|f| f.mime == "text/uri-list")
        .or_else(|| formats.iter().find(|f| file_mime(&f.mime)))
        .or_else(|| formats.iter().find(|f| f.mime.starts_with("image/")))
        .or_else(|| formats.iter().find(|f| f.mime == "text/html"))
        .or_else(|| formats.iter().find(|f| rich_mime(&f.mime)))
        .or_else(|| formats.iter().find(|f| f.mime.eq_ignore_ascii_case(TEXT)))
        .or_else(|| formats.iter().find(|f| plain_mime(&f.mime)))
        .unwrap_or(&formats[0])
}
