# Code quality and performance review — lucky38, 2026-10-03

Starting revision: `bbddde0`, eight commits beyond v0.12. Implementation work is
isolated on `lucky38/rust-quality`; the installed checkout, SDDM, wallpaper,
configuration, services, packages and session remain the baseline. No push or
release tag is part of this pass.

## Review before implementation

The repository has approximately 6,129 lines of Python, 1,499 helper-script lines,
1,385 Rust lines, 3,739 QML lines and 236 Lua lines, plus tests, configuration and
documentation. The inventory includes all tracked source directories, tests,
installer entry points, CI, licenses/provenance and historical validation notes.
Historical Tops acceptance is not evidence of current lucky38 acceptance.

| Subsystem / language | Runtime and resource implications | Ownership, weaknesses and Rust decision |
|---|---|---|
| Niri KDL / Python generator / Lua intent | Generated at deployment/theme change; two Lua subprocesses per render. Native compositor owns effects, layout and Finnish input. | Shared intent and host overrides are separated correctly. Preserve lucky38 profile. Lua execution is occasional and necessary for secondary-backend compatibility; no rewrite justified here. |
| Quickshell QML composition | One shell; lazy launcher/clipboard/Bluetooth/power, resident overview and small hidden volume object. No persistent helper daemon. | Preserve every visual token, animation and layout. No evidence supports deleting effects or moving rendering to Rust. |
| Niri QML adapter | Direct local socket event stream, normalized copies of windows/workspaces, coalesced output reads, bounded action queue. No subprocess per event. | Requests lack a response deadline: a stalled request can wedge the queue. Add failure recovery. Moving this working direct socket into a daemon would duplicate state/IPC without a measured benefit. |
| Hyprland QML/Lua adapter | Native Quickshell subscriptions and relevant coalesced refreshes. Secondary backend. | Native identifiers remain adapter-local. Preserve supported behavior; runtime acceptance needs a separate Hyprland session. |
| Launcher QML/JS | Native desktop-entry model, lazy view, virtualized rows and 32 px icon decode. Search re-ranks when query/model changes. One launch command. | Search text is never evaluated as shell. Native discovery already handles metadata/localization; do not replace it with a less complete Rust scanner. Role launches are a separate worthwhile one-shot Rust candidate. |
| Clipboard Python/SQLite/QML | Two native wl-paste event watchers and wl-clip-persist; Python starts on each selection/action. 100 entries, 8 MiB/item, 32 MiB payloads, 2,048-character previews. | DB lock/transaction and private modes are good. Fixed temporary filename, projection cleanup and error handling need hardening. Migrate storage/actions into typed Rust while keeping schema, hash IDs, exact payloads and JSON contract. |
| Theme Python / Matugen / Pillow | One-shot palette generation, several whole-file hashes and native renderer subprocesses. Cached blur outside UI; complete immutable bundles. | Atomic current/previous pointers and reload rollback matter more than rewrite speed. Revisions/palette/overview caches are unbounded; introduce guarded Rust maintenance. Keep orchestration until crash-recovery migration has its own parity tests. |
| Wallpaper Python → swaybg | Startup helper execs one upstream wallpaper process; no retained interpreter. | Hash/cache generation is occasional. Preserve scaling and blur. Cache publication should be atomic, not a direct final-file write. |
| Notifications Python/shell → Mako | One D-Bus-owned Mako unit, packaged activation alias, owner check. Upstream expiry timers. | Existing provider is respected. No custom notification history exists to migrate; inventing one would change scope/ownership. |
| Bluetooth QML / Python | Native BlueZ and PipeWire state in UI. On-demand busctl/pactl actions, event subscription during audio setup; bounded connection retries. | Avoid moving native models into a duplicate resident backend. Generic codec setup remains Python until hardware parity covers device reconnect/routing/child cleanup. |
| Nothing/CMF Rust | On-demand single-thread Tokio RFCOMM backend; JSON lines, notifications and finite deadlines, exits with surface. | Already Rust. Readback validation is good. Framing resynchronization removes bytes one at a time; input line bound is checked after allocation; runtime gesture unwraps and broad Tokio features deserve repair. Keep AGPL attribution. |
| Volume Python | Starts per keypress, lock, wpctl set/read, Quickshell IPC, notification fallback. No daemon. | Good behavior, but unbounded native set/read calls. Suitable for the shared Rust command backend with bounded subprocesses and identical 3% steps/OSD fallback. |
| Power Python / QML | Fixed commands only, --check routing, no resident helper. | Preserve explicit interaction and session-exit routing; a small typed Rust action registry can serve this existing consumer and future menus. Never execute power operations in tests. |
| Session Bash / Python units | Startup imports environment, starts one target; independent shell/notifications/polkit, clipboard init before native watchers. | Correct lifecycle and restart limits. Do not create a service for the new one-shot backend. Generated units change only on deliberate future deployment. |
| Application roles Python | Launch validates desktop entries on each use; apply/check/restore manage exact MIME keys and separate journal. xdg queries are occasional. | Move frequent launch parsing/validation to Rust; retain complex MIME ownership/recovery until fully ported. Preserve Brave choice and unrelated defaults. Avoid two xdg-mime queries for a failed key. |
| Deploy / preferences Python | Occasional locks, write-ahead symlink/key journals, native config validation, original backups and external-change refusal. | Significant recovery coverage. Preferences restoration cannot resume after partial native writes; repair that bug with tests. No benefit from wholesale rewriting during this pass. |
| Installer / boot / greeter Python/Bash | Explicit maintenance only; package/system writes separated from user session. | SDDM is preserved with --no-greeter. Retain boot/system original journals and reject foreign paths; do not exercise installation or boot changes on lucky38. Privileged journal durability remains separate debt. |
| Spotify Python / Brave Python/JS | User-requested install/seed and one-shot theme refresh; no account data copied. Pinned archives, narrow native-browser adapter. | Preserve original tool/config backups. Streaming hashing and archive-size bounds are future maintenance improvements; not an idle optimization. |
| Fish/Kitty/Fastfetch Python | Native terminal; foreign package count cached against package DB signatures; two pacman-conf queries per invocation. | Cache avoids repeated pacman enumeration. Stable metadata can be consolidated later; native terminal appearance stays unchanged. |
| Doctor Python | Read-only, many bounded one-shot system/package/XDG queries. | Healthy baseline with --core --no-greeter. Report failure accurately; no resident health worker. Batch queries/migrate typed checks later when host coverage exists. |
| Tests / CI / documentation | 107 portable Python tests, 9 Rust tests; Qt adapter opt-in, live harnesses separate. CI omits QML/Niri native capability gates. | Strengthen Rust fmt/Clippy/test gates, include lucky38 in config checks and publish migration/measurement contracts. Keep destructive/live tests opt-in. |

## Complete timer and polling inventory

There is no steady-state foundation polling process. Retained timers have distinct
purposes; a timer used for expiry or a user-requested deadline is not periodic
state polling.

| Location | Frequency / bound | Purpose and decision |
|---|---|---|
| Niri adapter centering | 80 ms one-shot restart on window/workspace events | Debounce, preserve existing single-window behavior. |
| Niri adapter reconnect | 1–30 s exponential one-shot, only while disconnected | Reconnect after actual failure; preserve. |
| ClipboardHistory | 150 ms, at most 20 attempts / 3 s while index missing | Startup race only; subsequent file notifications. Preserve bounded fallback. |
| OverviewBackdrop | 250 ms, at most 20 attempts / 5 s while manifest missing | Startup race only; normal file watcher. Preserve. |
| Volume | 1.5 s one-shot on presentation | Required OSD expiry, preserve. |
| BluetoothPopup | 120 s one-shot after battery receipt; 120 ms exit | Cached battery expiry and animation cleanup; no battery poll. |
| NothingControls | 1.5 s shutdown, 6 s command (24 s fit), 35 s startup | One-shot failure/cleanup deadlines, preserve. |
| Rust earbud backend | 2 s query, 3 s write/ring, 10 s connect, 20 s fit; 120 s battery expiry | Event-driven socket/D-Bus/select; one-shot deadlines. No battery refresh poll. |
| Rust earbud connect | 100/200/400/800/1,600 ms, finite retry | User-requested connection only, preserve. |
| bluetooth-power.py | 150 ms, bounded 5 s retry window (each bus call bounded 2 s) | BlueZ readiness after rfkill; worthwhile future D-Bus Rust migration, not resident polling. |
| bluetooth-action.py | Initial 5 s wait after codec reboot, 2 s retry within 30 s (calls separately bounded) | Device reboot/reconnect behavior; preserve. Audio setup waits for relevant pactl events within 12 s. |
| shell-ipc | 50 ms, at most 60 probes | Early-keypress readiness; no idle activity. Requests run once. |
| spotify launcher | 1 s native client-state wait, user-launched only | First-run patch completion; not a background service. |
| bench/startup/trace/profile/smoke/failure/boot-report and live tests | 10–100 ms readiness probes; finite test deadlines, explicit 1–45 s sample waits | Development diagnostics only. Leave separate from session startup. Some operate the live desktop and must remain opt-in. |
| theme contrast loop, protocol parsing, filesystem inventory loops | No scheduling interval | Finite computation/data traversal; not polling. |
| Mako/Qt/PipeWire/BlueZ/Niri/SDDM upstream | External native event/render/expiry behavior | Do not replace upstream internals or disable effects to improve a counter. |

## Recorded baseline

Three consecutive 10-second live samples, same PIDs. Live user activity is not
controlled; these describe current costs, not a before/after optimization claim.
Context switches are not measured hardware wakeups.

| Process | PSS KiB | RSS KiB |
|---|---:|---:|
| Quickshell | 238,481 | 330,644 |
| Niri | 148,089 | 210,568 |
| Mako | 6,348 | 21,920 |
| swaybg | 3,564–3,567 | 16,440 |
| hyprpolkitagent | 36,152 | 92,656 |
| wl-clip-persist | 3,381 | 5,864 |
| wl-paste text | 203 | 2,292 |
| wl-paste image | 199 | 2,344 |

Seven supporting desktop processes, plus Niri. Quickshell records zero CPU ticks
and 0–0.1 surviving-thread context switches/s; other supporting workers record
zero CPU ticks. Niri records 1.5–1.6% of one core and about 93–94 switches/s under
this live workload. No claim of zero wakeups follows from those counters.

Synthetic clipboard text store, 12 one-shot runs, warm filesystem, same temporary
history: median 43.98 ms and peak RSS 23,484 KiB. Fixtures contain no real clipboard
data. A paired/interleaved comparison will be used after migration.

Storage: tracked file payloads 35,327,386 bytes; original checkout allocated
836,689,920 bytes including 778,653,696 bytes of Rust target output. Runtime state
185,274,368 bytes, theme 37,240,832 bytes (four revisions), cache 806,912 bytes.
Originals/backups remain untouched. The installed Nothing release binary is
4,655,416 bytes. Build output is not the deployed binary budget.

Baseline doctor and `install-core --check --no-greeter` pass. ShellCheck/shfmt were
missing; verified upstream binaries were downloaded into temporary tooling only,
without a package installation. Initial QML lint exits 0 with metadata/property
warnings; formatting and available validators pass. Full results and controlled
after measurements will be appended after implementation.
