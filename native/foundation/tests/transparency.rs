//! Inert audio fixtures: no PipeWire, microphone transmission or live volume changes.
use desktop_foundationctl::{
    Result, invalid,
    transparency::{self, Audio, Config, Journal, Monitor, Stream},
};
#[derive(Clone)]
struct Fake {
    monitor: Monitor,
    streams: Vec<Stream>,
    journal: Option<Journal>,
    missing: bool,
    api_down: bool,
    path: bool,
    fail_set: bool,
    writes: Vec<String>,
}
impl Fake {
    fn new(enabled: bool) -> Self {
        Self {
            monitor: Monitor {
                instance: "ee:1".into(),
                enabled,
                gain: 0.72,
            },
            streams: vec![
                Stream {
                    id: 1,
                    serial: "100".into(),
                    volumes: vec![41288, 30001],
                },
                Stream {
                    id: 2,
                    serial: "101".into(),
                    volumes: vec![10000],
                },
            ],
            journal: None,
            missing: false,
            api_down: false,
            path: true,
            fail_set: false,
            writes: vec![],
        }
    }
}
impl Audio for Fake {
    fn monitor(&mut self) -> Result<Monitor> {
        if self.api_down {
            Err(invalid("EE unavailable"))
        } else {
            Ok(self.monitor.clone())
        }
    }
    fn devices(&mut self, _: &str) -> Result<(String, String)> {
        if self.missing {
            Err(invalid("No TC30"))
        } else {
            Ok(("tc30".into(), "daybreak".into()))
        }
    }
    fn streams(&mut self) -> Result<Vec<Stream>> {
        Ok(self.streams.clone())
    }
    fn set_enabled(&mut self, value: bool) -> Result<()> {
        self.writes.push(format!("monitor:{value}"));
        self.monitor.enabled = value;
        Ok(())
    }
    fn set_gain(&mut self, value: f64) -> Result<()> {
        self.writes.push(format!("gain:{value}"));
        self.monitor.gain = value;
        Ok(())
    }
    fn set_stream(&mut self, expected: &Stream, next: &Stream) -> Result<()> {
        if self.fail_set {
            self.fail_set = false;
            return Err(invalid("fixture setter failure"));
        }
        let s = self
            .streams
            .iter_mut()
            .find(|s| s.id == next.id && s.serial == next.serial)
            .ok_or_else(|| invalid("gone"))?;
        if s != expected {
            return Err(invalid("manual conflict"));
        }
        *s = next.clone();
        self.writes.push("stream".into());
        Ok(())
    }
    fn verify_path(&mut self, _: &str, _: &str) -> Result<()> {
        if self.path {
            Ok(())
        } else {
            Err(invalid("monitor path missing"))
        }
    }
    fn persist(&mut self, j: Option<&Journal>) -> Result<()> {
        self.journal = j.cloned();
        Ok(())
    }
    fn wait(&mut self, _: u64) {}
}
#[test]
fn exact_restoration_for_initially_enabled_and_disabled_monitor() {
    for enabled in [false, true] {
        let mut a = Fake::new(enabled);
        let before = a.clone();
        let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
        assert!(a.monitor.enabled);
        assert_eq!(a.monitor.gain, 0.5);
        assert_eq!(a.streams[0].volumes, vec![10322, 7500]);
        transparency::restore(&mut a, &mut j, 0).unwrap();
        assert_eq!(a.streams, before.streams);
        assert_eq!(a.monitor, before.monitor);
        assert!(a.journal.is_none());
    }
}
#[test]
fn missing_microphone_or_api_never_ducks_playback() {
    for mic in [false, true] {
        let mut a = Fake::new(false);
        a.missing = mic;
        a.api_down = !mic;
        let before = a.streams.clone();
        assert!(transparency::activate(&mut a, &Config::default()).is_err());
        assert_eq!(a.streams, before);
        assert!(a.writes.is_empty());
    }
}
#[test]
fn failed_monitor_path_or_stream_change_rolls_back_every_owned_value() {
    for path in [false, true] {
        let mut a = Fake::new(false);
        let before = a.clone();
        a.path = path;
        a.fail_set = path;
        assert!(transparency::activate(&mut a, &Config::default()).is_err());
        assert_eq!(a.streams, before.streams);
        assert_eq!(a.monitor, before.monitor);
        assert!(a.journal.is_none());
    }
}
#[test]
fn user_volume_changes_are_not_overwritten_or_reducked() {
    let mut a = Fake::new(true);
    let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
    a.streams[0].volumes = vec![12345, 23456];
    a.monitor.gain = 0.3;
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    assert_eq!(a.streams[0].volumes, vec![12345, 23456]);
    transparency::restore(&mut a, &mut j, 0).unwrap();
    assert_eq!(a.streams[0].volumes, vec![12345, 23456]);
    assert_eq!(a.monitor.gain, 0.3);
    assert_eq!(a.streams[1].volumes, vec![10000]);
}
#[test]
fn stream_arrival_departure_and_reused_ids_have_independent_ownership() {
    let mut a = Fake::new(false);
    let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
    a.streams.remove(0);
    a.streams.push(Stream {
        id: 1,
        serial: "200".into(),
        volumes: vec![63000],
    });
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    assert_eq!(a.streams[1].volumes, vec![15750]);
    transparency::restore(&mut a, &mut j, 0).unwrap();
    assert_eq!(a.streams[1].volumes, vec![63000]);
}
#[test]
fn crash_between_journal_and_write_restores_the_previous_or_pending_value() {
    for written in [false, true] {
        let mut a = Fake::new(false);
        let before = a.clone();
        let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
        let s = a.streams[0].clone();
        let mut pending = s.clone();
        pending.volumes = vec![2000, 3000];
        j.streams.get_mut("1:100").unwrap().pending = Some(pending.clone());
        a.persist(Some(&j)).unwrap();
        if written {
            a.streams[0] = pending;
        }
        let mut recovered = a.journal.clone().unwrap();
        transparency::restore(&mut a, &mut recovered, 0).unwrap();
        assert_eq!(a.streams, before.streams);
        assert_eq!(a.monitor, before.monitor);
    }
}
#[test]
fn repeated_activation_deactivation_and_recovery_are_idempotent() {
    let mut a = Fake::new(false);
    let before = a.clone();
    for _ in 0..20 {
        let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
        transparency::restore(&mut a, &mut j, 0).unwrap();
        transparency::restore(&mut a, &mut j, 0).unwrap();
    }
    assert_eq!(a.streams, before.streams);
    assert_eq!(a.monitor, before.monitor);
}
#[test]
fn unavailable_ee_still_restores_applications_and_keeps_recovery_journal() {
    let mut a = Fake::new(false);
    let before = a.clone();
    let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
    a.api_down = true;
    assert!(transparency::restore(&mut a, &mut j, 0).is_err());
    assert_eq!(a.streams, before.streams);
    assert!(a.journal.is_some());
    a.api_down = false;
    a.monitor.instance = "ee:2".into();
    transparency::restore(&mut a, &mut j, 0).unwrap();
    assert!(!a.monitor.enabled);
    assert_eq!(a.monitor.gain, before.monitor.gain);
    assert!(a.journal.is_none());
}
#[test]
fn invalid_config_never_changes_audio() {
    let mut a = Fake::new(false);
    let c = Config {
        monitoring_gain: 2.0,
        ..Config::default()
    };
    assert!(transparency::activate(&mut a, &c).is_err());
    assert!(a.writes.is_empty());
}

#[test]
fn socket_owner_has_crash_recovery_and_no_login_process() {
    let definitions = desktop_foundationctl::deploy_units::definitions(
        std::path::Path::new("/fixture/repo with spaces"),
        std::path::Path::new("/fixture/state"),
    );
    let unit = |name: &str| {
        definitions
            .iter()
            .find(|(n, _)| n == name)
            .unwrap()
            .1
            .as_str()
    };
    let socket = unit("desktop-foundation-transparency.socket");
    assert!(socket.contains("ListenDatagram=%t/desktop-foundation-transparency.sock"));
    assert!(socket.contains("SocketMode=0600"));
    let service = unit("desktop-foundation-transparency.service");
    assert!(
        service.contains("ExecStopPost=\"/fixture/repo with spaces/scripts/transparency\" recover")
    );
    assert!(!unit("desktop-foundation-session.target").contains("transparency"));
}
