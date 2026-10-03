//! v0.12-compatible SQLite storage and a bounded, typed UI projection.
use crate::{Result, invalid, state};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TOTAL: usize = 32 * 1024 * 1024;
pub const MAX_ITEMS: usize = 100;
const TEXT: &str = "text/plain;charset=utf-8";

#[derive(Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub mime: String,
    pub preview: String,
    pub size: usize,
    pub updated: f64,
    pub image: String,
}

pub struct History {
    directory: PathBuf,
    db: Connection,
    _lock: fs::File,
}

pub fn directory() -> Result<PathBuf> {
    Ok(
        match env::var_os("DF_CLIPBOARD_STATE").filter(|s| !s.is_empty()) {
            Some(p) => PathBuf::from(p),
            None => {
                state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/clipboard")
            }
        },
    )
}

pub fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn image_mime(data: &[u8]) -> Option<&'static str> {
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

fn valid_mime(mime: &str) -> bool {
    matches!(
        mime,
        TEXT | "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    )
}

fn preview(bytes: &[u8], total: usize) -> Result<String> {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) if total > bytes.len() && e.error_len().is_none() => {
            std::str::from_utf8(&bytes[..e.valid_up_to()])
                .map_err(|_| invalid("Invalid UTF-8 clipboard preview"))?
        }
        Err(_) => {
            return Err(invalid(
                "Invalid UTF-8 in clipboard history; existing index retained",
            ));
        }
    };
    Ok(text.chars().take(2048).collect())
}

impl History {
    pub fn open(directory: &Path) -> Result<Self> {
        state::private_directory(directory)?;
        let directory = fs::canonicalize(directory)?;
        let lock = state::lock(&directory.join("lock"), Duration::from_secs(5))?;
        let database = directory.join("history.sqlite");
        if fs::symlink_metadata(&database).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid("Refusing symlink clipboard database"));
        }
        let db = Connection::open(&database)?;
        fs::set_permissions(&database, fs::Permissions::from_mode(0o600))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS clips (id TEXT PRIMARY KEY, mime TEXT, payload BLOB, updated REAL);")?;
        Ok(Self {
            directory,
            db,
            _lock: lock,
        })
    }

    pub fn store(&mut self, kind: &str, data: &[u8]) -> Result<()> {
        let mime = match kind {
            "text" if std::str::from_utf8(data).is_ok() => TEXT,
            "text" => return Ok(()),
            "image" => match image_mime(data) {
                Some(m) => m,
                None => return Ok(()),
            },
            _ => return Err(invalid("store requires text or image")),
        };
        if data.is_empty() || data.len() > MAX_BYTES {
            return Ok(());
        }
        let mut digest = Sha256::new();
        digest.update(mime);
        digest.update([0]);
        digest.update(data);
        let id = format!("{:x}", digest.finalize());
        let updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("System clock precedes Unix epoch"))?
            .as_secs_f64();
        let tx = self.db.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO clips VALUES (?,?,?,?)",
            params![id, mime, data, updated],
        )?;
        let rows: Vec<(String, usize)> = {
            let mut query =
                tx.prepare("SELECT id,length(payload) FROM clips ORDER BY updated DESC")?;
            query
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?
        };
        let mut total = 0usize;
        for (index, (id, size)) in rows.iter().enumerate() {
            total = total.saturating_add(*size);
            if index >= MAX_ITEMS || total > MAX_TOTAL {
                tx.execute("DELETE FROM clips WHERE id=?", [id])?;
            }
        }
        tx.commit()?;
        self.export()
    }

    pub fn payload(&self, id: &str) -> Result<(String, Vec<u8>)> {
        if !valid_id(id) {
            return Err(invalid("Invalid history ID"));
        }
        let value: (String, Vec<u8>) =
            self.db
                .query_row("SELECT mime,payload FROM clips WHERE id=?", [id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
        if !valid_mime(&value.0) || value.1.len() > MAX_BYTES {
            return Err(invalid("Invalid stored clipboard item"));
        }
        Ok(value)
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        if !valid_id(id) {
            return Err(invalid("Invalid history ID"));
        }
        let tx = self.db.transaction()?;
        if tx.execute("DELETE FROM clips WHERE id=?", [id])? == 0 {
            return Err(invalid("History item no longer exists"));
        }
        tx.commit()?;
        self.export()
    }

    pub fn clear(&mut self) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM clips", [])?;
        tx.commit()?;
        self.export()
    }

    pub fn export(&self) -> Result<()> {
        let mut entries = Vec::new();
        let mut images = std::collections::HashSet::new();
        let mut query = self.db.prepare("SELECT id,mime,length(payload),updated,CASE WHEN mime LIKE 'image/%' THEN NULL ELSE substr(payload,1,8192) END FROM clips ORDER BY updated DESC")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let mime: String = row.get(1)?;
            let size: usize = row.get(2)?;
            let updated: f64 = row.get(3)?;
            if !valid_id(&id) || !valid_mime(&mime) || size > MAX_BYTES || !updated.is_finite() {
                return Err(invalid(
                    "Invalid clipboard history metadata; existing index retained",
                ));
            }
            let (text, image) = if mime.starts_with("image/") {
                let extension = mime
                    .strip_prefix("image/")
                    .ok_or_else(|| invalid("Invalid image MIME"))?;
                let name = format!("{id}.{extension}");
                let filename = self.directory.join(&name);
                images.insert(name);
                if fs::symlink_metadata(&filename).is_ok_and(|m| !m.is_file()) {
                    return Err(invalid("Invalid image projection path"));
                }
                if !filename.exists() {
                    let data: Option<Vec<u8>> = self
                        .db
                        .query_row("SELECT payload FROM clips WHERE id=?", [&id], |r| r.get(0))
                        .optional()?;
                    state::atomic(
                        &filename,
                        &data.ok_or_else(|| invalid("History item disappeared"))?,
                        0o600,
                    )?;
                }
                (String::from("Copied image"), state::file_uri(&filename)?)
            } else {
                let bytes: Vec<u8> = row.get(4)?;
                (preview(&bytes, size)?, String::new())
            };
            entries.push(Entry {
                id,
                mime,
                preview: text,
                size,
                updated,
                image,
            });
        }
        state::atomic(
            &self.directory.join("index.json"),
            &serde_json::to_vec(&entries)?,
            0o600,
        )?;
        // Remove only recognized projections, after publishing the new index.
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            let name = item.file_name();
            let path = item.path();
            if let Some(name) = name.to_str()
                && let Some((id, ext)) = name.rsplit_once('.')
                && valid_id(id)
                && matches!(ext, "png" | "jpeg" | "gif" | "webp")
                && !images.contains(name)
            {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

pub fn accepted(kind: &str, data: &[u8]) -> Result<bool> {
    if !matches!(kind, "text" | "image") {
        return Err(invalid("store requires text or image"));
    }
    Ok(
        env::var("CLIPBOARD_STATE").unwrap_or_else(|_| "data".into()) == "data"
            && !data.is_empty()
            && data.len() <= MAX_BYTES
            && if kind == "text" {
                std::str::from_utf8(data).is_ok()
            } else {
                image_mime(data).is_some()
            },
    )
}
