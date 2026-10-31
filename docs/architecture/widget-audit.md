# Widget implementation audit

Inspection baseline: `d45e7cc`, before the shared-lifecycle refactor in this batch.
This inventory describes source, not live hardware acceptance. Reference-machine
local edits (including Transparency gain limits) are retained separately.

## Comparison before refactoring

| Surface/module | Category and authority | Lifecycle, focus and input | Data/resources | Inconsistency or intentional boundary |
| --- | --- | --- | --- | --- |
| Launcher | Search popup; DesktopEntries catalogue, Rust launch wrapper | Local closing flags + SequentialAnimation; entrance starts with callLater; search first | Lazy catalogue projection, presentation search | Entrance differs from Power; duplicate teardown timer |
| Clipboard + confirmation | Search popup; Rust SQLite/MIME/pin owner | Same local flags/timer; search/confirmation focus; busy copy retained | FileView index watch; async thumbnails; init only on missing index | Entrance/close duplication; preserve recent fast path and pin/selection semantics |
| Bluetooth device list | Interactive popup; BlueZ/Quickshell readback, Rust mutations/discovery | Local close Timer, callLater entrance, first device selected, radio/list/settings order | Lazy discovery + bounded battery requests; child controls own shutdown | Third teardown implementation; cancellation/retention is device-specific |
| Nothing controls | Nested interactive device view; separate Rust RFCOMM process and PipeWire codec commands | Back/Escape stops owned transport; coherent initial snapshot | JSON line events, timeouts, bounded kill fallback | Specialized backend stop handshake must remain, not replaced by generic destruction |
| Session/Power | Action popup; Rust power actions | First frame starts entrance; local SequentialAnimation; busy action blocks dismissal | One finite action process | Different entrance from other popups; arrow/Tab navigation wraps while other lists clamp |
| Control Center shell | Interactive non-modal sidebar; Rust snapshots/actions | SidebarMotion; OnDemand focus; back before close; retain queued actions | Separate audio/device queues; visible-only two-second snapshots | Intentional sidebar timing; custom `closed` signal conflicts with inherited Window signal |
| CC output/input | Embedded controls; Rust/PipeWire state | Shared choice/mute/slider; immediate values | Coalesced volume writes, concurrent device/audio queues | An older snapshot can arrive after a newer action; needs request revisions |
| CC network/Wi-Fi/connections | Embedded controls; Rust network integration | Shared action/choice, back-focus bookmark | Same visible snapshot loop | Polling is bounded to open view but remains a future event-driven candidate |
| CC wallpaper selector | Embedded grid; Rust catalogue/cache/theme transaction | Three columns, spatial keys, checked tile | Lazy thumbnails and bounded apply process | Owned apply must finish before disposal; preserve current wallpaper on updates |
| CC Cachy Update | Launch action; existing native handoff | Shared full-width action | No polling/installation work until activation | External updater is not a Foundation modal or a df-update operation |
| Volume | Transient passive readout; Rust volume backend | No focus; immediate value; first frame + 100ms fades | One 1500ms expiry; zero interpolation of volume | Intentional readout timing; shares notice surface with Transparency |
| Transparency notice | Transient passive readout; Rust socket-activated transaction | Same Volume surface; hearing/status icons | No QML audio logic; owned active-mode subscriber and crash recovery | Preserve Insert, application volume ownership and exact restoration; no live audio testing |
| Clock | Persistent passive readout, presentation-only | No input/focus; toggle loader owns lifetime | Minute precision clock, destroyed when hidden | No Rust backend needed for formatting a clock |
| Shared modal backdrop | Passive popup-session component | No input; stays mapped across handoffs | One shared compositor blur window | Must not become part of each popup's disposable owner |
| Overview backdrop | Persistent passive component | Per-output background layer | File watcher + bounded startup retry | Intentional per-screen background, no modal Escape behavior |
| Right Kitty/btop monitor | External interactive sidebar; Rust lifecycle, Kitty/btop rendering | Native reversible motion, OnDemand explicit focus | Owned terminal, saved btop settings; graceful retirement | External renderer keeps native machinery; not a QML popup to rewrite |
| Notifications | External passive provider, Mako D-Bus owner | Provider controls interaction and timeout | Event-driven notification service, generated theme | Shares visual roles/font, not QML lifecycle; do not replace a provider already owning D-Bus |
| Probe/diagnostics | Explicit developer surface | Diagnostic loader lifetime; focus not explicitly passive | Backend state readout only | Should explicitly request no keyboard focus; not part of normal desktop |

## Shared mechanisms already present

`SurfaceRouter` arbitrates modal intent and retained owners. `ModalBackdrop` owns
blur continuity. `SurfaceCard`/`SurfaceOutline`, `ControlGroup`, `ControlAction`,
`ControlSwitch`, `ControlChoice`, `ControlVolume`, rows, selection wheel and
Material tokens already cover presentation. `SidebarMotion` and native monitor
motion cover the sidebar category. Rust `process`, `state` and domain modules own
bounded work, filesystem ownership and recovery. Keep these owners; do not add
another shell-wide state machine or protocol.

## First migration and remaining boundaries

This batch migrates all four centered popups to `PopupWindow` (actual window,
input ownership, scrim and focus setup), `PopupCard` (actual entrance/exit motion)
and `PopupLifecycle` (first-frame presentation, retargetable reopen, close timing
and disposal readiness). Their former controllers and animations are removed.
`ControlRequest` owns finite request parsing, phase, errors and completion for
Control Center, Bluetooth actions/discovery, Clipboard mutations and Power. `WidgetStatus` presents
loading, empty and error messages. Shared `Material` functions own list bounds,
activation filtering, focus restoration and popup bounds.
Search focus, confirmation state, session-action
safety and Nothing shutdown stay with their actual owners. Control Center keeps
SidebarMotion; readouts keep their passive timing. A shared popup bounds helper
centralizes the existing output insets without merging layouts.

The batch also gives ordinary lists clamped navigation, guards Control Center
snapshot application with request revisions, removes its signal collision, and
makes passive diagnostic focus explicit. The authoritative category rules live in
[the Widget Standard](../FOUNDATION_UI_STANDARD.md#widget-standard).

Future event-driven CC subscriptions, moving more Bluetooth projection logic into
Rust, and provider-level notification refinements remain **unimplemented**, not
features. No full-desktop compliance or physical latency result is implied by
this audit. Offscreen tests validate lifecycle/focus/transport contracts; Niri
visual blur and restoration to an external application remain compositor behavior.

## Reference review

The [Google reference catalogue](../MATERIAL_REFERENCES.md) links the applicable
motion, focus/accessibility, menu, dialog, switch, slider, typography/color and
snackbar specifications. The Material site returned JavaScript-only shells during
this audit; its contents are not claimed as inspected. Google's
[Compose M3 guide](https://developer.android.com/develop/ui/compose/designsystems/material3)
and linked official implementations corroborate shared theme subsystems and
component ownership. No measurements were invented from unavailable pages;
existing desktop motion/edge/halo adaptations remain explicitly documented.
