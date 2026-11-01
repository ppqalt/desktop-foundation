//! Finite process CPU, context-switch and memory measurements.
use crate::{Result, invalid};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Sample {
    ticks: u64,
    switches: BTreeMap<String, u64>,
    memory: BTreeMap<String, u64>,
}

fn fields(text: &str, names: &[&str], suffix: &str) -> Result<BTreeMap<String, u64>> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, value)) = line.split_once(':')
            && names.contains(&name)
        {
            let number = value
                .split_whitespace()
                .next()
                .ok_or_else(|| invalid("Missing process counter"))?
                .parse::<u64>()
                .map_err(|_| invalid("Invalid process counter"))?;
            values.insert(format!("{name}{suffix}"), number);
        }
    }
    Ok(values)
}

fn sample(proc: &Path) -> Result<Sample> {
    let stat = fs::read_to_string(proc.join("stat"))?;
    let rest = stat
        .rsplit_once(')')
        .ok_or_else(|| invalid("Invalid process stat"))?
        .1;
    let numbers: Vec<_> = rest.split_whitespace().collect();
    let ticks = [11, 12].iter().try_fold(0_u64, |sum, &i| {
        let value = numbers
            .get(i)
            .ok_or_else(|| invalid("Incomplete process stat"))?
            .parse::<u64>()
            .map_err(|_| invalid("Invalid process CPU counter"))?;
        sum.checked_add(value)
            .ok_or_else(|| invalid("Process CPU counter overflow"))
    })?;
    let switches = fields(
        &fs::read_to_string(proc.join("status"))?,
        &["voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"],
        "",
    )?;
    let memory = fields(
        &fs::read_to_string(proc.join("smaps_rollup"))?,
        &["Rss", "Pss"],
        "_KiB",
    )?;
    Ok(Sample {
        ticks,
        switches,
        memory,
    })
}

pub fn run(args: &[String]) -> Result<()> {
    let mut pid = None;
    let mut seconds = 10.0_f64;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--seconds" {
            i += 1;
            seconds = args
                .get(i)
                .ok_or_else(|| invalid("--seconds requires a value"))?
                .parse()
                .map_err(|_| invalid("Invalid seconds"))?;
        } else if let Some(value) = args[i].strip_prefix("--seconds=") {
            seconds = value.parse().map_err(|_| invalid("Invalid seconds"))?;
        } else if pid.is_none() {
            pid = Some(args[i].parse::<u32>().map_err(|_| invalid("Invalid PID"))?);
        } else {
            return Err(invalid("Expected baseline PID [--seconds SECONDS]"));
        }
        i += 1;
    }
    let pid = pid
        .filter(|p| *p > 0)
        .ok_or_else(|| invalid("Expected baseline PID [--seconds SECONDS]"))?;
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 60.0 {
        return Err(invalid("seconds must be in (0, 60]"));
    }
    let proc = std::path::PathBuf::from(format!("/proc/{pid}"));
    let start = Instant::now();
    let before = sample(&proc)?;
    thread::sleep(Duration::from_secs_f64(seconds));
    let after = sample(&proc)?;
    let elapsed = start.elapsed().as_secs_f64();
    // SAFETY: sysconf takes a constant query and has no pointer arguments.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if hz <= 0 {
        return Err(invalid("Cannot determine system clock tick rate"));
    }
    let mut switches = BTreeMap::new();
    for (key, value) in &before.switches {
        let end = after
            .switches
            .get(key)
            .ok_or_else(|| invalid("Process context counter disappeared"))?;
        switches.insert(
            key,
            end.checked_sub(*value)
                .ok_or_else(|| invalid("Process counters reset"))? as f64
                / elapsed,
        );
    }
    let ticks = after
        .ticks
        .checked_sub(before.ticks)
        .ok_or_else(|| invalid("Process counters reset"))?;
    let mut result = json!({"pid":pid,"duration_s":elapsed,"cpu_percent_one_core":100.0 * ticks as f64 / hz as f64 / elapsed,"main_thread_context_switches_per_s":switches});
    for (key, value) in after.memory {
        result[key] = json!(value);
    }
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_counters_and_ignores_other_fields() {
        let result = fields(
            "Name:\tx\nvoluntary_ctxt_switches:\t12\nnonvoluntary_ctxt_switches:\t3\n",
            &["voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"],
            "",
        )
        .unwrap();
        assert_eq!(result["voluntary_ctxt_switches"], 12);
        assert_eq!(result["nonvoluntary_ctxt_switches"], 3);
        assert!(fields("Rss: nope kB", &["Rss"], "_KiB").is_err());
    }
    #[test]
    fn rejects_invalid_intervals_and_pid() {
        for args in [
            vec!["0", "--seconds", "1"],
            vec!["1", "--seconds", "nan"],
            vec!["1", "--seconds", "61"],
        ] {
            assert!(run(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }
}
