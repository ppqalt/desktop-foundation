# Native backend boundary

No Rust crate or daemon is needed yet. Add one only for a concrete requirement that Quickshell/native services cannot meet well. Stable Rust, rustfmt, Clippy and rust-analyzer are prepared; the toolchain file applies when a crate is added. Prefer structured errors, minimal dependencies and event subscriptions. Async and Serde require an actual use case.
