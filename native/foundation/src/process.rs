//! Bounded one-shot subprocesses. No shell evaluation or resident supervisor.
use crate::{Error, Result, invalid};
use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_OUTPUT: u64 = 64 * 1024;

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
    let mut owned = OwnedChild {
        child: command.spawn().map_err(|e| Error::Process {
            command: name.clone(),
            detail: e.to_string(),
        })?,
        completed: false,
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
    let deadline = Instant::now() + timeout;
    let result = (|| {
        let mut status = None;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        loop {
            collect(&mut stdout_pipe, &mut stdout)?;
            collect(&mut stderr_pipe, &mut stderr)?;
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
            if Instant::now() >= deadline {
                return Err(Error::Process {
                    command: name.clone(),
                    detail: format!("timed out after {} ms", timeout.as_millis()),
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if pipes.is_empty() {
                thread::sleep(remaining.min(Duration::from_millis(5)));
            } else {
                let wait_ms = remaining.as_millis().clamp(1, 50) as libc::c_int;
                // SAFETY: every descriptor belongs to a live pipe above; the
                // vector remains valid for this finite poll call.
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

fn collect(reader: &mut Option<impl Read>, bytes: &mut Vec<u8>) -> Result<()> {
    let mut buffer = [0; 8192];
    while let Some(pipe) = reader.as_mut() {
        let available = ((MAX_OUTPUT + 1) as usize - bytes.len()).min(buffer.len());
        match pipe.read(&mut buffer[..available]) {
            Ok(0) => *reader = None,
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() as u64 > MAX_OUTPUT {
                    return Err(invalid("Subprocess output exceeded 64 KiB"));
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
