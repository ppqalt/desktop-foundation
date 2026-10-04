//! Bounded one-shot subprocesses. No shell evaluation or resident supervisor.
use crate::{Error, Result, invalid};
#[cfg(target_os = "linux")]
use std::os::fd::{FromRawFd, OwnedFd};
use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_OUTPUT: u64 = 64 * 1024;
const MAX_IMAGE: u64 = 128 * 1024 * 1024;

/// Hand ownership to a long-lived app or session command; no supervisor remains.
pub fn replace(args: &[String]) -> Result<()> {
    let name = args.first().ok_or_else(|| invalid("Empty command"))?;
    let error = Command::new(name).args(&args[1..]).exec();
    Err(Error::Process {
        command: name.clone(),
        detail: error.to_string(),
    })
}

#[derive(Debug)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: ExitStatus,
}

struct OwnedChild {
    child: Child,
    completed: bool,
}
impl OwnedChild {
    fn stop(&mut self) {
        // Command creates this child's own process group. Kill only that group
        // on failed/expired operations, including children retaining pipe FDs.
        let pid = self.child.id() as libc::pid_t;
        // SAFETY: a positive child PID identifies our newly created group.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.completed {
            self.stop();
        }
    }
}

pub fn run(
    args: &[String],
    input: Option<Vec<u8>>,
    timeout: Duration,
    capture: bool,
) -> Result<Output> {
    run_with_limit(args, input, Some(timeout), capture, MAX_OUTPUT)
}

fn run_with_limit(
    args: &[String],
    input: Option<Vec<u8>>,
    timeout: Option<Duration>,
    capture: bool,
    stdout_limit: u64,
) -> Result<Output> {
    let name = args
        .first()
        .ok_or_else(|| invalid("Empty subprocess command"))?;
    let mut command = Command::new(name);
    command
        .args(&args[1..])
        .process_group(0)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(if capture {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(if capture {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    #[cfg(target_os = "linux")]
    {
        let parent = std::process::id() as libc::pid_t;
        // SAFETY: this hook only uses async-signal-safe libc calls and constructs
        // a raw-errno error. It runs after fork, before exec, in our child.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                }
                Ok(())
            });
        }
    }
    let mut owned = OwnedChild {
        child: command.spawn().map_err(|e| Error::Process {
            command: name.clone(),
            detail: e.to_string(),
        })?,
        completed: false,
    };
    // Poll child exit alongside its pipes instead of adding a 5 ms reap delay
    // when a fast native command closes its streams just before exiting.
    #[cfg(target_os = "linux")]
    let child_exit = {
        // SAFETY: pidfd_open only opens a handle to our newly spawned child.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, owned.child.id(), 0) };
        if fd >= 0 {
            // SAFETY: a successful pidfd_open returns a fresh owned descriptor.
            Some(unsafe { OwnedFd::from_raw_fd(fd as libc::c_int) })
        } else {
            None // Older kernels retain the bounded portable fallback below.
        }
    };
    let mut stdout_pipe = owned.child.stdout.take();
    let mut stderr_pipe = owned.child.stderr.take();
    let mut stdin_pipe = owned.child.stdin.take();
    for pipe in [
        stdout_pipe.as_ref().map(AsRawFd::as_raw_fd),
        stderr_pipe.as_ref().map(AsRawFd::as_raw_fd),
        stdin_pipe.as_ref().map(AsRawFd::as_raw_fd),
    ]
    .into_iter()
    .flatten()
    {
        nonblocking(pipe)?;
    }
    let input = input.unwrap_or_default();
    let mut written = 0;
    if input.is_empty() {
        stdin_pipe = None;
    }
    let deadline = timeout.map(|duration| Instant::now() + duration);
    let result = (|| {
        let mut status = None;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        loop {
            collect(&mut stdout_pipe, &mut stdout, stdout_limit)?;
            collect(&mut stderr_pipe, &mut stderr, MAX_OUTPUT)?;
            if let Some(pipe) = stdin_pipe.as_mut() {
                match pipe.write(&input[written..]) {
                    Ok(0) => {
                        return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into());
                    }
                    Ok(n) => written += n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.into()),
                }
                if written == input.len() {
                    stdin_pipe = None;
                }
            }
            if status.is_none() {
                status = owned.child.try_wait()?;
            }
            if stdout_pipe.is_none()
                && stderr_pipe.is_none()
                && stdin_pipe.is_none()
                && let Some(status) = status
            {
                return Ok(Output {
                    stdout,
                    stderr,
                    status,
                });
            }
            if let Some(deadline) = deadline
                && Instant::now() >= deadline
            {
                return Err(Error::Process {
                    command: name.clone(),
                    detail: format!("timed out after {} ms", timeout.unwrap().as_millis()),
                });
            }
            // Every pipe stays owned by this loop. A detached descendant may
            // retain its peer, but cannot hold a blocking stream worker alive
            // after our deadline. Successful ownership handoffs remain intact.
            let mut pipes: Vec<_> = [
                stdout_pipe.as_ref().map(|p| (p.as_raw_fd(), libc::POLLIN)),
                stderr_pipe.as_ref().map(|p| (p.as_raw_fd(), libc::POLLIN)),
                stdin_pipe.as_ref().map(|p| (p.as_raw_fd(), libc::POLLOUT)),
            ]
            .into_iter()
            .flatten()
            .map(|(fd, events)| libc::pollfd {
                fd,
                events,
                revents: 0,
            })
            .collect();
            #[cfg(target_os = "linux")]
            if status.is_none()
                && let Some(fd) = &child_exit
            {
                pipes.push(libc::pollfd {
                    fd: fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
            let remaining = deadline.map(|end| end.saturating_duration_since(Instant::now()));
            if pipes.is_empty() {
                // Only used on kernels without pidfd support. A selector with
                // live output pipes blocks in poll below while awaiting input.
                thread::sleep(
                    remaining
                        .unwrap_or(Duration::from_millis(5))
                        .min(Duration::from_millis(5)),
                );
            } else {
                // Interactive tools wait for the user, without a deadline or
                // periodic wakeups. Their output and child ownership stay bounded.
                let wait_ms = remaining.map_or(-1, |duration| {
                    duration.as_millis().clamp(1, 50) as libc::c_int
                });
                // SAFETY: every descriptor belongs to a live pipe above; the
                // vector remains valid throughout this poll call.
                if unsafe { libc::poll(pipes.as_mut_ptr(), pipes.len() as libc::nfds_t, wait_ms) }
                    < 0
                {
                    let error = std::io::Error::last_os_error();
                    if error.kind() != std::io::ErrorKind::Interrupted {
                        return Err(error.into());
                    }
                }
            }
        }
    })();
    if result.is_err() {
        owned.stop();
    }
    owned.completed = true;
    result
}

fn nonblocking(fd: libc::c_int) -> Result<()> {
    // SAFETY: fd belongs to a live child pipe; neither fcntl operation takes a
    // pointer or transfers descriptor ownership.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn collect(reader: &mut Option<impl Read>, bytes: &mut Vec<u8>, limit: u64) -> Result<()> {
    let mut buffer = [0; 8192];
    while let Some(pipe) = reader.as_mut() {
        let available = ((limit + 1) as usize - bytes.len()).min(buffer.len());
        match pipe.read(&mut buffer[..available]) {
            Ok(0) => *reader = None,
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() as u64 > limit {
                    return Err(invalid(format!(
                        "Subprocess output exceeded {} KiB",
                        limit / 1024
                    )));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub fn checked(
    args: &[String],
    input: Option<Vec<u8>>,
    timeout: Duration,
    capture: bool,
) -> Result<Output> {
    let output = run(args, input, timeout, capture)?;
    ensure_success(args, output)
}

/// Larger finite snapshots can opt in without loosening other commands' limits.
/// stderr keeps its 64 KiB cap and deadlines/child cleanup remain identical.
pub fn checked_capture(args: &[String], timeout: Duration, stdout_limit: u64) -> Result<Output> {
    if !(1..=16 * 1024 * 1024).contains(&stdout_limit) {
        return Err(invalid(
            "Captured stdout limit must be between 1 byte and 16 MiB",
        ));
    }
    ensure_success(
        args,
        run_with_limit(args, None, Some(timeout), true, stdout_limit)?,
    )
}

/// User-driven selectors have no wall-clock deadline. Keep both output caps,
/// empty/explicit stdin and the same cancellation/child cleanup as timed tools.
/// Return the exit status so callers can distinguish cancellation from failure.
pub fn interactive(args: &[String], input: Option<Vec<u8>>) -> Result<Output> {
    run_with_limit(args, input, None, true, MAX_OUTPUT)
}

/// Binary screenshot payloads can exceed the text-command limit. This opt-in
/// keeps a finite 128 MiB cap, 64 KiB stderr, and the caller's capture deadline.
pub fn checked_image(args: &[String], timeout: Duration) -> Result<Output> {
    ensure_success(
        args,
        run_with_limit(args, None, Some(timeout), true, MAX_IMAGE)?,
    )
}

fn ensure_success(args: &[String], output: Output) -> Result<Output> {
    if !output.status.success() {
        return Err(Error::Process {
            command: args[0].clone(),
            detail: if output.stderr.is_empty() {
                format!("exited with {}", output.status)
            } else {
                String::from_utf8_lossy(&output.stderr).trim().to_owned()
            },
        });
    }
    Ok(output)
}
