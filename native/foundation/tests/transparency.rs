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
                    application: Some("application.name:Player A".into()),
                    volumes: vec![41288, 30001],
                },
                Stream {
                    id: 2,
                    serial: "101".into(),
                    application: Some("application.name:Player B".into()),
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
fn monitoring_only_never_changes_playback_and_restores_monitor() {
    for enabled in [false, true] {
        let mut a = Fake::new(enabled);
        let before = a.clone();
        let mut j = transparency::activate_mode(&mut a, &Config::default(), false).unwrap();
        assert!(a.monitor.enabled);
        assert_eq!(a.streams, before.streams);
        assert!(j.streams.is_empty());
        a.streams.push(Stream {
            id: 3,
            serial: "102".into(),
            application: Some("application.name:Player C".into()),
            volumes: vec![55555],
        });
        transparency::restore(&mut a, &mut j, 0).unwrap();
        assert_eq!(a.monitor, before.monitor);
        assert_eq!(a.streams[0..2], before.streams);
        assert_eq!(a.streams[2].volumes, vec![55555]);
        assert!(!a.writes.iter().any(|w| w == "stream"));
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
        application: Some("application.name:Different player".into()),
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
        monitoring_gain: 5.0,
        ..Config::default()
    };
    assert!(transparency::activate(&mut a, &c).is_err());
    assert!(a.writes.is_empty());
}

#[test]
fn socket_owner_has_crash_recovery_and_only_conditional_login_recovery() {
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
    let target = unit("desktop-foundation-session.target");
    assert!(!target.contains("desktop-foundation-transparency.service"));
    assert!(target.contains("desktop-foundation-transparency-resume.service"));
    assert!(unit("desktop-foundation-transparency-resume.service").contains(" resume"));
    assert!(unit("desktop-foundation-transparency.service").contains("StartLimitBurst=3"));
}

#[test]
fn recreated_playback_keeps_its_original_level_while_transparency_is_on() {
    let mut a = Fake::new(false);
    let original = a.streams[0].volumes.clone();
    let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
    let mut recreated = a.streams.remove(0);
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    recreated.id = 9;
    recreated.serial = "900".into();
    let attenuated = recreated.volumes.clone();
    a.streams.push(recreated);
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    assert_eq!(
        a.streams.last().unwrap().volumes,
        attenuated,
        "must not attenuate twice"
    );
    transparency::restore(&mut a, &mut j, 0).unwrap();
    assert_eq!(a.streams.last().unwrap().volumes, original);
}

#[test]
fn recreated_playback_with_a_different_volume_is_not_forced_to_the_old_level() {
    let mut a = Fake::new(false);
    let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
    let mut recreated = a.streams.remove(0);
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    recreated.id = 9;
    recreated.serial = "900".into();
    recreated.volumes = vec![20000, 30000];
    a.streams.push(recreated);
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    assert_eq!(a.streams.last().unwrap().volumes, vec![5000, 7500]);
    transparency::restore(&mut a, &mut j, 0).unwrap();
    assert_eq!(a.streams.last().unwrap().volumes, vec![20000, 30000]);
}

#[test]
fn ambiguous_recreated_stream_is_not_ducked_again() {
    let mut a = Fake::new(false);
    a.streams[1].application = a.streams[0].application.clone();
    a.streams[1].volumes = a.streams[0].volumes.clone();
    let mut j = transparency::activate(&mut a, &Config::default()).unwrap();
    let mut recreated = a.streams.remove(0);
    a.streams.clear();
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    recreated.id = 42;
    recreated.serial = "420".into();
    let attenuated = recreated.volumes.clone();
    a.streams.push(recreated);
    transparency::reconcile(&mut a, &mut j, &Config::default()).unwrap();
    assert_eq!(a.streams[0].volumes, attenuated);
}

#[test]
fn legacy_journal_restores_live_streams_with_new_application_metadata() {
    let mut audio = Fake::new(false);
    let before = audio.clone();
    let journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    let mut legacy = serde_json::to_value(journal).unwrap();
    for owned in legacy["streams"].as_object_mut().unwrap().values_mut() {
        for name in ["original", "managed", "pending"] {
            if let Some(stream) = owned[name].as_object_mut() {
                stream.remove("application");
            }
        }
    }
    let mut recovered: Journal = serde_json::from_value(legacy).unwrap();
    transparency::restore(&mut audio, &mut recovered, 0).unwrap();
    assert_eq!(audio.streams, before.streams);
    assert_eq!(audio.monitor, before.monitor);
}

#[test]
fn simultaneous_recreated_streams_do_not_reuse_one_ambiguous_history() {
    let mut audio = Fake::new(false);
    let mut journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    let mut first = audio.streams.remove(0);
    first.id = 10;
    first.serial = "new-one".into();
    let mut second = first.clone();
    second.id = 11;
    second.serial = "new-two".into();
    let inherited = first.volumes.clone();
    audio.streams.extend([first, second]);
    transparency::reconcile(&mut audio, &mut journal, &Config::default()).unwrap();
    assert_eq!(audio.streams[1].volumes, inherited);
    assert_eq!(audio.streams[2].volumes, inherited);
}

#[test]
fn legacy_journal_does_not_override_a_manual_volume_change() {
    let mut audio = Fake::new(false);
    let journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    let mut legacy = serde_json::to_value(journal).unwrap();
    for owned in legacy["streams"].as_object_mut().unwrap().values_mut() {
        for name in ["original", "managed", "pending"] {
            if let Some(stream) = owned[name].as_object_mut() {
                stream.remove("application");
            }
        }
    }
    audio.streams[0].volumes = vec![24000, 18000];
    let mut recovered: Journal = serde_json::from_value(legacy).unwrap();
    transparency::restore(&mut audio, &mut recovered, 0).unwrap();
    assert_eq!(audio.streams[0].volumes, vec![24000, 18000]);
    assert_eq!(audio.streams[1].volumes, vec![10000]);
}

#[test]
fn restored_stream_recreated_before_shutdown_recovers_original_volume() {
    let mut audio = Fake::new(false);
    let before = audio.streams[0].volumes.clone();
    let mut journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    audio.streams[0].id = 14;
    audio.streams[0].serial = "recreated-before-shutdown".into();
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    assert_eq!(audio.streams[0].volumes, before);
}

#[test]
fn absent_application_restores_after_mode_is_off_and_journal_is_reloaded() {
    let mut audio = Fake::new(false);
    let original_monitor = audio.monitor.clone();
    let original_volume = audio.streams[0].volumes.clone();
    let mut journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    let mut returning = audio.streams.remove(0);
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    assert_eq!(audio.monitor, original_monitor);
    assert!(journal.monitor_restored);
    let bytes = serde_json::to_vec(audio.journal.as_ref().unwrap()).unwrap();
    let mut journal: Journal = serde_json::from_slice(&bytes).unwrap();
    returning.id = 50;
    returning.serial = "returned-after-off".into();
    audio.streams.push(returning);
    audio.api_down = true; // Restoring playback must no longer require EasyEffects.
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    assert_eq!(audio.streams.last().unwrap().volumes, original_volume);
    assert!(audio.journal.is_none());
}

#[test]
fn returning_application_keeps_a_new_volume_after_mode_is_off() {
    let mut audio = Fake::new(false);
    let mut journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    let mut returning = audio.streams.remove(0);
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    returning.id = 51;
    returning.serial = "changed-after-off".into();
    returning.volumes = vec![15000, 23000];
    audio.streams.push(returning);
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    assert_eq!(audio.streams.last().unwrap().volumes, vec![15000, 23000]);
    assert!(audio.journal.is_none());
}

#[test]
fn login_recovery_without_a_journal_never_starts_an_audio_owner() {
    let base =
        std::env::temp_dir().join(format!("df-transparency-no-journal-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["transparency", "resume"])
        .env("XDG_STATE_HOME", &base)
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    std::fs::remove_dir_all(base).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn interrupted_volume_write_survives_absence_during_recovery() {
    let mut audio = Fake::new(false);
    let original = audio.streams[0].volumes.clone();
    let mut journal = transparency::activate(&mut audio, &Config::default()).unwrap();
    let mut returning = audio.streams.remove(0);
    let owned = journal.streams.get_mut("1:100").unwrap();
    owned.pending = Some(returning.clone());
    owned.managed = owned.original.clone();
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    assert!(audio.journal.is_some());
    returning.id = 52;
    returning.serial = "interrupted-return".into();
    audio.streams.push(returning);
    transparency::restore(&mut audio, &mut journal, 0).unwrap();
    assert_eq!(audio.streams.last().unwrap().volumes, original);
    assert!(audio.journal.is_none());
}
