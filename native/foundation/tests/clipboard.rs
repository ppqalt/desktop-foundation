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
