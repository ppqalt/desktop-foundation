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
| CC Cachy Update | Cached details plus explicit availability check | Shared compact Details/Check actions in a tonal section | Finite ControlRequest runs arch-update --check; retained through close, reports failures | Checking never installs packages or runs df-update |
| Volume | Transient passive readout; Rust volume backend | No focus; immediate value; first frame + 100ms fades | One 1500ms expiry; zero interpolation of volume | Intentional readout timing; shares notice surface with Transparency |
| Transparency notice | Transient passive readout; Rust socket-activated transaction | Same Volume surface; hearing/status icons | No QML audio logic; owned active-mode subscriber and crash recovery | Preserve Pause/Shift+Pause, application volume ownership and exact restoration; no live audio testing |
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

## Mirrored sidebar comparison (2026-10-31)

The right monitor and left Control Center are the **same interactive-sidebar
category**. The monitor is not exempt from shared infrastructure requirements.
Its renderer and btop save handshake are the exceptions, not its geometry,
accessibility, theme or motion policy.

| Concern | Left Control Center | Right Kitty/btop | Actual sharing / gap |
| --- | --- | --- | --- |
| Dimensions and timing | Rust-supplied `sidebarStyle` | Rust `Settings` from `config/monitor.toml` | Same settings loader: width 720, padding 12, radius 14, alpha .75, open 240ms / close 180ms |
| Output bounds | Width capped to output width | Width capped to output width minus 26 | Narrow-output behavior differs; not a single geometry calculation |
| Surface | QML semantic surface, primary outline/tonal halo, inner corners | Kitty terminal palette and rounding shader | Same theme transaction, different surface roles; monitor has no matching outer accent outline |
| Motion | `SidebarMotion.qml`, frame-driven normalized Hermite interpolation | `monitor.rs::Motion`, 8ms native timer and Kitty placement | Equivalent equations copied across languages, **not one shared implementation** |
| Reduced motion | `Theme.reducedMotion` settles immediately | Native monitor has no reduced-motion branch | Accessibility gap |
| Keyboard focus | OnDemand while opening/open; None when closing/superseded | Not allowed during movement, OnDemand after fully open | Both non-exclusive; activation timing differs |
| Toggle ownership | Shell `SurfaceRouter` | Rust monitor socket and retirement lifecycle | Separate intent owners; no shared cross-sidebar arbitration |
| Close/reopen | Reverse QML slide, drain finite operations before unloading | Reverse native slide, short retirement grace, SIGINT btop save, remember intent during save | Renderer-independent intent/motion can converge; terminal save/child ownership must remain native |
| Closed cost | LazyLoader destroys view and refresh workers | Owned terminal/monitor exits after save | Both on-demand; no permanent hidden btop sampler required |
| Keyboard content | Shared Material controls and navigation | btop's native terminal keyboard/menu handling | Intentionally different content, not a reason for different outer lifecycle policy |

Recommended next boundary: give mirrored sidebars a shared Rust-owned settings,
geometry and transition/intent model, with thin QML and Kitty placement adapters.
Do not add a second permanent process, send per-frame command invocations, embed
btop into a popup, or force terminal content through Material control navigation.
A shared in-process motion implementation needs an explicit bridge/build design;
the present copied QML/Rust equations must not be described as architectural
unification. Until that bridge is selected, retain the working adapters and test
reversal trajectories against the same fixtures. Preserve the btop settings and
graceful save path throughout that migration.

This comparison was source-only: no sidebar was opened, no audio/network setting
changed and no live btop configuration was rewritten.

### Follow-up: shared switching

Public `monitor toggle` now enters the same `SurfaceRouter` as Control Center,
Clipboard, Bluetooth, Power and Launcher. The router accepts an external-renderer
adapter instead of treating btop as an unrelated overlay. `ExternalSurface` uses
`ControlRequest` to serialize native toggle/close requests, coalesces queued toggle
pairs, and lets a later close supersede queued startup intent. Rust remains the
authority for actual monitor state, motion and btop retirement; QML does not poll
or mirror that state. Both sidebars use the router's `independent` category:
centered widgets and the other sidebar can open without dismissing either one.
Only their own toggle or explicit close action requests close/save. Centered
popups continue replacing other centered popups. The existing busy Power guard applies to monitor
requests too. `toggle-native` is the adapter transport entry, not a shortcut path.

The monitor also reads the existing shared reduced-motion preference on service
startup and settles zero-duration motion safely. The copied interpolation,
narrow-output geometry, focus timing and outer border differences above remain
explicit gaps; this change does not claim to eliminate them.

## Opening-path correction, 2026-11-10

The asynchronous retained Control Center experiment was removed after owner
reports of delayed, stuttering and unreliable opening. Discovery is now gated by
an explicit settled-open state. SidebarMotion waits for the surface's own first
frame; cancellation before that frame settles closed immediately. Deferred
initialization respects a close intent. The view returns to normal safe disposal.
Both public sidebar shortcuts still dispatch one IPC request without a preliminary
status probe, and existing duration/curve settings are unchanged.

A full-shell comparison in a private Niri session reproduced a 241 ms opening
frame gap with 500 wallpaper entries in the previous implementation. Deferring
model discovery until after opening reduced the largest observed opening gap to
17 ms in the same virtual/software-rendered scenario. This is not a Lucky38
physical presentation measurement. Native Kitty/btop cold startup remains its
own renderer boundary.

### Control Center live state (2026-11-12)

The older visible-only snapshot-loop entries above describe the audit baseline.
Production now projects `ControlCenterState.current` from a shell-owned Rust
`control-center watch` stdio session. PipeWire/PulseAudio and NetworkManager
subscriptions feed changes while the lazy panel is closed; opening adds no scan
and no Refreshing status. Epoch barriers protect action readback. Service
reconnection, idle blocking and owned-child cleanup live in Rust. The finite
snapshot fallback remains for standalone hosts/cache fixtures, without adding a
second production watcher or retaining a hidden sidebar. See
[Control Center](../control-center.md) for the current ownership and checks.

### Control Center keyboard follow-up (2026-11-13)

Control Center keeps the keyboard focus reason when committing a page, so keyboard
navigation to Audio/Network retains the shared focus indicator. Focus acquisition
on an interactive control retires pending first-snapshot focus placement; device
readback must not move a user's current selection. The redundant unconditional
mute-button focus request on construction is removed. Arrows/WASD on an empty
page focus owner enter an available control, including when devices are unavailable.

`DF_CONTROL_KEYBOARD_TEST=1 tests/test-control-center` exercises the production
Control Center with inert system workers in a fixed short window: arrows/WASD,
Tab/Shift+Tab, disabled Wi-Fi, Audio/Network entry and return, dropdown Escape,
slider adjustment versus vertical navigation, focus reveal and readback without
focus theft. The native integration suite invokes this fixture. Production
controls still use the existing shared navigation, slider and dropdown handlers.

A separate source copy was checked in the CachyOS/Niri VM with Wayland input and
physical QMP key events; the owner's active desktop and the guest's installed
checkout were not replaced. VM device coverage was its virtual audio/Ethernet;
this check does not establish physical Bluetooth or recording hardware parity.

### Continuous arrow traversal correction (2026-11-13)

The preceding keyboard pass covered page focus and selected routes, but missed
whole-section traversal. Centre-projection scoring could skip the audio columns
from a full-width action, skip Cachy Update for better-aligned wallpapers, and
move Right into a different row. The shared `Material.moveFocus` now compares
control rectangles, visits the adjacent row before alignment tie-breaks, retains
the horizontal column through wide rows, and clamps horizontal traversal to the
current visual row. `navigationBoundary` limits a composition's candidates;
Control Center marks its focus owner, and the widget test marks its overlapping
test composition so navigation cannot enter controls drawn behind it.

The keyboard fixture now includes enabled recording/replay controls and wallpaper
tiles. It performs an uninterrupted arrow-only walk through Networking, both
audio columns, Application volumes, Recording, Replay, all three Cachy Update
buttons and Wallpapers, then back upward. No focus is assigned between those
walk steps. Additional physical QMP key events in the VM confirm Details → Check
→ Update and reverse traversal, row-edge clamping and return through audio.
