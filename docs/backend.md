# Shared Rust command backend

`native/foundation` builds `desktop-foundationctl`. `scripts/foundation` selects
the release binary from the invoking checkout and passes its absolute root.
Build explicitly with `scripts/build-backend`; runtime never builds on demand.
There is no daemon, network listener, new systemd unit or privileged API.

Commands:

- `clipboard [--state DIRECTORY] init|store text|store image|copy ID|delete ID|clear`
- `apps launch [terminal|browser|files|pdf|image|text] [--check]`
- `volume up|down`
- `power suspend|logout|reboot|poweroff [--check]`
- `actions list|plan ID|invoke ID`

Arguments are passed as an argv array, without shell evaluation. JSON output is
for finite requests, and errors go to stderr with a nonzero status. Clipboard
consumers retain the established watched index format. App launches and power
commands replace the helper process; no supervisor remains. Finite native
queries have bounded input/output and deadlines and clean up only their own
child process group on failure. Successful clipboard ownership processes survive.

Application roles are typed and selected using the existing installation journal.
Desktop entries are validated against the configured executable, including local
user overrides. Existing compositor-aware `scripts/launch` and `session-exit`
remain responsible for Niri/UWSM routing. MIME apply/check/restore retain their
separate, tested Python ownership journal. `scripts/applications launch` uses Rust
directly; maintenance uses the Python tool.

The initial action registry contains nine existing application, volume and power
actions. It supplies stable IDs, labels, descriptions, icons, categories, keywords,
availability diagnostics and invocation policy to future consumers. Listing and
planning actions never execute them. Availability checks local prerequisites;
logind/polkit still decide whether a power operation is allowed. The current
menu's immediate execution behavior and rendering are unchanged. New consumers
must deliberately handle session-changing actions; metadata is not a security gate.

Volume preserves 3% steps, the 100% cap, ties-to-even readout rounding, native
Quickshell OSD and Mako fallback. Failures reading or changing the sink are
reported, and never display stale volume. Old Python volume/clipboard entry
points only exec the new backend, for compatibility with existing callers.

Dependencies stay small and shared. SQLite is dynamically linked from the system;
the crate does not bundle SQLite or a GUI/async runtime. Release builds use thin
LTO and remove debug information while preserving normal panic unwinding.

Tests use temporary databases, synthetic desktop entries and fake executables.
They never change the real clipboard, volume, power state or application defaults.
