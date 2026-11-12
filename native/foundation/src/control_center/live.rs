//! Shell-scoped read-only subscriptions. Block when idle; query only on events.
use crate::{
    Result, invalid,
    process::{StreamRead, Subscription},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

struct Source {
    key: &'static str,
    epoch: u64,
    subscription: Option<Subscription>,
    buffer: Vec<u8>,
    due: Option<Instant>,
    retry: Option<Instant>,
    backoff: u64,
    last: Option<(u64, Value)>,
}
impl Source {
    fn new(key: &'static str) -> Self {
        Self {
            key,
            epoch: 0,
            subscription: None,
            buffer: Vec::new(),
            due: None,
            retry: Some(Instant::now()),
            backoff: 1,
            last: None,
        }
    }
    fn emit(&mut self, value: Value) -> Result<()> {
        if self.last.as_ref() == Some(&(self.epoch, value.clone())) {
            return Ok(());
        }
        let epoch_key = format!("{}_epoch", self.key);
        let mut packet = json!({});
        packet[self.key] = value.clone();
        packet[epoch_key] = self.epoch.into();
        let mut out = std::io::stdout().lock();
        serde_json::to_writer(&mut out, &packet)?;
        out.write_all(b"\n")?;
        out.flush()?;
        self.last = Some((self.epoch, value));
        Ok(())
    }
    fn dirty(&mut self) {
        // Bound burst coalescing: continuous events cannot postpone forever.
        if self.subscription.is_some() {
            self.due
                .get_or_insert(Instant::now() + Duration::from_millis(35));
        }
    }
    fn disconnect(&mut self) -> Result<()> {
        self.subscription = None; // Reap only this owned process group.
        self.buffer.clear();
        self.due = None;
        self.retry = Some(Instant::now() + Duration::from_secs(self.backoff));
        self.backoff = (self.backoff * 2).min(8);
        self.emit(json!({"available":false,"error":"Event connection unavailable"}))
    }
    fn connect(&mut self) -> Result<()> {
        let args = if self.key == "audio" {
            ["pactl", "subscribe"]
        } else {
            ["nmcli", "monitor"]
        };
        match Subscription::open(&args.map(String::from)) {
            Ok(subscription) => {
                self.subscription = Some(subscription);
                self.retry = None;
                self.due = Some(Instant::now());
                Ok(())
            }
            Err(_) => self.disconnect(),
        }
    }
    fn events(&mut self) -> Result<()> {
        let result = self
            .subscription
            .as_mut()
            .unwrap()
            .read(Instant::now() + Duration::from_millis(1));
        match result {
            Ok(StreamRead::Data(bytes)) => {
                self.backoff = 1;
                self.buffer.extend(bytes);
                if self.buffer.len() > 65536 {
                    return self.disconnect();
                }
                while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<_> = self.buffer.drain(..=end).collect();
                    if relevant(self.key, &line) {
                        self.dirty();
                    }
                }
                Ok(())
            }
            Ok(StreamRead::Timeout) => Ok(()),
            _ => self.disconnect(),
        }
    }
}
fn relevant(key: &str, bytes: &[u8]) -> bool {
    if key == "network" {
        return bytes.iter().any(|b| !b.is_ascii_whitespace());
    }
    [
        b" on sink".as_slice(),
        b" on source",
        b" on server",
        b" on card",
    ]
    .iter()
    .any(|topic| bytes.windows(topic.len()).any(|part| part == *topic))
}
fn controls(buffer: &mut Vec<u8>, sources: &mut [Source; 2]) -> Result<bool> {
    loop {
        let mut bytes = [0; 8192];
        match std::io::stdin().read(&mut bytes) {
            Ok(0) => return Ok(false),
            Ok(count) => buffer.extend_from_slice(&bytes[..count]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
        if buffer.len() > 65536 {
            return Err(invalid("Control Center watch input is too large"));
        }
        while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<_> = buffer.drain(..=end).collect();
            let Ok(message) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            for source in sources.iter_mut() {
                if let Some(epoch) = message[format!("{}_epoch", source.key)].as_u64()
                    && epoch > source.epoch
                {
                    source.epoch = epoch;
                    source.dirty();
                }
                if message["refresh"] == source.key || message["refresh"] == "all" {
                    source.dirty();
                }
            }
        }
    }
    Ok(true)
}
pub fn run() -> Result<()> {
    // This stdio protocol belongs to the shell. Never change terminal flags.
    // SAFETY: isatty/fcntl/signal operate on this process's standard descriptor.
    unsafe {
        if libc::isatty(0) == 1 {
            return Err(invalid("Control Center watch requires a stdin pipe"));
        }
        let flags = libc::fcntl(0, libc::F_GETFL);
        if flags < 0 || libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
    }
    STOP.store(false, Ordering::Relaxed);
    let mut sources = [Source::new("audio"), Source::new("network")];
    let mut input = Vec::new();
    while !STOP.load(Ordering::Relaxed) {
        if !controls(&mut input, &mut sources)? {
            break;
        }
        for source in &mut sources {
            if source.retry.is_some_and(|at| at <= Instant::now()) {
                source.connect()?;
            }
        }
        let next = sources
            .iter()
            .flat_map(|s| [s.due, s.retry])
            .flatten()
            .min();
        let timeout = next.map_or(-1, |at| {
            at.saturating_duration_since(Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32
        });
        let mut fds = [
            libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: sources[0]
                    .subscription
                    .as_ref()
                    .map_or(-1, AsRawFd::as_raw_fd),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: sources[1]
                    .subscription
                    .as_ref()
                    .map_or(-1, AsRawFd::as_raw_fd),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: every subscribed descriptor stays owned throughout this poll.
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, timeout) } < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e.into());
        }
        if !controls(&mut input, &mut sources)? {
            break;
        }
        for index in 0..2 {
            if fds[index + 1].revents != 0 && sources[index].subscription.is_some() {
                sources[index].events()?;
            }
        }
        for index in 0..2 {
            if sources[index].due.is_some_and(|at| at <= Instant::now()) {
                sources[index].due = None;
                let epoch = sources[index].epoch;
                let value = if index == 0 {
                    super::audio()
                } else {
                    super::network()
                }
                .unwrap_or_else(|e| json!({"available":false,"error":e.to_string()}));
                // Commands arriving during a query invalidate its old result.
                if !controls(&mut input, &mut sources)? {
                    return Ok(());
                }
                if epoch == sources[index].epoch {
                    let available = value["available"] == true;
                    sources[index].emit(value)?;
                    if available {
                        sources[index].backoff = 1;
                    } else {
                        // A transient read failure need not produce another
                        // service event. Retry only the unavailable domain.
                        let delay = Duration::from_secs(sources[index].backoff);
                        sources[index].due.get_or_insert(Instant::now() + delay);
                        sources[index].backoff = (sources[index].backoff * 2).min(8);
                    }
                } else {
                    sources[index].dirty();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_audio_state_events_trigger_queries() {
        for name in ["sink", "source", "sink-input", "server", "card"] {
            assert!(relevant(
                "audio",
                format!("Event 'change' on {name} #5\n").as_bytes()
            ));
        }
        assert!(!relevant("audio", b"Event 'new' on client #5\n"));
        assert!(!relevant("network", b" \n"));
        assert!(relevant("network", b"wlan0: disconnected\n"));
    }
}
