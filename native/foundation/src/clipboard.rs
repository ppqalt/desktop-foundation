//! Backward-compatible SQLite history, format bundles and a typed UI projection.
use crate::{Result, invalid, state};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
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
mod formats;
mod projections;
pub use formats::supported_mime;
use formats::{
    files, image_mime, normalized, plain_mime, plain_text, primary, rich_mime, validate,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Format {
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub mime: String,
    pub preview: String,
    pub size: usize,
    pub updated: f64,
    pub image: String,
    pub pinned: bool,
    pub kind: String,
    pub label: String,
    pub detail: String,
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    pub unavailable: bool,
    pub formats: Vec<String>,
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

fn image_projection(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(id, ext)| valid_id(id) && matches!(ext, "png" | "jpeg" | "gif" | "webp"))
}

fn interrupted_publication(name: &str) -> bool {
    let Some(temporary) = name.strip_suffix(".tmp") else {
        return false;
    };
    // The old worker used fixed temporary names. The native publisher adds a
    // dot, positive process ID and sequence; match that exact owned shape.
    if temporary == "index.json"
        || temporary == projections::CACHE_NAME
        || image_projection(temporary)
    {
        return true;
    }
    let Some(native) = temporary.strip_prefix('.') else {
        return false;
    };
    let Some((publication, sequence)) = native.rsplit_once('-') else {
        return false;
    };
    let Some((destination, pid)) = publication.rsplit_once('-') else {
        return false;
    };
    (destination == "index.json"
        || destination == projections::CACHE_NAME
        || image_projection(destination))
        && pid
            .parse::<u32>()
            .is_ok_and(|n| n > 0 && n.to_string() == pid)
        && sequence
            .parse::<u64>()
            .is_ok_and(|n| n.to_string() == sequence)
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
        let mut db = Connection::open(&database)?;
        fs::set_permissions(&database, fs::Permissions::from_mode(0o600))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("PRAGMA secure_delete=ON; PRAGMA foreign_keys=ON;")?;
        migrate(&mut db)?;
        let history = Self {
            directory,
            db,
            _lock: lock,
        };
        // Abrupt termination skips the atomic publisher's destructor. With
        // the shared lock held, no recognized publication can still be active.
        // Validate/open SQLite first so a corrupt database retains evidence.
        history.remove_interrupted_publications()?;
        Ok(history)
    }

    fn remove_interrupted_publications(&self) -> Result<()> {
        let mut removed = false;
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            if item
                .file_name()
                .to_str()
                .is_some_and(interrupted_publication)
                && item.file_type()?.is_file()
            {
                fs::remove_file(item.path())?;
                removed = true;
            }
        }
        if removed {
            fs::File::open(&self.directory)?.sync_all()?;
        }
        Ok(())
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
        self.store_bundle(&[Format {
            mime: mime.into(),
            data: data.to_vec(),
        }])
    }

    pub fn store_bundle(&mut self, offered: &[Format]) -> Result<()> {
        let formats = normalized(offered)?;
        let id = bundle_id(&formats);
        let updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("System clock precedes Unix epoch"))?
            .as_secs_f64();
        let tx = self.db.transaction()?;
        // Replaying a plain fallback of a pinned rich item must not replace
        // its formats or turn the protected item into an unpinned duplicate.
        let pinned_fallback: Option<String> = if formats.len() == 1 && plain_mime(&formats[0].mime)
        {
            tx.query_row("SELECT c.id FROM clips c JOIN clip_meta m ON m.id=c.id WHERE m.pinned=1 AND c.mime NOT LIKE 'image/%' AND c.mime NOT IN ('text/uri-list','x-special/gnome-copied-files') AND ((c.mime IN ('text/plain;charset=utf-8','text/plain','UTF8_STRING','TEXT','STRING') AND c.payload=?1) OR EXISTS (SELECT 1 FROM clip_formats f WHERE f.id=c.id AND f.mime IN ('text/plain;charset=utf-8','text/plain','UTF8_STRING','TEXT','STRING') AND f.data=?1)) ORDER BY c.updated DESC LIMIT 1", [&formats[0].data], |r| r.get(0)).optional()?
        } else {
            None
        };
        if let Some(existing) = pinned_fallback {
            tx.execute(
                "UPDATE clips SET updated=? WHERE id=?",
                params![updated, existing],
            )?;
        } else {
            let selected = primary(&formats);
            tx.execute("INSERT INTO clips(id,mime,payload,updated) VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET updated=excluded.updated", params![id, selected.mime, selected.data, updated])?;
            tx.execute(
                "INSERT OR IGNORE INTO clip_meta(id,pinned) VALUES (?,0)",
                [&id],
            )?;
            for format in formats.iter().filter(|f| f.mime != selected.mime) {
                tx.execute("INSERT INTO clip_formats(id,mime,data) VALUES (?,?,?) ON CONFLICT(id,mime) DO UPDATE SET data=excluded.data", params![id, format.mime, format.data])?;
            }
        }
        retain_unpinned(&tx)?;
        tx.commit()?;
        self.export()
    }

    fn stored_bundle(&self, id: &str) -> Result<Vec<Format>> {
        if !valid_id(id) {
            return Err(invalid("Invalid history ID"));
        }
        let stored: Option<(String, usize, f64, i64)> = self.db.query_row(
            "SELECT c.mime,length(c.payload),c.updated,m.pinned FROM clips c JOIN clip_meta m ON m.id=c.id WHERE c.id=?", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
        let (mime, primary_size, updated, pinned) =
            stored.ok_or_else(|| invalid("History item no longer exists"))?;
        if !updated.is_finite()
            || updated < 0.0
            || !matches!(pinned, 0 | 1)
            || primary_size == 0
            || primary_size > MAX_BYTES
        {
            return Err(invalid("Invalid clipboard history metadata"));
        }
        let (count, total, maximum, minimum): (usize, usize, usize, usize) = self.db.query_row(
            "SELECT count(*),coalesce(sum(length(data)),0),coalesce(max(length(data)),0),coalesce(min(length(data)),0) FROM clip_formats WHERE id=?", [id],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        if count >= 32
            || total.saturating_add(primary_size) > MAX_TOTAL
            || maximum > MAX_BYTES
            || (count > 0 && minimum == 0)
        {
            return Err(invalid("Invalid clipboard bundle size"));
        }
        let payload: Vec<u8> =
            self.db
                .query_row("SELECT payload FROM clips WHERE id=?", [id], |r| r.get(0))?;
        let mut query = self
            .db
            .prepare("SELECT mime,data FROM clip_formats WHERE id=? ORDER BY mime")?;
        let mut formats: Vec<Format> = query
            .query_map([id], |r| {
                Ok(Format {
                    mime: r.get(0)?,
                    data: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        formats.push(Format {
            mime: mime.clone(),
            data: payload,
        });
        formats.sort_by(|a, b| a.mime.cmp(&b.mime));
        validate(&formats)?;
        if primary(&formats).mime != mime {
            return Err(invalid("Invalid clipboard primary format"));
        }
        // Stored cut operations are never replayed. Normalization is also a
        // defensive read boundary for externally changed database rows.
        normalized(&formats)
    }

    pub fn bundle(&self, id: &str) -> Result<Vec<Format>> {
        let formats = self.stored_bundle(id)?;
        if files(&formats)?
            .iter()
            .any(|path| fs::metadata(path).is_err())
        {
            return Err(invalid("A copied file or folder is no longer available"));
        }
        Ok(formats)
    }

    pub fn plain_payload(&self, id: &str) -> Result<Vec<u8>> {
        let formats = self.stored_bundle(id)?;
        if !files(&formats)?.is_empty() || formats.iter().any(|f| f.mime.starts_with("image/")) {
            return Err(invalid("This clipboard item is not text"));
        }
        plain_text(&formats)
            .map(String::into_bytes)
            .ok_or_else(|| invalid("No plain text is available"))
    }

    pub fn payload(&self, id: &str) -> Result<(String, Vec<u8>)> {
        let formats = self.bundle(id)?;
        let selected = primary(&formats);
        Ok((selected.mime.clone(), selected.data.clone()))
    }

    pub fn pin(&mut self, id: &str, pinned: bool) -> Result<()> {
        self.stored_bundle(id)?;
        let tx = self.db.transaction()?;
        if pinned {
            let (count, size): (usize, usize) = tx.query_row(
                "SELECT count(*),coalesce(sum(length(c.payload)+(SELECT coalesce(sum(length(data)),0) FROM clip_formats f WHERE f.id=c.id)),0) FROM clip_meta m JOIN clips c ON c.id=m.id WHERE m.pinned=1 AND m.id!=?", [id], |r| Ok((r.get(0)?,r.get(1)?)))?;
            let added: usize = tx.query_row(
                "SELECT length(payload)+(SELECT coalesce(sum(length(data)),0) FROM clip_formats f WHERE f.id=c.id) FROM clips c WHERE id=?",
                [id],
                |r| r.get(0),
            )?;
            if count >= MAX_ITEMS || size.saturating_add(added) > MAX_TOTAL {
                return Err(invalid(
                    "Pinned history is full (100 items or 32 MiB); unpin an item first",
                ));
            }
        }
        if tx.execute(
            "UPDATE clip_meta SET pinned=? WHERE id=?",
            params![i64::from(pinned), id],
        )? == 0
        {
            return Err(invalid("History item no longer exists"));
        }
        retain_unpinned(&tx)?;
        tx.commit()?;
        self.export()
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
        tx.execute(
            "DELETE FROM clips WHERE id IN (SELECT id FROM clip_meta WHERE pinned=0)",
            [],
        )?;
        tx.commit()?;
        self.export()
    }

    pub fn clear_all(&mut self) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM clips", [])?;
        tx.commit()?;
        self.export()
    }

    pub fn export(&self) -> Result<()> {
        // Build and validate the full projection before publishing. Invalid
        // metadata cannot overwrite the last complete index.
        let mut query = self.db.prepare("SELECT c.id,c.mime,c.updated,m.pinned FROM clips c LEFT JOIN clip_meta m ON m.id=c.id ORDER BY m.pinned DESC,c.updated DESC,c.id")?;
        let rows: Vec<(String, String, f64, Option<i64>)> = query
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut entries = Vec::new();
        let mut images = std::collections::HashSet::new();
        let mut projection_cache = projections::Cache::load(&self.directory);
        let mut projection_bytes: Vec<(String, PathBuf, Vec<u8>)> = Vec::new();
        let mut counts = [0usize; 2];
        let mut totals = [0usize; 2];
        for (id, mime, updated, pinned) in rows {
            let pinned = match pinned {
                Some(0) => false,
                Some(1) => true,
                _ => return Err(invalid("Invalid clipboard pin metadata")),
            };
            let formats = self.stored_bundle(&id)?;
            let size: usize = formats.iter().map(|f| f.data.len()).sum();
            let bucket = usize::from(pinned);
            counts[bucket] += 1;
            totals[bucket] = totals[bucket]
                .checked_add(size)
                .ok_or_else(|| invalid("Invalid clipboard size"))?;
            if counts[bucket] > MAX_ITEMS || totals[bucket] > MAX_TOTAL {
                return Err(invalid("Clipboard history retention limit exceeded"));
            }
            let paths = files(&formats)?;
            // Availability and folder classification share the same snapshot.
            // File references remain live; their contents are never read.
            let path_metadata: Vec<_> = paths.iter().map(|path| fs::metadata(path).ok()).collect();
            let unavailable = path_metadata.iter().any(Option::is_none);
            let mut image = String::new();
            let (kind, label, detail, text) = if !paths.is_empty() {
                let names: Vec<String> = paths
                    .iter()
                    .map(|path| {
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect();
                let folders = path_metadata
                    .iter()
                    .filter(|metadata| metadata.as_ref().is_some_and(|m| m.is_dir()))
                    .count();
                let label = if paths.len() == 1 {
                    names[0].chars().take(120).collect()
                } else {
                    let mut label = names
                        .iter()
                        .take(2)
                        .map(|name| name.chars().take(50).collect::<String>())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if names.len() > 2 {
                        label.push_str(&format!(" + {}", names.len() - 2));
                    }
                    label
                };
                let detail = if unavailable {
                    "Original file unavailable".into()
                } else if paths.len() == 1 && folders == 1 {
                    "Folder".into()
                } else if paths.len() == 1 {
                    paths[0]
                        .extension()
                        .map(|ext| format!("{} file", ext.to_string_lossy().to_uppercase()))
                        .unwrap_or_else(|| "File".into())
                } else if folders == 0 {
                    format!("{} files", paths.len())
                } else if folders == paths.len() {
                    format!("{} folders", folders)
                } else {
                    format!(
                        "{} {} · {} {}",
                        paths.len() - folders,
                        if paths.len() - folders == 1 {
                            "file"
                        } else {
                            "files"
                        },
                        folders,
                        if folders == 1 { "folder" } else { "folders" }
                    )
                };
                ("files", label, detail, names.join("\n"))
            } else if let Some(format) = formats.iter().find(|f| f.mime.starts_with("image/")) {
                let name = format!(
                    "{}.{}",
                    id,
                    format
                        .mime
                        .strip_prefix("image/")
                        .ok_or_else(|| invalid("Invalid image MIME"))?
                );
                let filename = self.directory.join(&name);
                images.insert(name.clone());
                // Bundles are always freshly validated above. Only the second
                // read of an unchanged image projection can use the cache.
                if !projection_cache.verify(&name, &filename, &format.data)? {
                    projection_bytes.push((name, filename.clone(), format.data.clone()));
                }
                image = state::file_uri(&filename)?;
                (
                    "image",
                    "Copied image".into(),
                    format
                        .mime
                        .strip_prefix("image/")
                        .unwrap_or_default()
                        .to_uppercase(),
                    "Copied image".into(),
                )
            } else {
                let text = plain_text(&formats).unwrap_or_else(|| "Formatted text".into());
                let rich = formats.iter().any(|f| rich_mime(&f.mime));
                let label = text
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or(if rich { "Formatted text" } else { "Text" })
                    .chars()
                    .take(120)
                    .collect();
                let detail = if rich {
                    "Formatted text".into()
                } else {
                    format!("{} characters", text.chars().count())
                };
                (if rich { "rich" } else { "text" }, label, detail, text)
            };
            entries.push(Entry {
                id,
                mime,
                preview: text.chars().take(2048).collect(),
                size,
                updated,
                image,
                pinned,
                kind: kind.into(),
                label,
                detail,
                file_count: paths.len(),
                unavailable,
                formats: formats.iter().map(|f| f.mime.clone()).collect(),
            });
        }
        for (name, filename, bytes) in projection_bytes {
            state::atomic(&filename, &bytes, 0o600)?;
            // A cache miss is safe, including a race with an external writer.
            // Cache publication never decides whether history was stored.
            let _ = projection_cache.verify(&name, &filename, &bytes);
        }
        state::atomic(
            &self.directory.join("index.json"),
            &serde_json::to_vec(&entries)?,
            0o600,
        )?;
        let mut removed = false;
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            let name = item.file_name();
            if let Some(name) = name.to_str()
                && (interrupted_publication(name)
                    || (image_projection(name) && !images.contains(name)))
                && item.file_type()?.is_file()
            {
                fs::remove_file(item.path())?;
                removed = true;
            }
        }
        if removed {
            fs::File::open(&self.directory)?.sync_all()?;
        }
        projection_cache.publish(&self.directory, &images);
        Ok(())
    }
}

fn schema(db: &Connection, name: &str, expected: &[(&str, &str)]) -> Result<()> {
    let mut query = db.prepare(&format!("PRAGMA table_info({name})"))?;
    let columns: Vec<(String, String, i64)> = query
        .query_map([], |r| Ok((r.get(1)?, r.get(2)?, r.get(5)?)))?
        .collect::<std::result::Result<_, _>>()?;
    if columns.len() != expected.len()
        || !columns.iter().zip(expected).enumerate().all(
            |(index, ((column_name, kind, pk), (expected_name, expected_kind)))| {
                let expected_pk = if name == "clip_formats" {
                    [1, 2, 0][index]
                } else {
                    i64::from(index == 0)
                };
                column_name == expected_name
                    && kind.eq_ignore_ascii_case(expected_kind)
                    && *pk == expected_pk
            },
        )
    {
        return Err(invalid(
            "Unsupported clipboard database schema; history retained",
        ));
    }
    Ok(())
}

fn migrate(db: &mut Connection) -> Result<()> {
    let tx = db.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS clips (id TEXT PRIMARY KEY, mime TEXT, payload BLOB, updated REAL);")?;
    schema(
        &tx,
        "clips",
        &[
            ("id", "TEXT"),
            ("mime", "TEXT"),
            ("payload", "BLOB"),
            ("updated", "REAL"),
        ],
    )?;
    let existing: usize = tx.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('clip_meta','clip_formats')", [], |r| r.get(0))?;
    if existing == 1 {
        return Err(invalid(
            "Incomplete clipboard metadata schema; history retained",
        ));
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS clip_meta (id TEXT PRIMARY KEY REFERENCES clips(id) ON DELETE CASCADE, pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0,1))); CREATE TABLE IF NOT EXISTS clip_formats (id TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE, mime TEXT NOT NULL, data BLOB NOT NULL, PRIMARY KEY(id,mime));")?;
    schema(&tx, "clip_meta", &[("id", "TEXT"), ("pinned", "INTEGER")])?;
    schema(
        &tx,
        "clip_formats",
        &[("id", "TEXT"), ("mime", "TEXT"), ("data", "BLOB")],
    )?;
    for name in ["clip_meta", "clip_formats"] {
        let mut query = tx.prepare(&format!("PRAGMA foreign_key_list({name})"))?;
        let keys: Vec<(String, String, String, String)> = query
            .query_map([], |r| Ok((r.get(2)?, r.get(3)?, r.get(4)?, r.get(6)?)))?
            .collect::<std::result::Result<_, _>>()?;
        if keys != [("clips".into(), "id".into(), "id".into(), "CASCADE".into())] {
            return Err(invalid("Invalid clipboard metadata ownership schema"));
        }
    }
    // Import rows written by the old four-column worker after a rollback,
    // as well as rows from the initial migration. Their IDs stay unchanged.
    {
        let (count, total, maximum): (usize,usize,usize) = tx.query_row("SELECT count(*),coalesce(sum(length(payload)),0),coalesce(max(length(payload)),0) FROM clips WHERE id NOT IN (SELECT id FROM clip_meta)", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if count > MAX_ITEMS || total > MAX_TOTAL || maximum > MAX_BYTES {
            return Err(invalid("Legacy clipboard history exceeds retention limits"));
        }
        let mut query = tx.prepare(
            "SELECT id,mime,payload,updated FROM clips WHERE id NOT IN (SELECT id FROM clip_meta)",
        )?;
        let rows: Vec<(String, String, Vec<u8>, f64)> = query
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<std::result::Result<_, _>>()?;
        for (id, mime, data, updated) in rows {
            if !valid_id(&id) || !updated.is_finite() || updated < 0.0 {
                return Err(invalid(
                    "Invalid legacy clipboard metadata; history retained",
                ));
            }
            let formats = normalized(&[Format {
                mime: mime.clone(),
                data,
            }])?;
            let selected = primary(&formats);
            // Added rich-text fallbacks and copy-safe file-operation markers
            // are stored once, while the legacy ID remains unchanged.
            tx.execute(
                "UPDATE clips SET mime=?,payload=? WHERE id=?",
                params![selected.mime, selected.data, id],
            )?;
            tx.execute("INSERT INTO clip_meta(id,pinned) VALUES (?,0)", [&id])?;
            for format in formats.iter().filter(|f| f.mime != selected.mime) {
                tx.execute(
                    "INSERT INTO clip_formats(id,mime,data) VALUES (?,?,?)",
                    params![id, format.mime, format.data],
                )?;
            }
        }
    }
    // The four-column table remains readable by the previous worker. If it
    // ran during rollback, reconcile its deleted rows without losing pins
    // on the rows that still exist.
    tx.execute(
        "DELETE FROM clip_formats WHERE id NOT IN (SELECT id FROM clips)",
        [],
    )?;
    tx.execute(
        "DELETE FROM clip_meta WHERE id NOT IN (SELECT id FROM clips)",
        [],
    )?;
    tx.execute("DELETE FROM clip_formats WHERE EXISTS (SELECT 1 FROM clips c WHERE c.id=clip_formats.id AND c.mime=clip_formats.mime AND c.payload=clip_formats.data)", [])?;
    tx.commit()?;
    Ok(())
}

fn retain_unpinned(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let mut query = tx.prepare("SELECT c.id,length(c.payload)+(SELECT coalesce(sum(length(data)),0) FROM clip_formats f WHERE f.id=c.id) FROM clips c JOIN clip_meta m ON m.id=c.id WHERE m.pinned=0 ORDER BY c.updated DESC,c.id")?;
    let rows: Vec<(String, usize)> = query
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut total = 0usize;
    for (index, (id, size)) in rows.iter().enumerate() {
        total = total.saturating_add(*size);
        if index >= MAX_ITEMS || total > MAX_TOTAL {
            tx.execute("DELETE FROM clips WHERE id=?", [id])?;
        }
    }
    Ok(())
}

fn bundle_id(formats: &[Format]) -> String {
    let mut digest = Sha256::new();
    if formats.len() == 1 {
        digest.update(&formats[0].mime);
        digest.update([0]);
        digest.update(&formats[0].data);
    } else {
        digest.update(b"desktop-foundation-clipboard-bundle-v1\0");
        for format in formats {
            digest.update((format.mime.len() as u64).to_be_bytes());
            digest.update(&format.mime);
            digest.update((format.data.len() as u64).to_be_bytes());
            digest.update(&format.data);
        }
    }
    format!("{:x}", digest.finalize())
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
