//! Root-owned one-shot helper. No credential handling and no daemon.
unsafe extern "C" fn inspection_deadline(_: libc::c_int) {
    // SAFETY: _exit is async-signal-safe. The unit ignores this pre-start failure
    // and starts conventional greetd using the already published baseline.
    unsafe {
        libc::_exit(1);
    }
}
fn main() {
    // SAFETY: single-threaded initial setup.
    unsafe {
        libc::umask(0o077);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "prepare-login") {
        // Bound optional boot authorization globally, including NSS and file I/O.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = inspection_deadline as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGALRM, &action, std::ptr::null_mut());
            libc::alarm(3);
        }
    }
    if let Err(error) = desktop_foundationctl::authentication::run(std::path::Path::new("/"), &args)
    {
        eprintln!("Desktop Foundation authentication: {error}");
        std::process::exit(1);
    }
}
