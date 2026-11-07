//! A finite native PipeWire loopback tapping the processed, pre-mix microphone.
use crate::{Result, invalid, process};
use serde_json::Value;
use std::{
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn run(args: &[String]) -> Result<()> {
    process::checked_capture(args, Duration::from_secs(2), 65536)?;
    Ok(())
}
fn dump() -> Result<Value> {
    let out =
        process::checked_capture(&["pw-dump".into()], Duration::from_secs(2), 4 * 1024 * 1024)?;
    Ok(serde_json::from_slice(&out.stdout)?)
}
pub struct Branch {
    child: Child,
    _demand: Demand,
    capture: String,
    playback: String,
    serial: String,
}
struct Demand(Child);
impl Drop for Demand {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}
impl Drop for Branch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Branch {
    pub fn start(output: &str, gain: f64) -> Result<Self> {
        let prefix = format!(
            "df-monitor-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let capture = format!("{prefix}-capture");
        let playback = format!("{prefix}-playback");
        // Keep EasyEffects' input pipeline active through its ordinary recording
        // interface. This stream is drained, never routed to playback: only the
        // pre-mix output-level ports below feed the audible monitoring branch.
        let demand = Demand(Command::new("pw-cat").args([
            "--record", "--target", "easyeffects_source", "--raw", "--format", "f32",
            "--rate", "48000", "--channels", "2", "--latency", "5ms",
            "--properties", &format!("node.name={prefix}-demand node.dont-reconnect=true"), "-",
        ]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?);
        let child = Command::new("pw-loopback")
            .args([
                "--channels",
                "2",
                "--latency",
                "5",
                "--playback",
                output,
                "--capture-props",
                &format!("node.name={capture} node.autoconnect=false"),
                "--playback-props",
                &format!("node.name={playback} node.autoconnect=true node.dont-reconnect=true"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut branch = Self {
            child,
            _demand: demand,
            capture,
            playback,
            serial: String::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if branch.child.try_wait()?.is_some() {
                return Err(invalid("Monitoring loopback exited during startup"));
            }
            let graph = dump()?;
            let node = graph.as_array().and_then(|rows| {
                rows.iter().find(|row| {
                    row["type"] == "PipeWire:Interface:Node"
                        && row["info"]["props"]["node.name"] == branch.playback
                })
            });
            if let Some(node) = node {
                branch.serial = node["info"]["props"]["object.serial"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .or_else(|| {
                        node["info"]["props"]["object.serial"]
                            .as_str()
                            .map(str::to_owned)
                    })
                    .ok_or_else(|| invalid("Monitoring stream has no stable serial"))?;
            }
            let ready = graph.as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["type"] == "PipeWire:Interface:Port"
                        && row["info"]["props"]["port.name"] == "input_FL"
                        && row["info"]["props"]["node.id"].as_u64().is_some_and(|id| {
                            rows.iter().any(|n| {
                                n["id"] == id && n["info"]["props"]["node.name"] == branch.capture
                            })
                        })
                })
            });
            if !branch.serial.is_empty() && ready {
                break;
            }
            if Instant::now() >= deadline {
                return Err(invalid("Monitoring ports did not become ready"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        branch.set_gain(gain)?;
        for channel in ["FL", "FR"] {
            let source = format!("ee_sie_output_level:output_{channel}");
            let target = format!("{}:input_{channel}", branch.capture);
            run(&["pw-link".into(), source, target])?;
        }
        Ok(branch)
    }
    fn node(&self) -> Result<Value> {
        let graph = dump()?;
        let row = graph
            .as_array()
            .and_then(|rows| {
                rows.iter().find(|row| {
                    row["type"] == "PipeWire:Interface:Node"
                        && row["info"]["props"]["node.name"] == self.playback
                        && row["info"]["props"]["object.serial"]
                            .to_string()
                            .trim_matches('"')
                            == self.serial
                })
            })
            .ok_or_else(|| invalid("Monitoring stream disappeared or was replaced"))?;
        Ok(row.clone())
    }
    pub fn set_gain(&self, gain: f64) -> Result<()> {
        if !gain.is_finite() || !(0.0..=4.0).contains(&gain) {
            return Err(invalid("Invalid monitoring gain"));
        }
        let node = self.node()?;
        let id = node["id"]
            .as_u64()
            .ok_or_else(|| invalid("Invalid monitoring node"))?;
        run(&[
            "pw-cli".into(),
            "set-param".into(),
            id.to_string(),
            "Props".into(),
            format!("{{ channelVolumes = [ {gain}, {gain} ] }}"),
        ])?;
        let deadline = Instant::now() + Duration::from_millis(300);
        loop {
            if (self.gain()? - gain).abs() <= 1e-6 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(invalid("Monitoring volume readback timed out"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn gain(&self) -> Result<f64> {
        let node = self.node()?;
        let props = node["info"]["params"]["Props"]
            .as_array()
            .and_then(|rows| rows.iter().find_map(|row| row["channelVolumes"].as_array()))
            .ok_or_else(|| invalid("Monitoring volume is unavailable"))?;
        if props.len() != 2 || props[0] != props[1] {
            return Err(invalid("Monitoring channel volumes changed independently"));
        }
        props[0]
            .as_f64()
            .ok_or_else(|| invalid("Invalid monitoring gain response"))
    }
    pub fn capture_name(&self) -> &str {
        &self.capture
    }
    pub fn playback_name(&self) -> &str {
        &self.playback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the private stock EasyEffects/PipeWire validation fixture"]
    fn isolated_stock_monitor_branch_gain_and_cleanup() {
        assert_eq!(std::env::var("DF_ISOLATED_AUDIO_TEST").as_deref(), Ok("1"));
        let graph = dump().unwrap();
        assert!(!graph.as_array().unwrap().iter().any(|r| {
            r["info"]["props"]["node.name"]
                .as_str()
                .is_some_and(|s| s.starts_with("alsa_") || s.starts_with("bluez_"))
        }));
        let branch = Branch::start("test_output", 0.375).unwrap();
        assert!((branch.gain().unwrap() - 0.375).abs() < 1e-6);
        if std::env::var("DF_ISOLATED_AUDIO_HOLD").as_deref() == Ok("1") {
            std::thread::sleep(Duration::from_secs(3));
        }
        branch.set_gain(1.75).unwrap();
        assert!((branch.gain().unwrap() - 1.75).abs() < 1e-6);
        let name = branch.playback_name().to_owned();
        drop(branch);
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let graph = dump().unwrap();
            if !graph
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["info"]["props"]["node.name"] == name)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "monitor nodes remained after cleanup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
