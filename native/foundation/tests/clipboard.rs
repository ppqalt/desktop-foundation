use desktop_foundationctl::{
    clipboard::{History, MAX_BYTES, MAX_ITEMS, MAX_TOTAL},
    state,
};
use rusqlite::Connection;
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "foundation-history-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn entries(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("index.json")).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn exact_unicode_nul_and_dedup_with_private_modes() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    let data = "ä ö å €\0🙂\nline two\n".as_bytes();
    h.store("text", data).unwrap();
    h.store("text", data).unwrap();
    let e = f.entries();
    assert_eq!(e.as_array().unwrap().len(), 1);
    assert_eq!(e[0]["preview"], std::str::from_utf8(data).unwrap());
    assert_eq!(h.payload(e[0]["id"].as_str().unwrap()).unwrap().1, data);
    assert_eq!(
        fs::metadata(&f.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["index.json", "history.sqlite", "lock"] {
        assert_eq!(
            fs::metadata(f.0.join(name)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn image_projection_is_repaired_and_removed_without_touching_unrelated_files() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    let bytes = b"\x89PNG\r\n\x1a\nfixture";
    h.store("image", bytes).unwrap();
    let e = f.entries();
    let image = f.0.join(format!("{}.png", e[0]["id"].as_str().unwrap()));
    fs::remove_file(&image).unwrap();
    h.export().unwrap();
    assert_eq!(fs::read(&image).unwrap(), bytes);
    fs::write(f.0.join("user.png"), b"unrelated").unwrap();
    h.delete(e[0]["id"].as_str().unwrap()).unwrap();
    assert!(!image.exists());
    assert_eq!(fs::read(f.0.join("user.png")).unwrap(), b"unrelated");
    assert_eq!(f.entries(), serde_json::json!([]));
}

#[test]
fn character_boundary_previews_are_not_byte_truncated() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    for text in [
        "a".to_owned() + &"🙂".repeat(3000),
        "äöå€".repeat(1200),
        "before\0after".into(),
    ] {
        h.store("text", text.as_bytes()).unwrap();
        assert_eq!(
            f.entries()[0]["preview"],
            text.chars().take(2048).collect::<String>()
        );
        assert_eq!(f.entries()[0]["size"], text.len());
    }
}

#[test]
fn unsupported_and_oversized_payloads_do_not_change_index() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    h.store("text", b"kept").unwrap();
    let before = fs::read(f.0.join("index.json")).unwrap();
    for (kind, bytes) in [
        ("text", vec![0xff]),
        ("text", Vec::new()),
        ("image", b"unsupported".to_vec()),
        ("text", vec![b'a'; MAX_BYTES + 1]),
    ] {
        h.store(kind, &bytes).unwrap();
    }
    assert_eq!(fs::read(f.0.join("index.json")).unwrap(), before);
}

#[test]
fn logical_item_and_byte_bounds_retain_newest() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    for i in 0..MAX_ITEMS + 5 {
        h.store("text", format!("item-{i}").as_bytes()).unwrap();
    }
    let e = f.entries();
    assert_eq!(e.as_array().unwrap().len(), MAX_ITEMS);
    assert_eq!(e[0]["preview"], format!("item-{}", MAX_ITEMS + 4));
    for i in 0..5 {
        let mut data = vec![b'a'; MAX_BYTES];
        data[0] += i;
        h.store("text", &data).unwrap();
    }
    let e = f.entries();
    assert_eq!(e.as_array().unwrap().len(), MAX_TOTAL / MAX_BYTES);
    assert_eq!(
        e.as_array()
            .unwrap()
            .iter()
            .map(|e| e["size"].as_u64().unwrap())
            .sum::<u64>(),
        MAX_TOTAL as u64
    );
}

#[test]
fn corrupt_metadata_cannot_escape_directory_or_replace_index() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    h.store("text", b"kept").unwrap();
    let before = fs::read(f.0.join("index.json")).unwrap();
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    db.execute(
        "INSERT INTO clips VALUES ('../escape','image/png',x'89',123.0)",
        [],
    )
    .unwrap();
    assert!(h.export().is_err());
    assert_eq!(fs::read(f.0.join("index.json")).unwrap(), before);
    assert!(h.delete("../../other").is_err());
}

#[test]
fn copy_read_does_not_republish_index_and_legacy_schema_is_accepted() {
    let f = Fixture::new();
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    db.execute_batch(
        "CREATE TABLE clips (id TEXT PRIMARY KEY, mime TEXT, payload BLOB, updated REAL)",
    )
    .unwrap();
    drop(db);
    let mut h = History::open(&f.0).unwrap();
    h.store("text", b"synthetic original").unwrap();
    let metadata = fs::metadata(f.0.join("index.json")).unwrap();
    let before = f.entries();
    assert_eq!(
        h.payload(before[0]["id"].as_str().unwrap()).unwrap().1,
        b"synthetic original"
    );
    assert_eq!(
        fs::metadata(f.0.join("index.json"))
            .unwrap()
            .modified()
            .unwrap(),
        metadata.modified().unwrap()
    );
    assert_eq!(f.entries(), before);
}

#[test]
fn publication_does_not_follow_a_temporary_symlink_and_encodes_uri() {
    let f = Fixture::new();
    let destination = f.0.join("index.json");
    std::os::unix::fs::symlink("/not-a-publication-target", f.0.join("index.json.tmp")).unwrap();
    state::atomic(&destination, b"complete", 0o600).unwrap();
    assert_eq!(fs::read(destination).unwrap(), b"complete");
    assert_eq!(
        state::file_uri(&PathBuf::from("/tmp/ä #.png")).unwrap(),
        "file:///tmp/%C3%A4%20%23.png"
    );
}

#[test]
fn interrupted_publications_are_reclaimed_on_recovery_and_clear() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    let payload = b"\x89PNG\r\n\x1a\nsynthetic private image";
    h.store("image", payload).unwrap();
    let id = f.entries()[0]["id"].as_str().unwrap().to_owned();
    let projection = f.0.join(format!("{id}.png"));
    drop(h);
    fs::remove_file(&projection).unwrap();
    let orphans = [
        format!(".{id}.png-123456-0.tmp"),
        ".index.json-123456-1.tmp".into(),
        format!("{id}.png.tmp"),
        "index.json.tmp".into(),
    ];
    for name in &orphans {
        fs::write(f.0.join(name), payload).unwrap();
    }
    let foreign = [
        ".user.png-123456-0.tmp".into(),
        ".index.json-worker-0.tmp".into(),
        ".index.json-0-0.tmp".into(),
        ".index.json-0012-0.tmp".into(),
        format!(".{id}.png-123456-sequence.tmp"),
        format!("{id}.png.tmp.backup"),
        "user-image.png".into(),
    ];
    for name in &foreign {
        fs::write(f.0.join(name), b"user-owned").unwrap();
    }
    let link = f.0.join(format!(".{id}.jpeg-123456-2.tmp"));
    std::os::unix::fs::symlink(f.0.join("user-image.png"), &link).unwrap();
    let directory = f.0.join(".index.json-123456-3.tmp");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("user-file"), b"kept").unwrap();

    let mut h = History::open(&f.0).unwrap();
    assert!(orphans.iter().all(|name| !f.0.join(name).exists()));
    h.export().unwrap();
    assert_eq!(fs::read(&projection).unwrap(), payload);
    assert_eq!(h.payload(&id).unwrap().1, payload);

    // Also reclaim a fragment encountered while the History handle is open,
    // so a successful clear cannot leave recognized payload/preview copies.
    let clear_orphans = [
        format!(".{id}.png-123456-4.tmp"),
        ".index.json-123456-5.tmp".into(),
    ];
    for name in &clear_orphans {
        fs::write(f.0.join(name), payload).unwrap();
    }
    h.clear().unwrap();
    assert_eq!(f.entries(), serde_json::json!([]));
    assert!(!projection.exists());
    assert!(clear_orphans.iter().all(|name| !f.0.join(name).exists()));
    for name in &foreign {
        assert_eq!(fs::read(f.0.join(name)).unwrap(), b"user-owned");
    }
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read(directory.join("user-file")).unwrap(), b"kept");
}

#[test]
fn unreadable_database_preserves_interrupted_publication_evidence() {
    let f = Fixture::new();
    fs::write(f.0.join("history.sqlite"), b"not a SQLite database").unwrap();
    let orphan = f.0.join(".index.json-123456-0.tmp");
    fs::write(&orphan, b"retained recovery evidence").unwrap();
    assert!(History::open(&f.0).is_err());
    assert_eq!(fs::read(orphan).unwrap(), b"retained recovery evidence");
}

fn format(mime: &str, data: impl AsRef<[u8]>) -> desktop_foundationctl::clipboard::Format {
    desktop_foundationctl::clipboard::Format {
        mime: mime.into(),
        data: data.as_ref().to_vec(),
    }
}

#[test]
fn pins_survive_recopies_clear_and_ordinary_retention() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    h.store("text", b"pinned note").unwrap();
    let pinned = f.entries()[0]["id"].as_str().unwrap().to_owned();
    h.pin(&pinned, true).unwrap();
    for i in 0..MAX_ITEMS + 5 {
        h.store("text", format!("ordinary-{i}").as_bytes()).unwrap();
    }
    let entries = f.entries();
    assert_eq!(entries.as_array().unwrap().len(), MAX_ITEMS + 1);
    assert_eq!(entries[0]["id"], pinned);
    assert_eq!(entries[0]["pinned"], true);
    h.store("text", b"pinned note").unwrap();
    assert_eq!(f.entries()[0]["pinned"], true);
    h.clear().unwrap();
    assert_eq!(f.entries().as_array().unwrap().len(), 1);
    assert_eq!(h.payload(&pinned).unwrap().1, b"pinned note");
    h.clear_all().unwrap();
    assert_eq!(f.entries(), serde_json::json!([]));
    assert!(h.payload(&pinned).is_err());
    drop(h);
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    for table in ["clips", "clip_meta", "clip_formats"] {
        let count: usize = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn pinned_item_budget_is_rejected_without_evicting_pins() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    for i in 0..MAX_ITEMS {
        h.store("text", format!("pin-{i}").as_bytes()).unwrap();
        let id = f
            .entries()
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["preview"] == format!("pin-{i}"))
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        h.pin(&id, true).unwrap();
    }
    h.store("text", b"over pin limit").unwrap();
    let id = f
        .entries()
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["preview"] == "over pin limit")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = f.entries();
    assert!(h.pin(&id, true).is_err());
    assert_eq!(before, f.entries());
    h.clear().unwrap();
    assert_eq!(f.entries().as_array().unwrap().len(), MAX_ITEMS);
    assert!(
        f.entries()
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["pinned"] == true)
    );
}

#[test]
fn pin_and_ordinary_byte_budgets_are_independent() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    let mut ids = Vec::new();
    for i in 0..MAX_TOTAL / MAX_BYTES {
        let mut bytes = vec![b'a'; MAX_BYTES];
        bytes[0] += i as u8;
        h.store("text", &bytes).unwrap();
        let entries = f.entries();
        let id = entries
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["pinned"] == false)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        h.pin(&id, true).unwrap();
        ids.push(id);
    }
    h.store("text", b"separate ordinary budget").unwrap();
    let id = f
        .entries()
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["pinned"] == false)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(h.pin(&id, true).is_err());
    for i in 0..5 {
        let mut bytes = vec![b'z'; MAX_BYTES];
        bytes[0] -= i;
        h.store("text", &bytes).unwrap();
    }
    let entries = f.entries();
    assert_eq!(entries.as_array().unwrap().len(), 8);
    for id in ids {
        assert!(h.payload(&id).is_ok());
    }
    h.clear().unwrap();
    assert_eq!(f.entries().as_array().unwrap().len(), 4);
}

#[test]
fn bundles_deduplicate_stably_and_preserve_rich_formats() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    let bundle = [
        format("text/html", b"<p><b>Hello</b> &amp; world</p>"),
        format("text/plain", b"Hello & world"),
        format("text/rtf", br"{\rtf1\ansi Hello & world}"),
    ];
    h.store_bundle(&bundle).unwrap();
    let id = f.entries()[0]["id"].as_str().unwrap().to_owned();
    h.pin(&id, true).unwrap();
    let reversed: Vec<_> = bundle.into_iter().rev().collect();
    h.store_bundle(&reversed).unwrap();
    assert_eq!(f.entries().as_array().unwrap().len(), 1);
    assert_eq!(f.entries()[0]["id"], id);
    assert_eq!(f.entries()[0]["kind"], "rich");
    assert_eq!(f.entries()[0]["label"], "Hello & world");
    assert_eq!(f.entries()[0]["pinned"], true);
    let saved = h.bundle(&id).unwrap();
    for original in reversed {
        assert!(
            saved
                .iter()
                .any(|v| v.mime == original.mime && v.data == original.data)
        );
    }
    assert_eq!(h.plain_payload(&id).unwrap(), b"Hello & world");
    // Copying only a plain fallback retains the already pinned rich item.
    h.store("text", b"Hello & world").unwrap();
    assert_eq!(f.entries().as_array().unwrap().len(), 1);
    assert_eq!(h.bundle(&id).unwrap().len(), 3);
}

#[test]
fn html_and_rtf_only_offers_get_safe_plain_fallbacks() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    let html = b"<style>hidden</style><script>also hidden</script><p>Good <b>text</b> &amp; &#x1f642;</p><p>Next</p>";
    h.store_bundle(&[format("text/html", html)]).unwrap();
    let id = f.entries()[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        h.plain_payload(&id).unwrap(),
        "Good text & 🙂\nNext".as_bytes()
    );
    assert!(
        h.bundle(&id)
            .unwrap()
            .iter()
            .any(|v| v.mime == "text/html" && v.data == html)
    );
    assert_eq!(f.entries()[0]["kind"], "rich");
    let rtf = br"{\rtf1\ansi{\fonttbl{\f0 Hidden font;}}Hello \b bold\b0\par Next \u228?}";
    h.store_bundle(&[format("application/rtf", rtf)]).unwrap();
    let id = f.entries()[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        h.plain_payload(&id).unwrap(),
        "Hello bold\nNext ä".as_bytes()
    );
    assert!(
        !f.entries()[0]["preview"]
            .as_str()
            .unwrap()
            .contains("Hidden")
    );
}

#[test]
fn arbitrary_files_folders_and_multiple_references_preserve_native_formats() {
    let f = Fixture::new();
    let originals = f.0.join("originals");
    fs::create_dir(&originals).unwrap();
    let names = [
        "video.mp4",
        "archive.zip",
        "photo.png",
        "ä space #.toml",
        "readme.md",
    ];
    let mut uris = Vec::new();
    for name in names {
        let path = originals.join(name);
        fs::write(&path, b"not examined by clipboard").unwrap();
        uris.push(state::file_uri(&path).unwrap());
    }
    let folder = originals.join("folder");
    fs::create_dir(&folder).unwrap();
    uris.push(state::file_uri(&folder).unwrap());
    let uri_list = format!("# Copied files\r\n{}\r\n", uris.join("\r\n"));
    let gnome = format!("cut\n{}\n", uris.join("\n"));
    let bundle = [
        format("text/uri-list", &uri_list),
        format("x-special/gnome-copied-files", &gnome),
        format("application/x-kde-cutselection", b"1"),
    ];
    let mut h = History::open(&f.0).unwrap();
    h.store_bundle(&bundle).unwrap();
    let entries = f.entries();
    let id = entries[0]["id"].as_str().unwrap();
    assert_eq!(entries[0]["kind"], "files");
    assert_eq!(entries[0]["fileCount"], 6);
    assert_eq!(entries[0]["unavailable"], false);
    assert_eq!(entries[0]["detail"], "5 files · 1 folder");
    assert!(
        entries[0]["preview"]
            .as_str()
            .unwrap()
            .contains("ä space #.toml")
    );
    assert!(entries[0]["size"].as_u64().unwrap() < 4096);
    let saved = h.bundle(id).unwrap();
    assert!(
        saved
            .iter()
            .any(|v| v.mime == "text/uri-list" && v.data == uri_list.as_bytes())
    );
    assert!(
        saved
            .iter()
            .any(|v| v.mime == "x-special/gnome-copied-files"
                && v.data == format!("copy\n{}\n", uris.join("\n")).as_bytes())
    );
    assert!(
        saved
            .iter()
            .any(|v| v.mime == "application/x-kde-cutselection" && v.data == b"0")
    );
    assert!(h.plain_payload(id).is_err());
}

#[test]
fn unavailable_files_refresh_without_forgetting_pins_and_copy_fails() {
    let f = Fixture::new();
    let path = f.0.join("temporary video.mkv");
    fs::write(&path, b"video").unwrap();
    let mut h = History::open(&f.0).unwrap();
    h.store_bundle(&[format("text/uri-list", state::file_uri(&path).unwrap())])
        .unwrap();
    let id = f.entries()[0]["id"].as_str().unwrap().to_owned();
    h.pin(&id, true).unwrap();
    fs::remove_file(&path).unwrap();
    h.export().unwrap();
    assert_eq!(f.entries()[0]["unavailable"], true);
    assert_eq!(f.entries()[0]["pinned"], true);
    assert!(h.bundle(&id).is_err());
    h.clear().unwrap();
    assert_eq!(f.entries().as_array().unwrap().len(), 1);
    fs::write(&path, b"restored video").unwrap();
    h.export().unwrap();
    assert_eq!(f.entries()[0]["unavailable"], false);
    assert!(h.bundle(&id).is_ok());
}

#[test]
fn malformed_or_remote_file_bundles_do_not_replace_index() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    h.store("text", b"kept").unwrap();
    let before = f.entries();
    for uri in [
        "https://example.com/file",
        "file://remote-server/path",
        "file:///path%00suffix",
        "file:///path%GG",
        "file:///path#fragment",
        "file:relative",
    ] {
        assert!(
            h.store_bundle(&[format("text/uri-list", uri)]).is_err(),
            "{uri}"
        );
        assert_eq!(f.entries(), before);
    }
    assert!(
        h.store_bundle(&[format("application/x-kde-cutselection", b"1")])
            .is_err()
    );
    assert!(
        h.store_bundle(&[
            format("text/uri-list", "file:///one"),
            format("x-special/gnome-copied-files", "copy\nfile:///two")
        ])
        .is_err()
    );
    assert_eq!(f.entries(), before);
}

#[test]
fn legacy_migration_retains_exact_ids_and_four_column_schema() {
    let f = Fixture::new();
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    db.execute_batch(
        "CREATE TABLE clips (id TEXT PRIMARY KEY, mime TEXT, payload BLOB, updated REAL)",
    )
    .unwrap();
    let old_id = "a".repeat(64);
    db.execute(
        "INSERT INTO clips VALUES (?,'text/plain;charset=utf-8',?,123.0)",
        rusqlite::params![old_id, b"legacy retained".as_slice()],
    )
    .unwrap();
    drop(db);
    let h = History::open(&f.0).unwrap();
    h.export().unwrap();
    assert_eq!(f.entries()[0]["id"], old_id);
    assert_eq!(h.payload(&old_id).unwrap().1, b"legacy retained");
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(clips)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(columns, ["id", "mime", "payload", "updated"]);
    assert_eq!(f.entries()[0]["pinned"], false);
}

#[test]
fn corrupt_migration_is_transactional_and_bundle_metadata_is_defensive() {
    let f = Fixture::new();
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE clips (id TEXT PRIMARY KEY, mime TEXT, payload BLOB, updated REAL); INSERT INTO clips VALUES ('broken','text/plain;charset=utf-8',x'61',123.0)").unwrap();
    assert!(History::open(&f.0).is_err());
    let tables: usize = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('clip_meta','clip_formats')", [], |r| r.get(0)).unwrap();
    assert_eq!(tables, 0);
    db.execute("DELETE FROM clips", []).unwrap();
    drop(db);
    let mut h = History::open(&f.0).unwrap();
    h.store("text", b"kept").unwrap();
    let before = f.entries();
    let id = before[0]["id"].as_str().unwrap();
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    db.execute(
        "INSERT INTO clip_formats(id,mime,data) VALUES (?2,'text/plain',zeroblob(?1))",
        rusqlite::params![MAX_BYTES + 1, id],
    )
    .unwrap();
    assert!(h.bundle(id).is_err());
    assert!(h.export().is_err());
    assert_eq!(f.entries(), before);
}

#[test]
fn primary_payload_is_stored_once_and_rollback_writer_rows_are_adopted() {
    let f = Fixture::new();
    let mut h = History::open(&f.0).unwrap();
    h.store("image", b"\x89PNG\r\n\x1a\nonly stored once")
        .unwrap();
    let image_id = f.entries()[0]["id"].as_str().unwrap().to_owned();
    h.pin(&image_id, true).unwrap();
    h.store_bundle(&[
        format("text/html", b"<b>Rich</b>"),
        format("text/plain", b"Rich"),
    ])
    .unwrap();
    drop(h);
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    let image_copies: usize = db
        .query_row(
            "SELECT count(*) FROM clip_formats WHERE id=?",
            [&image_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(image_copies, 0);
    let extras: usize = db
        .query_row("SELECT count(*) FROM clip_formats", [], |r| r.get(0))
        .unwrap();
    assert_eq!(extras, 1);
    // A previous four-column-only worker can add or remove ordinary rows.
    db.execute("DELETE FROM clips WHERE id!=?", [&image_id])
        .unwrap();
    let legacy_id = "b".repeat(64);
    db.execute(
        "INSERT INTO clips VALUES (?,'text/plain;charset=utf-8',?,321.0)",
        rusqlite::params![legacy_id, b"from old worker".as_slice()],
    )
    .unwrap();
    drop(db);
    let h = History::open(&f.0).unwrap();
    h.export().unwrap();
    let entries = f.entries();
    assert_eq!(entries.as_array().unwrap().len(), 2);
    assert_eq!(entries[0]["id"], image_id);
    assert_eq!(entries[0]["pinned"], true);
    assert_eq!(h.payload(&legacy_id).unwrap().1, b"from old worker");
    let db = Connection::open(f.0.join("history.sqlite")).unwrap();
    let extra_copies: usize = db
        .query_row("SELECT count(*) FROM clip_formats", [], |r| r.get(0))
        .unwrap();
    assert_eq!(extra_copies, 0);
}
