# Shared Rust command backend

`native/foundation` builds `desktop-foundationctl`. `scripts/foundation` selects
the release binary from the invoking checkout and passes its absolute root.
Build explicitly with `scripts/build-backend`; runtime never builds on demand.
There is no daemon, network listener, new systemd unit or privileged API.

Commands:

- `clipboard [--state DIRECTORY] init|store text|store image|copy ID|delete ID|clear`
- `apps launch [terminal|browser|files|pdf|image|text] [--check]`
- `volume up|down`
- `bluetooth power on|off`
- `bluetooth codecs DEVICE_PATH`
- `power suspend|logout|reboot|poweroff [--check]`
- `actions list|plan ID|invoke ID`
- `cache plan|prune`
- `system packages [--config FILE]`
- `shell call METHOD [ARGS...]`

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

Cache retention runs after successful deliberate theme publication, after releasing
the theme transaction lock, and acquires that same lock itself. It retains six
owned theme revisions, always including current and previous, 32 semantic-palette
caches and 16 overview PNGs. Active palette and legacy backdrop references are
protected regardless of age. Only recognized names are candidates. Foreign
revisions, wallpapers outside bundles, backups, journals and symlinks survive.
Broken pointers, malformed metadata or another checkout's ownership prevent
removal. Planning is read-only; maintenance failure reports a deferred cleanup
without reversing a successful publication. No idle cache process or timer exists.

Fastfetch package information uses the same Rust executable. Its total still
comes from native Fastfetch and its foreign count from pacman. Cached counts keep
the existing signature arrays (path, inode, modification/creation timestamps and
size), repository invalidation and refusal to reuse/publish during a package
transaction. Valid old caches remain readable. Malformed caches are reproducible;
native query errors are reported rather than displayed as zero. No resident
package monitor exists. The installed-directory symlink, copied immutable theme
revision, relative
invocation and relocated-checkout paths are tested. Copied revision helpers pin
the owning checkout binary and use their own revision package configuration.
Stable requests retain the same native subprocess count and
remove Python startup.

Shell IPC uses a shared monotonic three-second readiness budget, with 50 ms
pauses only while unavailable and 500 ms per-probe limits. The requested action
has a three-second deadline and is sent exactly once, including after failure.
No readiness timer runs while idle. This removes repeated shell/sleep process
creation and prevents a stalled status command from blocking a keypress forever.

## Bluetooth follow-up after v0.12-1

The Bluetooth popup's radio switch and earbud Controls' playback-codec discovery
call the shared release binary directly. Existing `bluetooth-power.py` and
`bluetooth-action.py codecs` callers forward to it. Connection, reconnection,
codec changes and event-driven audio routing retain their existing implementation;
Nothing/CMF protocol control remains in its separate Rust backend. QML appearance,
navigation, scroll behavior, battery reporting and actions are preserved.

Radio requests discover every `hciN` adapter, unblock the software radio only on
enable, set its native BlueZ `Powered` property and confirm the observed state.
Transient rejection after unblock is retried within a shared five-second power
budget, including all adapters and their set/read commands. Initial discovery
and optional unblock each have a separate two-second limit, so the entire call
is bounded by approximately nine seconds when enabling (seven when disabling).
Errors retain the popup's JSON contract and a nonzero exit status. No daemon,
polling service, privileged API or Cargo dependency is added.

Codec discovery is read-only. It validates the paired/bonded device, dynamically
matches its address/path to PipeWire-Pulse cards and lists the same available
AAC/LDAC/SBC/SBC XQ playback profiles as before. Device-property reads have
two-second limits; the audio-card snapshot has eight seconds and a 2 MiB stdout
cap to accommodate multi-card machines. Other subprocess output and all stderr
retain the 64 KiB cap. Invalid or oversized service responses fail explicitly.

Finite subprocesses use Linux parent-death protection for their direct children,
so a destroyed popup cannot leave its in-flight native command running. Failed
requests still clean their owned process group. Successful forked clipboard
owners retain their handoff; app/session `exec` paths are unaffected. Child exit
is polled using a native pidfd where supported, removing the former extra reap
delay for fast commands. Older kernels use the existing bounded fallback. These
handles exist only for a requested command; there is no additional idle worker.

### Isolated measurements, 2026-10-04

`python3 scripts/bluetooth-bench.py --runs 24` compares release Rust with the
unchanged v0.12-1 Python sources using only synthetic shell executables. Two
warmups precede 24 paired samples per implementation, alternating order and
including process startup. PATH contains only fixtures; no real radio/audio
commands can run. Median timings on this development host were:

| Helper | Python | Rust |
|---|---:|---:|
| Power off, two adapters | 31.272 ms | 7.126 ms |
| Codec discovery | 57.266 ms | 4.879 ms |

Results include identical JSON/argv behavior checks. These measurements concern
helper overhead, not physical Bluetooth connection latency or whole-desktop RAM
and CPU usage. Frequency/foreground activity was uncontrolled. Live desktop
interaction tests were deliberately omitted at the user's request.

Validation passes 153 Python/integration tests with both opt-in live Qt fixtures
skipped, 38 Rust tests across both crates, warnings-denied Clippy, release build
and `scripts/check` (including static QML/native config validation). Seventeen
new fake-command cases cover radio errors/retries, deadline enforcement, codec
matching/availability, large/malformed responses, killed-helper cleanup and a
clipboard owner surviving the native backend's exit.

Build with `scripts/build-backend`; the normal installer already builds this
same crate. Source changes and the executable must be deployed together. To undo
this follow-up, revert its commit and rebuild the backend before reloading QML.
