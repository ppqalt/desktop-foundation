fn main() {
    if let Err(error) = desktop_foundationctl::browser_policy::helper_main(
        &std::env::args().skip(1).collect::<Vec<_>>(),
    ) {
        eprintln!("Browser theme policy: {error}");
        std::process::exit(1);
    }
}
