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
