//! One GPU Screen Recorder owns the replay buffer and simultaneous recording.
use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::UnixStream,
    },
    thread,
};

fn ipc(path: &Path, name: &str, timeout: Duration) -> Result<Value> {
    let mut socket = UnixStream::connect(path)?;
    socket.set_read_timeout(Some(timeout))?;
    socket.set_write_timeout(Some(timeout))?;
    writeln!(socket, "{}", json!({"id": 1, "name": name}))?;
    let mut response = String::new();
    BufReader::new(socket)
        .take(65537)
        .read_line(&mut response)?;
    if response.len() > 65536 {
        return Err(invalid("Replay response is too large"));
    }
    let value: Value = serde_json::from_str(&response)?;
    if value["id"] != 1 || value["result"] != "ok" {
        return Err(invalid(format!(
            "Replay {name} failed: {}",
            value["data"].as_str().unwrap_or("invalid response")
        )));
    }
    Ok(value["data"].clone())
}

fn no_retained_output(staging: &Path, prefix: &str) -> Result<()> {
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with(prefix)
            && entry.path().extension().is_some_and(|ext| ext == "mkv")
        {
            return Err(invalid(format!(
                "Previous unvalidated {prefix} file retained in {}; resolve it before saving again",
                staging.display()
            )));
        }
    }
    Ok(())
}

fn publish_saved(path: &Value, staging: &Path, output: &Path, kind: &str) -> Result<PathBuf> {
    let source = PathBuf::from(
        path.as_str()
            .ok_or_else(|| invalid("Recorder returned no saved file"))?,
    );
    if source.parent() != Some(staging) || source.extension().is_none_or(|ext| ext != "mkv") {
        return Err(invalid(
            "Recorder returned a file outside its private session",
        ));
    }
    let meta = fs::symlink_metadata(&source)?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } {
        return Err(invalid("Saved recording is not an owned file"));
    }
    finalized(&source, false)?;
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600))?;
    // hard_link is atomic and fails on an existing destination; no recording is overwritten.
    for _ in 0..100 {
        let destination = output.join(readable_name(kind)?);
        match fs::hard_link(&source, &destination) {
            Ok(()) => {
                fs::remove_file(&source)?;
                return Ok(destination);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                std::thread::sleep(Duration::from_micros(2))
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(invalid("Could not reserve a unique recording name"))
}

fn save_replay(
    state: &mut Value,
    ipc_path: &Path,
    staging: &Path,
    output: &Path,
    timeout: Duration,
) -> Result<()> {
    state["save_status"] = "saving".into();
    state
        .as_object_mut()
        .expect("recording state object")
        .remove("save_error");
    publish(state)?;
    let result = no_retained_output(staging, "Replay_")
        .and_then(|()| ipc(ipc_path, "save-replay", timeout))
        .and_then(|v| publish_saved(&v, staging, output, "Replay"));
    match result {
        Ok(path) => {
            state["save_status"] = "saved".into();
            state["saved_path"] = path.to_string_lossy().to_string().into();
            publish(state)?;
            notify(&format!("Saved {}", path.display()));
        }
        Err(error) => {
            state["save_status"] = "error".into();
            state["save_error"] = error.to_string().into();
            publish(state)?;
            notify(&format!("Replay not saved: {error}"));
        }
    }
    Ok(())
}

pub(super) fn run(root: &Path, socket: &UnixDatagram, save_initial: bool) -> Result<bool> {
    capabilities()?;
    let help = process::run(
        &["gpu-screen-recorder".into(), "--help".into()],
        None,
        Duration::from_secs(2),
        true,
    )?;
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&help.stdout),
        String::from_utf8_lossy(&help.stderr)
    );
    for option in ["-ipc", "-ro", "-replay-storage", "-bm"] {
        if !help.contains(option) {
            return Err(invalid(format!(
                "GPU Screen Recorder lacks required replay option {option}"
            )));
        }
    }
    let (c, _) = settings(root)?;
    let display = output().unwrap_or(Value::Null);
    let output_dir = if c.directory.is_empty() {
        video_directory()?
    } else {
        PathBuf::from(&c.directory)
    };
    fs::create_dir_all(&output_dir)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("Invalid system clock"))?
        .as_nanos();
    let staging = output_dir.join(format!(".foundation-replay-{}-{nonce}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&staging)?;
    let ipc_path = directory()?.join(format!("gsr-{}.sock", std::process::id()));
    let state_dir =
        state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/recording");
    state::private_directory(&state_dir)?;
    let mut args = command(
        &c,
        &display,
        false,
        &staging,
        &state_dir.join("portal-token"),
    )?;
    // Replay output is a directory; the ordinary recording timestamp sidecar
    // is unnecessary because readiness comes from the recorder's frame events.
    let marker = args
        .iter()
        .position(|v| v == "-write-first-frame-ts")
        .ok_or_else(|| invalid("Missing frame timestamp option"))?;
    args.drain(marker..marker + 2);
    let q = args
        .iter()
        .position(|v| v == "-q")
        .ok_or_else(|| invalid("Missing quality option"))?;
    args[q + 1] = c.replay_bitrate_kbps.to_string();
    let verbosity = args
        .iter()
        .position(|v| v == "-v")
        .ok_or_else(|| invalid("Missing verbosity option"))?;
    args[verbosity + 1] = "yes".into();
    args.extend([
        "-bm".into(),
        "cbr".into(),
        "-r".into(),
        c.replay_seconds.to_string(),
        "-replay-storage".into(),
        "ram".into(),
        "-ro".into(),
        staging.to_string_lossy().into_owned(),
        "-ipc".into(),
        ipc_path.to_string_lossy().into_owned(),
    ]);
    let signals = Signals::new()?;
    let old = signals.old;
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // SAFETY: only async-signal-safe mask restoration is done between fork/exec.
    unsafe {
        cmd.pre_exec(move || {
            let e = libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut());
            if e == 0 {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(e))
            }
        });
    }
    let mut child = Capture {
        child: cmd.spawn()?,
        reaped: false,
    };
    let stderr = child
        .child
        .stderr
        .take()
        .ok_or_else(|| invalid("Missing replay diagnostics"))?;
    let (mut event_writer, mut event_reader) = UnixStream::pair()?;
    event_reader.set_nonblocking(true)?;
    let reader = thread::Builder::new()
        .name("replay-events".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut ready = false;
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if line.contains("gsr info: update fps:") {
                    if !ready {
                        let _ = event_writer.write_all(b"R");
                        ready = true;
                    }
                } else {
                    if ready && line.contains("pipewire: negotiated format:") {
                        let _ = event_writer.write_all(b"C");
                    }
                    eprintln!("{line}");
                }
            }
        })?;
    let mut ready = false;
    let mut regular = false;
    let mut pending_save = save_initial;
    let mut pending_regular = false;
    let mut state = json!({"phase":"starting", "replay":true, "replay_status":"starting", "buffer_seconds":c.replay_seconds, "save_status":if save_initial {"saving"} else {"idle"}, "fps": selected_fps(&c, &display), "focused_display":display["name"]});
    publish(&state)?;
    let started = Instant::now();
    let mut buffer_deadline: Option<Instant> = None;
    let mut failure = None;
    let mut desktop_only = false;
    loop {
        if let Some(exit) = child.child.try_wait()? {
            child.reaped = true;
            failure = Some(format!(
                "Replay recorder exited unexpectedly ({exit}); partial files retained in {}",
                staging.display()
            ));
            break;
        }
        let mut events = [0u8; 16];
        let count = match event_reader.read(&mut events) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => 0,
            Err(e) => return Err(e.into()),
        };
        let display_changed = events[..count].contains(&b'C');
        if !ready && events[..count].contains(&b'R') {
            ready = true;
            state["phase"] = "replay".into();
            state["replay_status"] = "buffering".into();
            buffer_deadline =
                Some(Instant::now() + Duration::from_secs(u64::from(c.replay_seconds)));
            publish(&state)?;
            if pending_regular {
                match no_retained_output(&staging, "Video_")
                    .and_then(|()| ipc(&ipc_path, "start-replay-recording", Duration::from_secs(5)))
                {
                    Ok(_) => {
                        regular = true;
                        state["phase"] = "replay-recording".into();
                        publish(&state)?;
                    }
                    Err(e) => notify(&format!("Recording not started: {e}")),
                }
                pending_regular = false;
            }
            if pending_save {
                save_replay(
                    &mut state,
                    &ipc_path,
                    &staging,
                    &output_dir,
                    Duration::from_secs(c.stop_timeout_seconds),
                )?;
                pending_save = false;
            }
        }
        if buffer_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            state["replay_status"] = "ready".into();
            buffer_deadline = None;
            publish(&state)?;
        }
        let request = queued(socket)?;
        let (shutdown, child_changed) = signals.events()?;
        let startup_expired =
            !ready && started.elapsed() >= Duration::from_secs(c.startup_timeout_seconds);
        let desktop_request = request.as_deref() == Some(b"silent") && !regular;
        if shutdown
            || (request.as_deref() == Some(b"stop") && !regular)
            || request.as_deref() == Some(b"replay-toggle")
            || request.as_deref() == Some(b"replay-off")
            || startup_expired
            || display_changed
            || desktop_request
        {
            desktop_only = desktop_request;
            if regular && ready {
                let value = ipc(
                    &ipc_path,
                    "stop-replay-recording",
                    Duration::from_secs(c.stop_timeout_seconds),
                )?;
                let saved = publish_saved(&value, &staging, &output_dir, "Recording")?;
                notify(&format!("Saved {}", saved.display()));
            }
            child.stop()?;
            let deadline = Instant::now()
                + if ready {
                    Duration::from_secs(c.stop_timeout_seconds)
                } else {
                    Duration::from_millis(c.cancel_timeout_milliseconds)
                };
            let exit = loop {
                if let Some(exit) = child.child.try_wait()? {
                    break exit;
                }
                if Instant::now() >= deadline {
                    child.child.kill()?;
                    let exit = child.child.wait()?;
                    child.reaped = true;
                    if ready {
                        return Err(invalid(format!(
                            "Replay recorder did not stop; partial files retained in {}",
                            staging.display()
                        )));
                    }
                    break exit;
                }
                thread::sleep(Duration::from_millis(25));
            };
            child.reaped = true;
            if startup_expired {
                failure = Some("Replay capture timed out before the first frame".into());
            } else if display_changed {
                failure = Some("Display capture format changed; replay stopped safely. Start it again for the new display mode".into());
            }
            if !exit.success() && ready {
                failure = Some(format!(
                    "Replay recorder exited with {exit}; partial files retained in {}",
                    staging.display()
                ));
            }
            break;
        }
        if let Some(request) = request {
            match request.as_slice() {
                b"replay-save" if !ready => {
                    pending_save = true;
                    state["save_status"] = "saving".into();
                    publish(&state)?;
                }
                b"replay-save" => save_replay(
                    &mut state,
                    &ipc_path,
                    &staging,
                    &output_dir,
                    Duration::from_secs(c.stop_timeout_seconds),
                )?,
                b"replay-on" => {}
                b"toggle" if !ready => pending_regular = !pending_regular,
                b"toggle" | b"stop" | b"silent" if regular && ready => {
                    state["phase"] = "stopping".into();
                    publish(&state)?;
                    let result = ipc(
                        &ipc_path,
                        "stop-replay-recording",
                        Duration::from_secs(c.stop_timeout_seconds),
                    )
                    .and_then(|v| publish_saved(&v, &staging, &output_dir, "Recording"));
                    regular = false;
                    state["phase"] = "replay".into();
                    publish(&state)?;
                    match result {
                        Ok(path) => notify(&format!("Saved {}", path.display())),
                        Err(e) => notify(&format!("Recording not saved: {e}")),
                    }
                }
                b"toggle" if ready => {
                    match no_retained_output(&staging, "Video_").and_then(|()| {
                        ipc(&ipc_path, "start-replay-recording", Duration::from_secs(5))
                    }) {
                        Ok(_) => {
                            regular = true;
                            state["phase"] = "replay-recording".into();
                            publish(&state)?;
                        }
                        Err(e) => notify(&format!("Recording not started: {e}")),
                    }
                }
                b"silent" => {}
                _ => {}
            }
        }
        if child_changed {
            continue;
        }
        // Kernel events wake the owner for requests, process exit or recorder readiness.
        let mut fds = [
            libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: signals.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: event_reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let timeout = if !ready {
            Duration::from_secs(c.startup_timeout_seconds)
                .saturating_sub(started.elapsed())
                .as_millis()
                .clamp(1, i32::MAX as u128) as i32
        } else if let Some(deadline) = buffer_deadline {
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .clamp(1, i32::MAX as u128) as i32
        } else {
            -1
        };
        // SAFETY: fds references initialized poll descriptors.
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) } < 0
            && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
        {
            return Err(io::Error::last_os_error().into());
        }
    }
    let _ = reader.join();
    if ipc_path.exists() {
        let _ = fs::remove_file(&ipc_path);
    }
    if let Some(error) = failure {
        return Err(invalid(error));
    }
    // Remove only our empty private staging directory; failed saves remain recoverable.
    let _ = fs::remove_dir(&staging);
    if desktop_only {
        publish(
            &json!({"phase":"starting", "replay":true, "replay_paused":true, "replay_status":"paused", "buffer_seconds":c.replay_seconds}),
        )?;
    } else {
        publish(&json!({"phase":"idle", "replay":false, "replay_status":"off"}))?;
    }
    if desktop_only {
        notify("Replay buffer paused for desktop-only recording; it will restart afterward");
    } else {
        notify("Replay buffer stopped");
    }
    Ok(desktop_only)
}
