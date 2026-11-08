# Design-system inventory

Source baseline: `c549bf3` plus the subsequent installer work, inspected on
2026-11-08. This is a source inventory, not a claim of visual or physical-device
acceptance. The earlier [widget audit](widget-audit.md) describes the lifecycle
migration and its historical baseline; this document records the current
component boundaries and outstanding design-system work.

| Interface | Current implementation | Reuse and remaining work |
| --- | --- | --- |
| Control Center, audio and Transparency | `surfaces/ControlCenter.qml`, Rust snapshots and actions | `ControlAction`, `ControlChoice`, `ControlVolume`, `ControlGroup`; application streams also use `ControlVolume`. Pending and confirmed state remain with the page. The page still has local layout, visible-only snapshot polling. Transparency's audio transaction is a separate backend concern. |
| Recording and Instant Replay | `components/ReplayControl.qml`, `services/RecordingState.qml` | Uses `ControlAction` and `ControlSwitch`; state comes from the recorder service. No replay-duration slider or five-minute default exists. Preserve the existing recording configuration until that feature is explicitly implemented and verified. |
| Wallpaper picker | `components/WallpaperGrid.qml` | Four columns, asynchronous previews and backend-owned apply state. Tile is a specialized `AbstractButton` with its own focus, keyboard, selected and image presentation; it is not yet an extension of a general tile primitive. |
| Launcher and Clipboard | `surfaces/Launcher.qml`, `surfaces/Clipboard.qml`; `ApplicationRow`, `ClipboardRow` | `PopupWindow`, `PopupCard`, `SearchInput` and routing are shared. Rows and clipboard confirmation/actions still have specialized pointer and visual implementations. Search must retain editing semantics. |
| Power | `surfaces/Power.qml` | Shares `PopupWindow`, `PopupCard`, action rows and lifecycle. Confirmation and busy protection are domain-specific. |
| Bluetooth and Nothing controls | `surfaces/BluetoothPopup.qml`, `surfaces/NothingControls.qml` | Shared popup, rows, actions and switch; device sessions and nested controls have specialized lifecycle and pointer handling. Audio/device actions must remain isolated in tests. |
| Cachy Update | `components/CachyUpdateRow.qml` | Shared compact `ControlAction`; existing updater owns actual updates. |
| Volume, clock and diagnostic feedback | `surfaces/Volume.qml`, `Clock.qml`, `Probe.qml` | Passive surfaces, some local geometry/rectangles and text. They must not steal focus; volume updates are intentionally immediate. |
| Backgrounds | `ModalBackdrop.qml`, `OverviewBackdrop.qml` | Noninteractive compositor surfaces, not buttons or dialogs. Their different ownership and visual purpose are deliberate. |
| Monitor | Native Kitty/btop surface via `ExternalSurface` | External renderer with its own settings and lifecycle, not a QML control. Existing audit records geometry, motion and reduced-motion gaps. |
| Notifications | Mako, configured by generated `notifications.conf` | Generated theme roles and a provider-specific format; not a QML notification component. Preserve the D-Bus owner and provider behavior. |
| Screenshots | Rust `screenshot.rs`, Niri, `slurp` and `grim` | Capture and selection are native/external interfaces. They are not evidence of a shared QML dialog or notification. |

## Current shared pieces

`Theme.qml` owns the watched semantic palette, fallback colors, font family,
legacy spacing, radii and motion. `Material.qml` exposes generated Material
roles, type, shape, motion and focus/navigation helpers. Both are imported by
most controls: this is a split token authority, not yet the requested single
system. Generated palette changes are watched; this inventory does not establish
that every external renderer changes live.

Actual reusable controls include `ControlAction` (list, compact, toggle and icon
variants), `ControlSwitch` (backend-owned marked/pending state), `ControlChoice`,
`FoundationSlider`, `ControlVolume` (its audio extension), `FoundationScrollbar`, `SearchInput`, `ControlGroup`, `SurfaceCard`, `PopupCard`,
`PopupWindow`, `PopupLifecycle`, `FocusIndicator`, `WidgetStatus` and
`SelectionWheel`. `ControlRequest`, `SurfaceRouter` and `ExternalSurface` are
lifecycle/transport helpers rather than visual controls. Existing row components
serve specialized content. Reuse or extend these before introducing aliases or
parallel implementations.

The component library is incomplete: there are no general Foundation checkbox,
radio, dialog, tooltip, progress or notification primitives with
verified common contracts. Some controls embed raw geometry, opacity and color
literals; several surfaces still use `MouseArea`-based actions. A `MouseArea`
alone is not evidence of a duplicate control: some handle outside dismissal,
selection, dragging or other specific input. Audit each before replacement.

## Verification baseline and next migrations

`tests/test-material-widgets` exercises production controls in an offscreen
Quickshell host, including navigation, search editing, popup lifecycle and
reduced motion. `tests/test-control-center` exercises the actual sidebar with
inert workers, including application audio, update and replay presentation.
Wallpaper and recording have separate fixtures. These checks do not prove real
compositor focus, physical audio, live palette changes in every application,
light themes, screenshot equivalence, or memory/performance budgets.

The [component gallery](../component-gallery.md) previews a subset of
production controls, example desktop compositions and sample light/dark Matugen
palettes. Next: make geometry/state-layer/typography tokens
authoritative without changing existing sizes; extend the gallery to cover all
components and variants;
then migrate one surface at a time with interaction, accessibility, palette and
visual checks. Keep popup/provider and native-process ownership intact. Add
static checks only for specific verified conventions, avoiding blanket bans on
legitimate specialized primitives or colors needed by external integrations.
