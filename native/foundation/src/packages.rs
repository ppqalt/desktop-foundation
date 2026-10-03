//! Fastfetch's existing package totals and invalidated foreign-count cache.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileStamp {
    Present(PathBuf, u64, i64, i64, u64),
    Missing(PathBuf, Option<()>),
}
#[derive(Serialize, Deserialize)]
struct CountCache {
    signature: Vec<FileStamp>,
    count: u64,
}

pub fn signature(database: &Path, repositories: &[String]) -> Result<Vec<FileStamp>> {
    let mut paths = vec![database.join("local")];
    paths.extend(
        repositories
            .iter()
            .map(|r| database.join("sync").join(format!("{r}.db"))),
    );
    paths
        .into_iter()
        .map(|path| match fs::metadata(&path) {
            Ok(m) => Ok(FileStamp::Present(
                path,
                m.ino(),
                m.mtime()
                    .saturating_mul(1_000_000_000)
                    .saturating_add(m.mtime_nsec()),
                m.ctime()
                    .saturating_mul(1_000_000_000)
                    .saturating_add(m.ctime_nsec()),
                m.len(),
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(FileStamp::Missing(path, None))
            }
            Err(e) => Err(e.into()),
        })
        .collect()
}

pub fn foreign_count(database: &Path, repositories: &[String], cache: &Path) -> Result<u64> {
    state::private_directory(
        cache
            .parent()
            .ok_or_else(|| invalid("Package cache needs a directory"))?,
    )?;
    let _lock = state::lock(&cache.with_extension("lock"), Duration::from_secs(5))?;
    let before = signature(database, repositories)?;
    let transaction = database.join("db.lck");
    // This cache is reproducible; malformed/old data causes a fresh native query.
    let saved = fs::File::open(cache).ok().and_then(|file| {
        let mut bytes = Vec::new();
        file.take(64 * 1024 + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 64 * 1024 {
            return None;
        }
        serde_json::from_slice::<CountCache>(&bytes).ok()
    });
    if let Some(saved) = saved
        && !transaction.exists()
        && saved.signature == before
    {
        return Ok(saved.count);
    }
    let output = process::run(
        &["pacman".into(), "-Qqm".into()],
        None,
        Duration::from_secs(5),
        true,
    )?;
    if !output.status.success()
        && !(output.status.code() == Some(1)
            && output.stdout.is_empty()
            && output.stderr.is_empty())
    {
        return Err(invalid(format!(
            "Unable to query foreign packages: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let count = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("Package names are not UTF-8"))?
        .lines()
        .count() as u64;
    let after = signature(database, repositories)?;
    if before == after && !transaction.exists() {
        state::atomic(
            cache,
            &serde_json::to_vec(&CountCache {
                signature: after,
                count,
            })?,
            0o600,
        )?;
    }
    Ok(count)
}

pub fn display(root: &Path) -> Result<String> {
    let output = |args: Vec<String>| -> Result<String> {
        let value = process::checked(&args, None, Duration::from_secs(5), true)?;
        String::from_utf8(value.stdout).map_err(|_| invalid("Package information is not UTF-8"))
    };
    let database = PathBuf::from(output(vec!["pacman-conf".into(), "DBPath".into()])?.trim());
    if !database.is_absolute() {
        return Err(invalid("Pacman database must be an absolute directory"));
    }
    let repositories: Vec<String> = output(vec!["pacman-conf".into(), "--repo-list".into()])?
        .lines()
        .map(String::from)
        .collect();
    if repositories
        .iter()
        .any(|r| r.is_empty() || r.contains('/') || r == "." || r == "..")
    {
        return Err(invalid("Invalid pacman repository name"));
    }
    let cache =
        state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/fastfetch-foreign.json");
    let count = foreign_count(&database, &repositories, &cache)?;
    let total = output(vec![
        "fastfetch".into(),
        "--config".into(),
        root.join("terminal/fastfetch/packages.jsonc")
            .to_string_lossy()
            .into_owned(),
        "--pipe".into(),
        "true".into(),
    ])?;
    Ok(format!("{}, {count} (AUR)", total.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_signature_layout_and_strict_count_types_are_preserved() {
        let saved: CountCache = serde_json::from_str(
            r#"{"signature":[["/db/local",1,2,3,4],["/db/sync/missing.db",null]],"count":5}"#,
        )
        .unwrap();
        assert_eq!(
            saved.signature[0],
            FileStamp::Present("/db/local".into(), 1, 2, 3, 4)
        );
        assert_eq!(serde_json::to_value(saved).unwrap()["count"], 5);
        assert!(serde_json::from_str::<CountCache>(r#"{"signature":[],"count":true}"#).is_err());
        assert!(serde_json::from_str::<CountCache>(r#"{"signature":[],"count":-1}"#).is_err());
    }
}
