//! One durable audio transaction. Adapters never own restoration policy.
mod runtime;
use crate::{Result, invalid};
pub use runtime::run;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Monitor {
    pub instance: String,
    pub enabled: bool,
    pub gain: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stream {
    pub id: u32,
    pub serial: String,
    pub volumes: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Owned<T> {
    pub original: T,
    pub managed: T,
    pub pending: Option<T>,
    pub released: bool,
}
impl<T: Clone + PartialEq> Owned<T> {
    fn new(original: T) -> Self {
        Self {
            managed: original.clone(),
            original,
            pending: None,
            released: false,
        }
    }
    fn owns(&self, current: &T) -> bool {
        !self.released && (&self.managed == current || self.pending.as_ref() == Some(current))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub version: u32,
    pub monitor_instance: String,
    pub microphone: String,
    pub output: String,
    pub enabled: Owned<bool>,
    pub gain: Owned<f64>,
    pub streams: BTreeMap<String, Owned<Stream>>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub microphone_match: String,
    pub playback_factor: f64,
    pub monitoring_gain: f64,
    pub fade_ms: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            microphone_match: String::new(),
            playback_factor: 0.25,
            monitoring_gain: 0.5,
            fade_ms: 80,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if !self.playback_factor.is_finite()
            || !(0.0..=1.0).contains(&self.playback_factor)
            || !self.monitoring_gain.is_finite()
            || !(0.0..=4.0).contains(&self.monitoring_gain)
            || self.fade_ms > 500
        {
            return Err(invalid("Invalid Transparency Mode configuration"));
        }
        Ok(())
    }
}
pub trait Audio {
    fn monitor(&mut self) -> Result<Monitor>;
    fn devices(&mut self, needle: &str) -> Result<(String, String)>;
    fn streams(&mut self) -> Result<Vec<Stream>>;
    fn set_enabled(&mut self, value: bool) -> Result<()>;
    fn set_gain(&mut self, value: f64) -> Result<()>;
    fn set_stream(&mut self, expected: &Stream, stream: &Stream) -> Result<()>;
    fn verify_path(&mut self, microphone: &str, output: &str) -> Result<()>;
    fn persist(&mut self, journal: Option<&Journal>) -> Result<()>;
    fn wait(&mut self, ms: u64);
}
fn identity(s: &Stream) -> String {
    format!("{}:{}", s.id, s.serial)
}
fn gain(a: &mut impl Audio, j: &mut Journal, target: f64) -> Result<()> {
    let current = a.monitor()?;
    if current.instance != j.monitor_instance {
        return Err(invalid("EasyEffects instance changed"));
    }
    if !j.gain.owns(&current.gain) {
        j.gain.released = true;
        a.persist(Some(j))?;
        return Err(invalid("Monitoring volume changed externally"));
    }
    j.gain.pending = Some(target);
    a.persist(Some(j))?;
    a.set_gain(target)?;
    let confirmed = a.monitor()?;
    if confirmed.instance != j.monitor_instance || (confirmed.gain - target).abs() > 1e-9 {
        return Err(invalid("Monitoring volume readback failed"));
    }
    j.gain.managed = confirmed.gain;
    j.gain.pending = None;
    a.persist(Some(j))?;
    Ok(())
}
fn enabled(a: &mut impl Audio, j: &mut Journal, target: bool) -> Result<()> {
    let current = a.monitor()?;
    if current.instance != j.monitor_instance {
        return Err(invalid("EasyEffects instance changed"));
    }
    if !j.enabled.owns(&current.enabled) {
        j.enabled.released = true;
        a.persist(Some(j))?;
        return Err(invalid("Monitoring changed externally"));
    }
    j.enabled.pending = Some(target);
    a.persist(Some(j))?;
    a.set_enabled(target)?;
    let confirmed = a.monitor()?;
    if confirmed.instance != j.monitor_instance || confirmed.enabled != target {
        return Err(invalid("Monitoring readback failed"));
    }
    j.enabled.managed = target;
    j.enabled.pending = None;
    a.persist(Some(j))?;
    Ok(())
}
/// New streams get an independent original. A manual edit releases that stream for this activation.
pub fn reconcile(a: &mut impl Audio, j: &mut Journal, c: &Config) -> Result<()> {
    let current = a.streams()?;
    if current.len() > 512 {
        return Err(invalid("Too many playback streams for safe restoration"));
    }
    j.streams
        .retain(|key, _| current.iter().any(|stream| identity(stream) == *key));
    a.persist(Some(j))?;
    for s in current {
        if s.volumes.is_empty() || s.volumes.len() > 64 {
            return Err(invalid("Invalid playback channel volumes"));
        }
        let key = identity(&s);
        let entry = j
            .streams
            .entry(key.clone())
            .or_insert_with(|| Owned::new(s.clone()));
        if !entry.owns(&s) {
            entry.released = true;
            a.persist(Some(j))?;
            continue;
        }
        let target = Stream {
            volumes: entry
                .original
                .volumes
                .iter()
                .map(|v| (*v as f64 * c.playback_factor).round() as u32)
                .collect(),
            ..s.clone()
        };
        if s == target {
            continue;
        }
        let start = s.volumes.clone();
        for step in 1..=4 {
            let actual = a.streams()?.into_iter().find(|s| identity(s) == key);
            let Some(actual) = actual else { break };
            if !j.streams[&key].owns(&actual) {
                j.streams.get_mut(&key).unwrap().released = true;
                a.persist(Some(j))?;
                break;
            }
            let next = Stream {
                volumes: start
                    .iter()
                    .zip(&target.volumes)
                    .map(|(x, y)| {
                        (*x as f64 + (*y as f64 - *x as f64) * step as f64 / 4.0).round() as u32
                    })
                    .collect(),
                ..s.clone()
            };
            j.streams.get_mut(&key).unwrap().pending = Some(next.clone());
            a.persist(Some(j))?;
            a.set_stream(&actual, &next)?;
            if a.streams()?
                .into_iter()
                .find(|s| identity(s) == key)
                .as_ref()
                != Some(&next)
            {
                return Err(invalid(
                    "Playback volume readback failed or stream disappeared",
                ));
            }
            let entry = j.streams.get_mut(&key).unwrap();
            entry.managed = next;
            entry.pending = None;
            a.persist(Some(j))?;
            a.wait(c.fade_ms / 4);
        }
    }
    Ok(())
}
pub fn activate(a: &mut impl Audio, c: &Config) -> Result<Journal> {
    activate_mode(a, c, true)
}
/// Start monitoring, optionally ducking application playback.
pub fn activate_mode(a: &mut impl Audio, c: &Config, duck: bool) -> Result<Journal> {
    c.validate()?;
    let (microphone, output) = a.devices(&c.microphone_match)?;
    let monitor = a.monitor()?;
    if !monitor.gain.is_finite() || !(0.0..=4.0).contains(&monitor.gain) {
        return Err(invalid("Invalid monitoring gain"));
    }
    let mut j = Journal {
        version: 1,
        monitor_instance: monitor.instance,
        microphone,
        output,
        enabled: Owned::new(monitor.enabled),
        gain: Owned::new(monitor.gain),
        streams: BTreeMap::new(),
    };
    a.persist(Some(&j))?;
    let result = (|| {
        // An inactive monitor starts at zero, before any link can become audible.
        if !monitor.enabled {
            gain(a, &mut j, 0.0)?;
            enabled(a, &mut j, true)?;
        }
        a.verify_path(&j.microphone, &j.output)?;
        let start = j.gain.managed;
        for step in 1..=4 {
            gain(
                a,
                &mut j,
                start + (c.monitoring_gain - start) * step as f64 / 4.0,
            )?;
            a.wait(c.fade_ms / 4);
        }
        if duck {
            reconcile(a, &mut j, c)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        return match restore(a, &mut j, c.fade_ms) {
            Ok(()) => Err(error),
            Err(recovery) => Err(invalid(format!("{error}; restoration pending: {recovery}"))),
        };
    }
    Ok(j)
}
/// Idempotent crash recovery: only matching stream identities and owned values are restored.
pub fn restore(a: &mut impl Audio, j: &mut Journal, fade_ms: u64) -> Result<()> {
    if j.version != 1 {
        return Err(invalid("Unknown Transparency Mode journal version"));
    }
    let mut errors = Vec::new();
    match if j.streams.is_empty() {
        Ok(Vec::new())
    } else {
        a.streams()
    } {
        Ok(streams) => {
            for current in streams {
                let key = identity(&current);
                let Some(owned) = j.streams.get(&key).cloned() else {
                    continue;
                };
                if !owned.owns(&current) {
                    continue;
                }
                let start = current.volumes.clone();
                for step in 1..=4 {
                    let result: Result<bool> = (|| {
                        let actual = a.streams()?.into_iter().find(|s| identity(s) == key);
                        let Some(actual) = actual else {
                            return Ok(false);
                        };
                        if !j.streams[&key].owns(&actual) {
                            j.streams.get_mut(&key).unwrap().released = true;
                            return Ok(false);
                        }
                        let next = Stream {
                            volumes: start
                                .iter()
                                .zip(&owned.original.volumes)
                                .map(|(x, y)| {
                                    (*x as f64 + (*y as f64 - *x as f64) * step as f64 / 4.0)
                                        .round() as u32
                                })
                                .collect(),
                            ..owned.original.clone()
                        };
                        j.streams.get_mut(&key).unwrap().pending = Some(next.clone());
                        a.persist(Some(j))?;
                        a.set_stream(&actual, &next)?;
                        let entry = j.streams.get_mut(&key).unwrap();
                        entry.managed = next;
                        entry.pending = None;
                        a.persist(Some(j))?;
                        a.wait(fade_ms / 4);
                        Ok(true)
                    })();
                    match result {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(e) => {
                            errors.push(e.to_string());
                            break;
                        }
                    }
                }
            }
        }
        Err(e) => errors.push(e.to_string()),
    }
    match a.monitor() {
        Ok(current) => {
            // EE can restart and reload our temporary setting from its own autosave.
            // Adopt only the new process identity; value ownership still decides restoration.
            if current.instance != j.monitor_instance {
                j.monitor_instance = current.instance.clone();
                a.persist(Some(j))?;
            }
            // Disable first when it was originally off; restore gain afterward in silence.
            if j.enabled.owns(&current.enabled)
                && let Err(e) = enabled(a, j, j.enabled.original)
            {
                errors.push(e.to_string());
            }
            if j.gain.owns(&current.gain) {
                let target = j.gain.original;
                let start = current.gain;
                for step in 1..=4 {
                    if let Err(e) = gain(a, j, start + (target - start) * step as f64 / 4.0) {
                        errors.push(e.to_string());
                        break;
                    }
                    a.wait(fade_ms / 4);
                }
            }
        }
        Err(e) => errors.push(e.to_string()),
    }
    if errors.is_empty() {
        a.persist(None)
    } else {
        a.persist(Some(j))?;
        Err(invalid(errors.join("; ")))
    }
}
