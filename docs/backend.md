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
- `bluetooth connect|disconnect|reconnect DEVICE_PATH`
- `bluetooth codec DEVICE_PATH --codec sbc|sbc_xq`
- `screenshot [--backend niri|hyprland] region|window|output`
- `power suspend|logout|reboot|poweroff [--check]`
- `actions list|plan ID|invoke ID`
- `cache plan|prune`
- `theme generate IMAGE`
- `theme derive --source SOURCE --hash HASH` (material JSON on stdin)
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
Screenshot fixtures likewise avoid the real compositor and screen contents.

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
`bluetooth-action.py` callers forward to it. Connection, reconnection, explicit
SBC/SBC XQ selection and event-driven playback routing now also use Rust, as described
below. Nothing/CMF protocol control remains in its separate Rust backend. QML appearance,
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

## Screenshot follow-up

`screenshot [--backend niri|hyprland] region|window|output` replaces the Python
capture driver and both Python backend implementations. The existing Bash
entry point executes the shared binary; the Python entry point only forwards
older callers. QML actions and keybindings keep the same native behavior.
Niri remains clipboard-only, with native window/output IPC and memory-only
region captures. Hyprland still saves a private complete PNG and copies it;
filename collisions cannot overwrite earlier images. See [screenshots.md](screenshots.md).

The only new direct Cargo dependency is the TOML parser for the existing selector
and storage configuration. Selection has no deadline and waits in native poll;
captures and clipboard handoffs retain finite deadlines. Text/stderr caps stay
64 KiB. PNG stdout and saved temporary files have a separate 128 MiB byte cap
to bound failed capture output; this is a helper guard, not a resolution/quality
setting or a native Grim/Niri limit. No screenshot helper runs between requests.

`python3 scripts/screenshot-bench.py --runs 24` compares the previous commit
`1394382` with release Rust using a synthetic 196,992-byte PNG and private
slurp/grim/wl-copy/hyprctl fixtures. Two warmups precede 24 paired samples per
implementation in alternating order, including process startup, transfer and
payload checks. On 2026-10-04:

| Helper | Python | Rust |
|---|---:|---:|
| Niri region | 58.775 ms | 7.054 ms |
| Hyprland output | 60.079 ms | 7.559 ms |

These are helper-path measurements; they exclude user selection time and real
screen capture/Wayland latency. Background activity and CPU frequency were
uncontrolled. Peak RSS was unavailable because `/usr/bin/time` was not installed.
The release binary is 1,576,048 bytes (previously 1,262,136 bytes).

Twenty-eight screenshot CLI tests use only private fixtures, including wrapper
compatibility, cancellation, exact PNG transfer, geometry/clipping, native
actions, locks, output limits, special-file rejection, atomic saves and killed
selector cleanup. Live desktop tests remain deliberately omitted.

Validation: 177 Python/integration tests pass, with two live Qt fixtures skipped;
40 Rust tests across the two crates, warnings-denied Clippy, release build and
`scripts/check` pass. An existing fixture's `/proc` observation race was corrected
to recognize a child disappearing during its status-file read.

## Minimal corner clock

`scripts/clock` uses the existing Rust `shell call toggleClock` path. Readiness
is bounded and the toggle is sent exactly once; there is no Python intermediary
or separate clock service. `clockStatus` is available through shell IPC for
read-only visibility/lifecycle inspection.

The QML composition root lazy-loads `surfaces/Clock.qml` only while enabled.
Native `SystemClock.Minutes` drives the 24-hour display; hiding unloads both
surface and timer. The card uses the shared generated graphite palette and
monospace font, takes neither focus nor input/desktop space, and follows the
focused output. It starts hidden after shell/login startup. Super+Shift+C toggles
it in either compositor; Super+C retains manual centering. No new Cargo or
installer dependency is required.

Validation uses static QML/native config checks and the existing fake shell IPC
wrapper test, extended to cover the new clock command. Ten isolated session/IPC
tests and all 40 Rust tests pass; no clock was opened or tested on the desktop.


## Bluetooth connection and playback follow-up

The popup calls the shared release executable directly for connect/disconnect and
reconnection after a Nothing codec restart. SBC/SBC XQ selection does the same; device
firmware controls remain in `native/nothing`. `scripts/bluetooth-action.py` is now
an exec-only compatibility entry point, including the existing `codecs` command.
No QML appearance, navigation, battery reporting, shortcut or native microphone
policy changes are included.

`bluetooth/actions.rs` checks pairing/bonding, power and blocked state, invokes
native BlueZ methods, and verifies the final Connected property. Friendly JSON
errors preserve the existing UI contract. Invalid paths/codec choices fail before
native commands. Connect retains a 40-second native/45-second outer limit;
disconnect retains 15/20 seconds. Property requests have eight seconds. Firmware
restart reconnection retains its five-second initial pause, followed by a shared
30-second budget covering attempts, verification and two-second retry pauses.
These are operation limits, not a resident retry worker.

`bluetooth/audio.rs` starts its scoped `pactl subscribe` child before snapshots.
Only complete card/sink/server events trigger another snapshot; its own client
queries and source/stream events cannot create a polling loop. One 12-second
budget includes commands, snapshots and waiting; each command is additionally
limited to eight seconds. Available playback profiles retain LDAC → AAC → other
advertised priority. Address/path matching uses reported state, and only the
default playback sink is changed. Selected-profile/codec confirmation rejects
stale sinks. Explicit SBC/SBC XQ cannot report success against another codec
sink when a profile change fails; an already-confirmed matching A2DP sink can succeed. SBC XQ is offered in the
Quality page only when native profile discovery advertises it; it changes this
computer's playback profile without rebooting the earbuds. The CLI accepts
`sbc_xq` and the equivalent `sbc-xq` spelling, with matching reported codec aliases.
Connection remains successful with an audio warning if optional routing fails.

The process layer supplies a finite event subscription guard. Native events wait
in `poll`, with bounded chunks/64 KiB unfinished lines and no timer-driven query.
Snapshots allow 2 MiB stdout while stderr/ordinary command output retain 64 KiB.
Bluetooth subprocesses alone use the C locale. The guard stops/reaps its child
and owned group on return/error, and direct children retain Linux parent-death
protection. There are no new crates, services, listeners or idle helpers.

Build with `scripts/build-backend`; installer deployment already builds the same
crate. Publish the compatible executable before the watched QML source update.
Rollback requires the previous source commit plus its release binary, or a rebuild
of that source. No Bluetooth service, compositor or user session restart is needed.

Validation and measurements use only isolated command fixtures; live desktop and
physical Bluetooth/audio tests are intentionally omitted at the user's request.


### Isolated measurements, 2026-10-05

`python3 scripts/bluetooth-actions-bench.py --runs 24` compares the unchanged
`72f6d8d` action helper with release Rust. PATH contains only synthetic shell
executables and all state/endpoints are private. Two warmups precede 24 paired,
alternating samples per implementation, including startup. Exact JSON and native
mutation argv parity are asserted, and every fixture subscription must stop.

| Helper path | Python | Rust |
|---|---:|---:|
| Non-audio connect | 76.437 ms | 10.670 ms |
| Disconnect | 62.825 ms | 4.988 ms |
| Connect with immediately available A2DP | 87.626 ms | 15.412 ms |

These are helper-path measurements, not radio/device latency. User/audio waits are
excluded; foreground activity/frequency were uncontrolled. Peak RSS was
unavailable because `/usr/bin/time` was not installed. Reconnect's intentional
five-second restart pause is preserved and not benchmarked.

Final validation passes 202 Python/integration tests with both opt-in live Qt
fixtures skipped (204 discovered), all 50 Rust tests across both crates,
warnings-denied Clippy, release build and repository static/native config checks.
Thirty-one action CLI fixtures replace the old helper-internal tests; additional
pure Rust tests cover profile/error rules and the finite subscription owner.
The release executable is 1,651,168 bytes, up 75,120 bytes from the previous
screenshot/clock build. No Cargo dependencies were added. Physical desktop,
Bluetooth and audio acceptance was not run.

## Native wallpaper palette generation

The shared Rust backend now owns the finite Matugen request, streamed wallpaper
hashing, graphite semantic mapping, contrast validation and atomic palette cache.
`theme generate IMAGE` returns `{palette, cached}`; `theme derive` accepts the
primary/secondary material colors on stdin for standalone mapping and diagnostics.
Generation does not publish a theme, reload applications or change wallpaper.

`scripts/theme_pipeline.py` delegates generation and derivation to the built
release executable. Its file/state helpers and derive/generate interfaces remain
available to existing callers.
The Python transaction still selects the active wallpaper, holds the existing
lock, stages image/adapter outputs, validates them, publishes current/previous
and performs reload or rollback. Rust does not acquire that transaction lock
again. QML continues rendering the same semantic roles.

The `graphite-v1` policy, SHA-256 cache names and valid existing palettes remain
compatible. Color conversion preserves Python HLS operation order and
ties-to-even channel rounding. The packaged Matugen process uses an isolated
empty config and dry-run dark mode, with its existing 60-second deadline and
bounded JSON output. Wallpaper hashing streams without a wallpaper-size cap.
File identity, size and nanosecond modification/change stamps are checked around
hashing and Matugen; a source changed during generation cannot publish its result
under the old hash. This is a request-time check, without a resident watcher.
Malformed palette data, failed Matugen and cache publication failures report
errors before runtime publication; the last-known-good active bundle is retained.
No crate, daemon, listener or idle theme process is added.

Build with `scripts/build-backend`. For rollback, restore the previous source
and release binary together, or rebuild the previous source. Deployment uses
the compatible binary before updating its callers; no live theme is applied
merely by upgrading these files.

### Isolated validation and measurements, 2026-10-06

All **221 Python/integration checks** passed (223 discovered; two opt-in desktop
checks skipped), alongside **53 Rust tests**, Clippy, formatting and native config
validation. Frozen baseline palettes cover exact colors and metadata; a wider
audit matched 146 complete palettes and rejected the same 16 contrast failures.
Private fixtures cover both Matugen layouts, old caches, malformed/bounded data,
source replacement, unusual filenames, failed generation and existing rollback.

Packaged Matugen 4.2.0 also ran on private copies of the Windows/Tux wallpaper
and a synthetic orange image. Both results exactly matched the former generator.
Blue retained background `#1f262f` / accent `#98ccf9`; orange produced background
`#252429` / accent `#ffb59a`. These trials generated palettes only; no active
wallpaper, desktop surface or application was tested or reloaded.

`scripts/theme-generation-bench.py --runs 24` compares the `1ca553c` Python
palette helper with release Rust, using private state, synthetic Matugen and a
1,092,275-byte source. Two warmups precede 24 alternating paired measurements;
startup is included, outputs match exactly, and Rust's wait4 parent measures RSS.

| Palette helper | Python median | Rust median | Python/Rust peak RSS |
|---|---:|---:|---:|
| Derive | 38.777 ms | 0.870 ms | 19,690 / 4,048 KiB |
| Generate, cache miss | 42.741 ms | 2.711 ms | 19,922 / 4,236 KiB |
| Generate, cache hit | 40.795 ms | 1.599 ms | 19,998 / 4,134 KiB |

These measure palette helper processes with warm filesystem cache. They exclude
real Matugen extraction, the Python transaction, image blur and desktop reloads;
they are not end-to-end wallpaper-set timings. The private packaged-generator
trial took 236.942 ms cold / 3.863 ms cached for Windows/Tux and 12.921 / 1.064 ms
for the small solid-orange image (single samples, not comparative benchmarks).
The release executable is 1,718,992 bytes, up 67,824 bytes. No Cargo dependencies
or resident generation processes were added.
