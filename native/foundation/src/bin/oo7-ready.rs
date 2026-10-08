//! Root-owned, non-setuid PAM readiness executable. It handles no passwords.

unsafe extern "C" fn deadline_expired(_: libc::c_int) {
    const MESSAGE: &[u8] = b"desktop-foundation oo7: service readiness exceeded 10 seconds\n";
    // SAFETY: write and _exit are async-signal-safe. Fixed text contains no
    // credential or environment contents. Children receive parent-death kill.
    unsafe {
        libc::write(libc::STDERR_FILENO, MESSAGE.as_ptr().cast(), MESSAGE.len());
        libc::_exit(1);
    }
}

fn main() {
    // NSS can block independently of Rust's service deadlines. Bound the whole
    // dedicated process, including account lookup, with a fixed signal alarm.
    // SAFETY: this is a fresh, single-threaded helper with no owned children.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = deadline_expired as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        if libc::sigaction(libc::SIGALRM, &action, std::ptr::null_mut()) != 0 {
            eprintln!("desktop-foundation oo7: cannot establish the service readiness deadline");
            std::process::exit(1);
        }
        let mut signals: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut signals);
        libc::sigaddset(&mut signals, libc::SIGALRM);
        if libc::sigprocmask(libc::SIG_UNBLOCK, &signals, std::ptr::null_mut()) != 0 {
            eprintln!("desktop-foundation oo7: cannot activate the service readiness deadline");
            std::process::exit(1);
        }
        libc::alarm(10);
    }
    let result = desktop_foundationctl::keyring::ready();
    // SAFETY: cancel our alarm before formatting a fixed diagnostic result.
    unsafe { libc::alarm(0) };
    if let Err(error) = result {
        eprintln!("desktop-foundation oo7: {error}");
        std::process::exit(1);
    }
}
