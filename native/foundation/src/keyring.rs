//! Secret-free greetd/PAM readiness gate for the packaged oo7 user service.
//!
//! The native PAM module still owns authentication-token capture and delivery.
//! This helper neither reads stdin nor connects to oo7's password socket.
use crate::{Result, invalid, process};
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(25);
const FIRST_WAIT: Duration = Duration::from_secs(2);
const SYSTEMCTL_LIMIT: Duration = Duration::from_secs(3);
const SERVICE: &str = "oo7-daemon.service";

struct User {
    name: CString,
    uid: libc::uid_t,
    gid: libc::gid_t,
    home: PathBuf,
}

fn user(name: &OsStr) -> Result<User> {
    let name = CString::new(name.as_bytes()).map_err(|_| invalid("Invalid PAM username"))?;
    let mut size = 16 * 1024;
    loop {
        let mut buffer = vec![0_u8; size];
        let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut found = std::ptr::null_mut();
        // SAFETY: name is terminated, outputs point to writable storage, and
        // NSS strings are copied before the backing buffer is dropped.
        let status = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                record.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ERANGE && size < 1024 * 1024 {
            size *= 2;
            continue;
        }
        if status != 0 || found.is_null() {
            return Err(invalid("Cannot resolve the PAM account through NSS"));
        }
        // SAFETY: successful NSS lookup initializes record and its strings.
        let record = unsafe { record.assume_init() };
        if record.pw_name.is_null() || record.pw_dir.is_null() {
            return Err(invalid("The PAM account has incomplete NSS information"));
        }
        // SAFETY: these terminated strings remain inside the live NSS buffer.
        let (name, home) = unsafe {
            (
                CStr::from_ptr(record.pw_name).to_owned(),
                PathBuf::from(OsStr::from_bytes(CStr::from_ptr(record.pw_dir).to_bytes())),
            )
        };
        if name.to_bytes().is_empty() || !home.is_absolute() {
            return Err(invalid("The PAM account has invalid NSS information"));
        }
        return Ok(User {
            name,
            uid: record.pw_uid,
            gid: record.pw_gid,
            home,
        });
    }
}

fn drop_privileges(user: &User) -> Result<()> {
    // SAFETY: this executable is single-threaded at this point. Supplementary
    // groups and all real/effective/saved IDs are set before starting children
    // or touching user-controlled runtime paths.
    if unsafe {
        libc::initgroups(user.name.as_ptr(), user.gid) != 0
            || libc::prctl(libc::PR_SET_KEEPCAPS, 0, 0, 0, 0) != 0
            || libc::setresgid(user.gid, user.gid, user.gid) != 0
            || libc::setresuid(user.uid, user.uid, user.uid) != 0
    } {
        return Err(invalid(
            "Cannot permanently drop privileges to the PAM account",
        ));
    }
    // Explicitly discard capability sets even if the caller had unusual
    // securebits. Inheritable capabilities otherwise can survive setuid.
    #[repr(C)]
    struct CapHeader {
        version: u32,
        pid: libc::c_int,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CapData {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let header = CapHeader {
        version: 0x2008_0522, // _LINUX_CAPABILITY_VERSION_3
        pid: 0,
    };
    let data = [CapData {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    // SAFETY: capset receives the two Linux v3 capability records for this
    // process. no_new_privs permanently prevents privilege regain via exec.
    if unsafe {
        libc::syscall(libc::SYS_capset, &header, data.as_ptr()) != 0
            || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
            || libc::getuid() != user.uid
            || libc::geteuid() != user.uid
            || libc::getgid() != user.gid
            || libc::getegid() != user.gid
    } {
        return Err(invalid("Cannot discard privileged process credentials"));
    }
    Ok(())
}

fn clean_environment(user: &User, runtime: &Path) -> Result<()> {
    // SAFETY: the dedicated helper has not created threads or children. Drop
    // the complete inherited environment, including display-manager settings,
    // before installing only the user-service connection context.
    if unsafe { libc::clearenv() } != 0 {
        return Err(invalid("Cannot clear the display-manager environment"));
    }
    let name = OsStr::from_bytes(user.name.to_bytes());
    let bus = OsString::from(format!("unix:path={}/bus", runtime.display()));
    // SAFETY: same single-threaded context as clearenv above.
    unsafe {
        std::env::set_var("PATH", "/usr/bin");
        std::env::set_var("HOME", &user.home);
        std::env::set_var("USER", name);
        std::env::set_var("LOGNAME", name);
        std::env::set_var("XDG_RUNTIME_DIR", runtime);
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", bus);
    }
    Ok(())
}

fn runtime_is_safe(runtime: &Path, uid: libc::uid_t, deadline: Instant) -> Result<()> {
    let directory = fs::symlink_metadata(runtime)
        .map_err(|_| invalid("The PAM user runtime directory is unavailable"))?;
    if !directory.is_dir() || directory.uid() != uid || directory.mode() & 0o7777 != 0o700 {
        return Err(invalid(
            "The PAM user runtime directory is not private and owned",
        ));
    }
    let bus = loop {
        match fs::symlink_metadata(runtime.join("bus")) {
            Ok(bus) => break bus,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(invalid("The PAM user systemd bus did not become ready"));
                }
                thread::sleep(POLL.min(remaining));
            }
            Err(_) => return Err(invalid("Cannot inspect the PAM user systemd bus")),
        }
    };
    if bus.mode() & libc::S_IFMT != libc::S_IFSOCK || bus.uid() != uid {
        return Err(invalid("The PAM user bus is not an owned Unix socket"));
    }
    Ok(())
}

fn socket_ready(socket: &Path, uid: libc::uid_t) -> Result<bool> {
    let metadata = match fs::symlink_metadata(socket) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(invalid("Cannot inspect the oo7 PAM listener")),
    };
    if metadata.mode() & libc::S_IFMT != libc::S_IFSOCK
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(invalid(
            "The oo7 PAM listener is not a private owned Unix socket",
        ));
    }
    Ok(true)
}

fn wait(socket: &Path, uid: libc::uid_t, deadline: Instant) -> Result<bool> {
    loop {
        if socket_ready(socket, uid)? {
            return Ok(true);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        thread::sleep(POLL.min(remaining));
    }
}

fn systemctl(action: &str, deadline: Instant) -> Result<bool> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(invalid("oo7 service readiness timed out"));
    }
    let args = [
        "/usr/bin/systemctl",
        "--user",
        "--no-ask-password",
        "--no-pager",
        action,
        SERVICE,
    ]
    .map(str::to_owned);
    // No stdin, output capture or journal access: only lifecycle metadata is
    // needed. The shared process runner kills timed-out child process groups.
    let output = process::run(&args, None, remaining.min(SYSTEMCTL_LIMIT), false)
        .map_err(|_| invalid("Cannot contact the packaged oo7 user service within the deadline"))?;
    Ok(output.status.success())
}

/// Prepare oo7's native password listener for one authenticated greetd login.
/// Must only be called by the dedicated root-owned executable, never by the
/// ordinary multithreaded desktop backend: it changes process credentials.
pub fn ready() -> Result<()> {
    let deadline = Instant::now() + DEADLINE;
    if std::env::args_os().len() != 1 {
        return Err(invalid("The oo7 PAM readiness helper accepts no arguments"));
    }
    if std::env::var_os("PAM_TYPE").as_deref() != Some(OsStr::new("open_session"))
        || std::env::var_os("PAM_SERVICE").as_deref() != Some(OsStr::new("greetd"))
    {
        return Ok(()); // Not an applicable PAM login; never start a service.
    }
    // SAFETY: these simple accessors inspect only our process credentials.
    if unsafe { libc::getuid() != 0 || libc::geteuid() != 0 } {
        return Err(invalid(
            "The oo7 PAM readiness helper must be invoked by greetd PAM",
        ));
    }
    let name = std::env::var_os("PAM_USER").ok_or_else(|| invalid("PAM_USER is missing"))?;
    let user = user(&name)?;
    if user.uid < 1000 {
        return Ok(()); // Includes root and greetd's greeter/system accounts.
    }
    if user.gid == 0 {
        return Err(invalid(
            "A privileged primary group is not supported by the oo7 readiness helper",
        ));
    }
    drop_privileges(&user)?;
    let runtime = PathBuf::from(format!("/run/user/{}", user.uid));
    clean_environment(&user, &runtime)?;
    runtime_is_safe(
        &runtime,
        user.uid,
        (Instant::now() + FIRST_WAIT).min(deadline),
    )?;
    if !systemctl("start", deadline)? {
        return Err(invalid(
            "The packaged oo7 user service could not start; inspect its user-unit status",
        ));
    }
    let socket = runtime.join("oo7-pam.sock");
    if wait(
        &socket,
        user.uid,
        (Instant::now() + FIRST_WAIT).min(deadline),
    )? {
        return Ok(());
    }
    if !systemctl("is-active", deadline)? {
        return Err(invalid(
            "The packaged oo7 user service exited before its PAM listener was ready",
        ));
    }
    // oo7 0.6 removes the socket when a per-connection listener clone drops.
    // Only a live daemon with an absent listener gets this single recovery
    // restart; a healthy daemon is reused. No unsandboxed daemon is spawned.
    // https://github.com/linux-credentials/oo7/commit/2eac1187db54ae9c37d19a6f50a5f59adeac3a53
    if !socket_ready(&socket, user.uid)? && !systemctl("restart", deadline)? {
        return Err(invalid(
            "oo7's missing PAM listener could not be recovered through its user service",
        ));
    }
    if !wait(&socket, user.uid, deadline)? {
        return Err(invalid(
            "oo7's PAM listener did not become ready; automatic keyring unlock may need another login",
        ));
    }
    Ok(())
}
