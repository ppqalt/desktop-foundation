//! Optional benchmark driver. Small parent avoids counting Python's pre-exec
//! memory in a short-lived child's peak RSS. Never part of runtime installation.
use std::{
    fs::File,
    io,
    os::unix::process::ExitStatusExt,
    process::{Command, ExitStatus, Stdio},
    time::Instant,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = args.next().ok_or("Expected input file and command")?;
    let command = args.next().ok_or("Expected command")?;
    let source = File::open(input)?;
    let start = Instant::now();
    let child = Command::new(command)
        .args(args)
        .stdin(source)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let pid = child.id() as libc::pid_t;
    let mut status = 0;
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    loop {
        // SAFETY: wait only for the owned child; both output pointers are valid.
        let result = unsafe { libc::wait4(pid, &mut status, 0, usage.as_mut_ptr()) };
        if result == pid {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
    // SAFETY: successful wait4 initialized rusage. Child is already reaped.
    let usage = unsafe { usage.assume_init() };
    let status = ExitStatus::from_raw(status);
    if !status.success() {
        return Err(io::Error::other(format!("Benchmark child {status}")).into());
    }
    let cpu_ms = (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) as f64 * 1000.0
        + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as f64 / 1000.0;
    println!(
        "{{\"elapsed_ms\":{},\"cpu_ms\":{},\"peak_rss_kib\":{}}}",
        start.elapsed().as_secs_f64() * 1000.0,
        cpu_ms,
        usage.ru_maxrss
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Benchmark: {error}");
        std::process::exit(1);
    }
}
