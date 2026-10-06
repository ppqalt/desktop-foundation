# Surface quality-of-life pass

The Nothing controls keep a stable six-row footprint while loading. The Rust
helper publishes a complete ready snapshot, including feature availability;
QML stages partial replies and reveals the settings together. Persistent switches
and value badges show reported state, pending labels identify the active request,
and a short accent confirms successful matching readback. No optimistic toggle
state, polling, permanent helper or new runtime Python code is added.

Interactive surfaces retain keyboard operation and the shared graphite palette:

- Launcher: search editing, arrows, Ctrl+P/N, Tab/Shift+Tab, visible-page movement,
  Enter and Escape. Held Enter cannot repeatedly launch.
- Clipboard: the same search/navigation keys; boundary arrows and Tab reach
  Clear all. Ctrl+Delete removes the selected entry, Ctrl+Shift+Delete opens
  confirmation. The compact confirmation uses the ordinary graphite card,
  without a large elevated-color overlay. Cancel/Clear history have actual
  button focus, arrow/Tab movement, Enter/Space activation and Escape cancellation.
  Covered search/list actions are disabled until confirmation closes.
- Bluetooth: radio, every device and Manage are reachable with arrows,
  Tab/Shift+Tab, Home/End and paging; Enter/Space activates. Native device identity
  preserves selection across list changes. Returning from Controls restores
  parent focus after the lazy item unloads. A stationary pointer cannot take
  selection away from keyboard navigation.
- Nothing controls: navigation reaches every row, including information on small
  outputs. Left/Right adjust values; ordinary switches use Off/On respectively.
  Enter/Space activates, Escape backs out or cancels loading. Adjustment keys do
  not start ringing, fit tests, disconnection or firmware changes.
- Power: arrows, Tab/Shift+Tab, Home/End, PageUp/PageDown, Enter/Space, immediate
  action keys 1–4 and Escape. Activation repeats are suppressed.

Clock, volume readout and overview backdrop remain passive, click-through layers.
Their existing compositor shortcuts provide their interaction; they do not take
keyboard focus. Existing desktop shortcuts and manual column centering are kept.

## Isolated validation, 2026-10-05

`scripts/check` passed formatting, lint, native configuration validation, Rust
Clippy and **50 Rust tests**. The installed Quickshell metadata still emits its
existing type warnings. `scripts/test` discovered **211 cases: 209 passed and
two opt-in desktop/Wayland cases were skipped**.

The added policy fixtures extract the actual production handlers and functions
for launcher, clipboard, power and Bluetooth, using inert action spies. They
exercise navigation, repeated input, modal protection, selection preservation
and deferred focus without calling system services.

Two Qt fixtures additionally deliver keys through actual focused Qt windows:

- `tests/test_nothing_qml_qol.py` loads the real Nothing surface and shared rows,
  with private, inert executables. It checks staged loading, fixed geometry,
  paging, subpages, switches, failure/readback confirmation and cancellation.
- `tests/test_clipboard_qml_focus.py` hosts the production clipboard contents in
  an offscreen Window, replacing only the outer layer-shell transport. It uses
  private history plus an inert worker to verify Clear all, both confirmation
  buttons, typing, cancellation, restored focus and one clear action.

Both fixtures use the offscreen software renderer and private HOME/XDG paths,
with compositor/display and D-Bus environment removed. Optional private captures
verified the loading/ready cards and compact clipboard confirmation. This does
not validate layer-shell exclusivity, physical key delivery or hardware actions.
No testing on the running desktop, radio, audio, clipboard or power services was
performed. Deployment updates the managed QML and rebuilt Nothing helper; normal
source watching handles reload without restarting the compositor.

## Consistency follow-up, 2026-10-06

All interactive shortcut menus now share `SurfaceFooter.qml`, including keycaps,
separator, action hint and clickable Escape/Back. Power uses the same SurfaceCard,
640px maximum width, 62px rows, icon tiles, selected border and fade/scale as the
other list menus. Clipboard's compact confirmation keeps its existing animated
size transition. Its initial output geometry settles without that size morph;
the opening uses the common fade/scale instead. The application launcher uses
the same entrance scheduling. No desktop keybindings or global blur/opacity
settings change.

Bluetooth's empty startup list no longer treats index zero as a remembered
Manage choice. A separate initial-selection flag chooses the first paired row
when available, until deliberate keyboard, wheel, click or pointer interaction.
Subsequent device updates retain explicit choices and device identity; removal
falls back to a device/radio rather than accidentally selecting Manage.

Isolated actual-Qt checks cover Power's focus, inert action failure and animated
keyboard/mouse exit, alongside existing Nothing/Clipboard navigation. Clipboard
also checks initial geometry settling and the retained confirmation morph.
Bluetooth initial/late list loading, explicit choices and reopening are checked
against extracted production policy with fake data. Offscreen Power and compact
Clipboard previews were reviewed. Physical input/layer-shell behavior and live
desktop actions remain untested, per the user's restriction.

Shortcut menus retain the underlying application scene with the existing dim
scrim and blur. Their scoped Niri layer rule explicitly uses `xray false` so blur
samples the windows beneath the menu rather than replacing them with wallpaper.
Normal application-window rules and the separate overview backdrop keep their
existing behavior. Non-xray blur needs recomputation when underlying content
changes while a menu is shown; no hidden menu renders or polls. This setting was
validated through the native config parser, without a live screenshot/input test.

## First-frame and shortcut follow-up, 2026-10-06

The running shell could retain an old lazy component when file-watcher reload
began partway through a batch of source replacements. `scripts/shell-reload`
requests a supported final hard QML reload after the files are complete, without
restarting Niri or applications. The IPC reply is sent before reload starts; it
defers while a power action is pending. Use it after deliberate QML source updates
instead of treating a mid-update watcher reload as proof the final source loaded.

Power's entrance now starts from the card's own `Window.frameSwapped` signal.
Construction-time callbacks can finish before a layer is first presented. The
one-shot connection stops once entered or closing; no permanent frame timer is
added. Shortcut and cross-menu closure now call `dismiss()` rather than directly
disabling the lazy loader, so the shared exit animation can finish.

Isolated checks hold the test window hidden while another window renders, then
observe initial opacity/scale `0/.97`, intermediate values and final `1/1`.
Production shell policies are exercised with action spies; a private Quickshell
instance with watching disabled proves explicit reload reads an updated lazy
component. Native Niri parsing validates the single-press repeat flags. None of
these checks actuates the running desktop or any real power operation.

Volume feedback animates only visibility with a 100 ms fade in/out, starting on
its own first frame. Its bar and percentage update together immediately without
interpolation. One readout, mute indication and 1.5-second expiry remain. Rust
audio mutation/readback and fallback ordering are unchanged.

A private Qt presenter check verifies initial bar position, immediate updates,
mute/cap, hide/reopen and expiry without running an audio command. The source
reload wrapper also distinguishes a busy session action from Quickshell's brief
not-ready response, waiting within a bounded request before retrying only this
idempotent reload. An acknowledged reload is never replayed.
