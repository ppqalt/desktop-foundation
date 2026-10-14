use desktop_foundationctl::clipboard::{Format, History};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "foundation-pin-isolation-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn entries(&self) -> Vec<Value> {
        serde_json::from_slice(&fs::read(self.0.join("index.json")).unwrap()).unwrap()
    }

    fn pin_states(&self) -> BTreeMap<String, bool> {
        self.entries()
            .into_iter()
            .map(|entry| {
                (
                    entry["id"].as_str().unwrap().to_owned(),
                    entry["pinned"].as_bool().unwrap(),
                )
            })
            .collect()
    }

    fn add_pins(&self, history: &mut History) -> Vec<String> {
        [
            "first independent pin",
            "middle independent pin",
            "last independent pin",
        ]
        .into_iter()
        .map(|text| {
            history.store("text", text.as_bytes()).unwrap();
            let id = self
                .entries()
                .into_iter()
                .find(|entry| entry["preview"] == text)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned();
            history.pin(&id, true).unwrap();
            id
        })
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn first_middle_and_last_pins_toggle_independently_across_export_and_reopen() {
    let fixture = Fixture::new();
    let mut history = History::open(&fixture.0).unwrap();
    let ids = fixture.add_pins(&mut history);
    let mut expected: BTreeMap<_, _> = ids.iter().map(|id| (id.clone(), true)).collect();

    for id in &ids {
        history.pin(id, false).unwrap();
        expected.insert(id.clone(), false);
        history.export().unwrap();
        assert_eq!(fixture.pin_states(), expected);
        drop(history);
        history = History::open(&fixture.0).unwrap();
        history.export().unwrap();
        assert_eq!(fixture.pin_states(), expected);

        history.pin(id, true).unwrap();
        expected.insert(id.clone(), true);
        assert_eq!(fixture.pin_states(), expected);
    }
    history.clear_all().unwrap();
    drop(history);
    History::open(&fixture.0).unwrap().export().unwrap();
    assert!(fixture.entries().is_empty());
}

#[test]
fn deleting_one_pin_and_clearing_history_leave_the_other_pins_intact() {
    let fixture = Fixture::new();
    let mut history = History::open(&fixture.0).unwrap();
    let ids = fixture.add_pins(&mut history);
    history.store("text", b"ordinary history").unwrap();
    history.delete(&ids[1]).unwrap();
    history.clear().unwrap();
    drop(history);
    let history = History::open(&fixture.0).unwrap();
    history.export().unwrap();
    assert_eq!(
        fixture.pin_states(),
        [(ids[0].clone(), true), (ids[2].clone(), true)].into()
    );
    assert_eq!(
        history.plain_payload(&ids[0]).unwrap(),
        b"first independent pin"
    );
    assert_eq!(
        history.plain_payload(&ids[2]).unwrap(),
        b"last independent pin"
    );
}

#[test]
fn replaying_rich_plain_fallback_does_not_change_other_pin_states() {
    let fixture = Fixture::new();
    let mut history = History::open(&fixture.0).unwrap();
    let ids = fixture.add_pins(&mut history);
    history.pin(&ids[1], false).unwrap();
    let bundle = [
        Format {
            mime: "text/html".into(),
            data: b"<p><b>Formatted pin</b></p>".to_vec(),
        },
        Format {
            mime: "text/plain".into(),
            data: b"Formatted pin".to_vec(),
        },
    ];
    history.store_bundle(&bundle).unwrap();
    let rich = fixture
        .entries()
        .into_iter()
        .find(|entry| entry["kind"] == "rich")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    history.pin(&rich, true).unwrap();
    let expected = fixture.pin_states();
    history
        .store("text", &history.plain_payload(&rich).unwrap())
        .unwrap();
    history.store_bundle(&bundle).unwrap();
    drop(history);
    let history = History::open(&fixture.0).unwrap();
    history.export().unwrap();
    assert_eq!(fixture.pin_states(), expected);
    assert_eq!(history.bundle(&rich).unwrap().len(), 2);
    assert_eq!(history.plain_payload(&rich).unwrap(), b"Formatted pin");
}
