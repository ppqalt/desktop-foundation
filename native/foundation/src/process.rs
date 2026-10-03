//! Bounded one-shot subprocesses. No shell evaluation or resident supervisor.
use crate::{Error, Result, invalid};
use std::{
    io::{Read, Write},
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
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
    let (sender, receiver) = mpsc::channel::<(u8, Result<Vec<u8>>)>();
    let mut workers = Vec::new();
    if let Some(mut pipe) = owned.child.stdout.take() {
        let sender = sender.clone();
        workers.push(thread::Builder::new().spawn(move || {
            let result = collect(&mut pipe);
            let _ = sender.send((0, result));
        })?);
    }
    if let Some(mut pipe) = owned.child.stderr.take() {
        let sender = sender.clone();
        workers.push(thread::Builder::new().spawn(move || {
            let result = collect(&mut pipe);
            let _ = sender.send((1, result));
        })?);
    }
    if let Some(bytes) = input {
        let mut pipe = owned
            .child
            .stdin
            .take()
            .ok_or_else(|| invalid("Subprocess stdin unavailable"))?;
        workers.push(thread::Builder::new().spawn(move || {
            let result = pipe
                .write_all(&bytes)
                .map(|_| Vec::new())
                .map_err(Error::from);
            drop(pipe);
            let _ = sender.send((2, result));
        })?);
    }
    let deadline = Instant::now() + timeout;
    let result = (|| {
        let mut finished = 0;
        let mut status = None;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        loop {
            if status.is_none() {
                status = owned.child.try_wait()?;
            }
            if finished == workers.len() {
                if let Some(status) = status {
                    return Ok(Output {
                        stdout,
                        stderr,
                        status,
                    });
                }
                if Instant::now() >= deadline {
                    return Err(Error::Process {
                        command: name.clone(),
                        detail: "timed out waiting for process exit".into(),
                    });
                }
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            if Instant::now() >= deadline {
                return Err(Error::Process {
                    command: name.clone(),
                    detail: format!("timed out after {} ms", timeout.as_millis()),
                });
            }
            match receiver.recv_timeout(Duration::from_millis(5)) {
                Ok((kind, bytes)) => {
                    let bytes = bytes?;
                    match kind {
                        0 => stdout = bytes,
                        1 => stderr = bytes,
                        _ => {}
                    }
                    finished += 1;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(invalid("Subprocess stream worker stopped"));
                }
            }
        }
    })();
    if result.is_err() {
        owned.stop();
    }
    for worker in workers {
        if worker.join().is_err() {
            return Err(invalid("Subprocess stream worker failed"));
        }
    }
    owned.completed = true;
    result
}

fn collect(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(MAX_OUTPUT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err(invalid("Subprocess output exceeded 64 KiB"));
    }
    Ok(bytes)
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
