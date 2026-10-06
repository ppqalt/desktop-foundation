# Finnish input and Niri reference bindings

Shared keyboard settings live in `config/input.lua`, independently of machine
monitor/GPU overrides. Both native backends use fi, repeat rate 35 and delay 350.
Niri binds are in `compositor/niri/bindings.kdl`.

| Binding | Niri behavior |
|---|---|
| Super+Tab / Space / V | Native overview / launcher / clipboard |
| Super+comma / Shift+7 / Shift+Q | Reserved controls / cheatsheet / power menu |
| Super+T or Enter / E / W | Kitty / default file manager / default browser |
| Super+Q / D / F / A | Close / maximize column / fullscreen / floating |
| Super+R / C | Cycle 1/3, 1/2, 2/3 column width / center column |
| Super+Shift+C | Toggle the small top-left clock |
| Super+arrows or HJKL | Native column/window focus |
| Super+Shift+arrows or HJKL | Move column/window |
| Super+1..9 / Ctrl+1..9 | Focus workspace index / move window to workspace |
| Super+Shift+S / Print | Drag-release region copy / current output copy |
| Super+B | Paired Bluetooth popup; arrows/wheel select, Enter activates, Escape closes |
| PageUp / PageDown | Audio +3% / −3% |

Finnish slash is Shift+7, evdev key 8. The reserved Super+Shift+7 chord is
explicit, and workspace movement uses Ctrl rather than conflicting Shift.
The adapter's strict single-window policy uses native CenterColumn on events;
multiple tiled windows in one column remain centered as a column.

The following is historical Hyprland validation; its secondary bindings remain
supported and have different native maximization/width semantics.

Page Up increases volume by 3%, Page Down decreases it by 3%, capped at 100%.
A compact top-center readout shows the actual resulting volume and a thin line
bar for 1.5 seconds. Successive presses replace the same readout. End toggles
MPRIS play/pause, preferring Spotify when present. playerctl/libnotify are included
in bootstrap; the volume helper and notification styling are tracked.

Automatic centering is restricted to exactly one window on the focused workspace,
with a tiled sole window. Two windows stacked in one column, or a tiled window
with floating companions, do not trigger it. Super+C still centers explicitly.

## Hyprland compatibility history

Shared keyboard preferences live in `compositor/hyprland/input.lua`: `kb_layout = "fi"`, with symbol-based resolution enabled. They load before host profiles. `profiles/*/input.lua` is reserved for optional device overrides; monitor/GPU files do not select keyboard layout. This setting follows the shared checkout to lucky38. The original iNiR/FEN files were not present on Tops; this port follows the explicit binding list supplied by the user, with system-default applications.

| Binding | Current behavior |
| --- | --- |
| Super+T, Super+Enter | Kitty |
| Super+E | System-default file manager (`xdg-open "$HOME"`; currently COSMIC Files) |
| Super+W | System-default browser (`xdg-open about:blank`; currently Brave Origin nightly) |
| Super+Q | Gracefully close focused window |
| Super+D | Toggle native maximized state |
| Super+F | Toggle native fullscreen state |
| Super+A | Toggle floating |
| Super+R | Cycle native tiled-column width presets |
| Super+C | Center tiled column, or center floating window on the monitor |
| Super+Shift+C | Toggle the small top-left clock |
| Super+Arrow | Directional focus/navigation |
| Super+Tab | Reserved for overview; inactive |
| Super+Space | Application launcher |
| Super+V | Clipboard history |
| Super+, | Reserved for settings/control UI; inactive |
| Super+/ (Finnish: Super+Shift+7) | Reserved for cheatsheet; inactive |
| Super+Shift+Q | Reserved for power/session menu; inactive |

Reserved chords are explicit no-op bindings, with “not implemented” descriptions. They are consumed without starting processes or creating UI. Hyprland has no native Niri-style overview; no substitute plugin or shell surface was added. Super+Shift+Q does not immediately exit the session. The temporary direct exit remains Super+Shift+M; save work before using it.

Finnish slash is the shifted symbol on `<AE07>`: physical key 7, evdev code 8 / XKB code 16. The cheatsheet reservation uses `SUPER + SHIFT + code:16`, avoiding ambiguity about a shifted slash keysym and consumed Shift modifiers. Native keyboard-event testing confirmed that it is intercepted. Hyprland's legacy key/keycode fields in `hyprctl binds` do not fully serialize Lua multi-key/keycode bindings; that inspector alone is insufficient to validate this chord.

The temporary Super+Shift+7 workspace-move binding was removed to prevent a conflict. Super+7 still focuses workspace 7. Other temporary Super+digits/Super+Shift+digits, Super+Shift+arrows, mouse move/resize and Super+period scrolling remain. Reverse column scrolling moved from Super+comma to Super+Alt+comma, leaving the requested settings chord free. No old terminal-on-Super+Q or close-on-Super+Shift+C binding remains.

Width cycling uses native `colresize +conf` and current upstream presets (approximately 1/3, 1/2, 2/3, full width); original FEN preset values were not supplied. It applies to normal tiled columns, not floating windows. Maximization/fullscreen remain separate native states: use Super+D/Super+F to leave them before comparing visible column-width changes. Centering chooses the native tiled or floating operation using a nonblocking Lua callback; no process probing or polling runs in the compositor event loop.

Validation on Tops, 2026-09-30:

- Lua syntax/config verification, repository checks, live reload and empty configerrors passed. All eight real keyboard entries report `fi` / “Finnish”. The actual live Wayland keymap was captured with `xkbcli dump-keymap-wayland --raw`; no separately compiled guessed keymap was used for the live checks.
- A temporary Quickshell input receiver and a temporary kernel uinput keyboard tested real event delivery through Hyprland. This machine already grants the user access to /dev/uinput; no permission changes, package installation or resident test helper was needed. Received Finnish output matched: `7 / , ; ä ö å + ? \ @ € { [ ] } < > | :`, including Shift and AltGr combinations.
- All six reserved UI chords were consumed by the compositor without appearing as nonmodifier key presses in the receiver. Super+Shift+7 left the test window/workspace unchanged. Super+comma did not scroll the column. Super+Shift+Q did not close the test client or exit the session.
- Synthetic keyboard events exercised the configured bindings themselves, rather than only calling their dispatchers: both terminal chords opened new Kitty windows; Super+E opened COSMIC Files; Super+W opened Brave Origin nightly; Super+Q closed only owned test windows. Normalized test-window states then verified maximize `0→1→0`, fullscreen `0→2→0`, floating toggle, centering of tiled/floating windows, and native width cycle (1247 → 1873 → 619 → 933 pixels on the current output). These pixel counts are measurement results, not configuration constants. Left/right navigation was tested between columns; up/down between two test windows stacked in one column.
- The first width test encountered maximized test windows and was repeated after clearing only their fullscreen/maximized state, without changing user application preferences or shared layout defaults. All owned test app windows, temporary input receiver and synthetic keyboard were removed, and original focus restored. The normal resident shell still has no visible surface. No COSMIC configuration or monitor/GPU settings were modified.

The synthetic test validates actual compositor/client key processing, but cannot verify the labels or mechanical behavior of the physical keyboard. Physical Finnish typing can additionally be inspected with `xkbcli interactive-wayland`; no physical-keyboard test is claimed.

References: [Hyprland binds/keycode syntax](https://wiki.hypr.land/configuring/core/binds/), [native scrolling messages](https://wiki.hypr.land/Configuring/Layouts/Scrolling-Layout/), [Lua snippets and native window access](https://wiki.hypr.land/configuring/code-snippets/). API behavior was checked against the installed version and upstream v0.56.2 source, then tested live.

Launcher phase: Super+Space is now active and toggles the lazy launcher. Its other UI chords remain reserved.

Super+Shift+S selects a screenshot region; Print captures the focused output.
Both save a PNG in ~/Pictures/Screenshots and copy it to the clipboard.

Volume feedback uses the shared shell theme: one noninteractive top-centre
readout, 2 px animated bar, no notification stacking, 1.5 s hold then fade.
If the shell is unavailable, volume still changes and a text-only Mako toast
is used. No additional daemon or glyph-based progress bar.

Super+Shift+C toggles a compact top-left 24-hour clock on the focused output in
either compositor. Super+C retains its existing centering action. The clock
starts hidden, uses the shared graphite palette and Google Sans Code, and is
click-through: it takes neither input nor desktop space. The existing Rust
backend sends a single `toggleClock` shell IPC request. QML's native SystemClock
updates at minute precision while shown; hiding the lazy surface destroys its
clock/timer. No date/seconds, Python helper, resident clock process or saved
visibility state is added. `scripts/clock` toggles it from the command line.

Super+Shift+Q toggles the graphite session menu in Niri and the retained Hyprland
configuration. Actions: 1 Suspend, 2 Log out, 3 Reboot, 4 Power off. Arrows select;
a single click, Enter or the action number executes immediately, as requested.
Escape closes the menu. Key repeat is ignored. The shared shell provides the
surface; no extra daemon. Native systemctl/logind handles power operations;
logout uses session-exit. Errors remain visible. `scripts/power-action --check ACTION`
validates routing without changing the running session.

Mouse wheel and touchpad vertical scrolling move the highlighted selection in
the power menu, application launcher and clipboard. Wheel notches advance one
item; touchpad deltas accumulate to avoid erratic jumps. Results stay in view,
and scrolling alone never launches, copies or executes a power action.

## Surface keyboard controls

The application launcher and clipboard keep keyboard focus in their search
field. Type normally; Left/Right, Home/End, text selection and Space retain
normal text-editing behavior. Up/Down (also Ctrl+P/N), Tab/Shift+Tab and
PageUp/PageDown move through results, keeping the selected item visible. Enter
opens the selected application or copies the clipboard item. Escape closes the
surface. Holding Enter does not repeat an action.

Clipboard Tab navigation also reaches **Clear all**, which receives the same
visible selection treatment. Up at the first result or Down at the last result
also reaches it; the next arrow returns to the last/first result. Ctrl+Delete removes the highlighted item;
Ctrl+Shift+Delete opens the existing clear-history confirmation. Its Cancel and
Clear history actions are selectable with Tab/Shift+Tab or arrows, then
Enter/Space activates the highlighted choice. Escape cancels. While the
confirmation is visible, typing and scrolling cannot change the search or the
covered list, and deletion/copy shortcuts cannot affect an underlying item.

The power menu accepts Tab/Shift+Tab and arrows to select, Home/PageUp and
End/PageDown to reach the first and last actions, Enter/Space to execute, and
Escape to close. The existing immediate action keys 1–4 still work. Holding an
activation key cannot repeat a power action; holding navigation keys can move
selection normally.

Super+B initially selects the top paired device. While the list is still empty,
the radio is highlighted; the first arriving device becomes selected only if
no keyboard, wheel or pointer choice has been made. Explicit Manage/radio choices
and device identity survive subsequent list updates. Reopening starts fresh.

Launcher, clipboard, Bluetooth, earbud controls and power now share one footer,
graphite card/selection language, spacing and entrance/exit timing. Escape/Back
and the selected action hint are also clickable. Power retains its immediate
1–4 actions, while search fields retain ordinary text editing. Clock and volume
remain appropriately small, passive overlays using the same theme.

Clipboard uses the standard fade/scale entrance once output geometry resolves.
Initial sizing and history loading do not run the confirmation resize effect;
the existing Clear all shrink/expand transition keeps its duration and easing.

Niri menu toggles (Space, V, B, Shift+Q, Shift+C and Tab with Super) use native
`repeat=false`: a held chord toggles once, and release/press toggles again. The
input repeat settings and volume/navigation bindings retain their useful repeat.

The session menu starts its entrance on its own Qt window's first presented
frame, rather than at construction. Closing it with the shortcut or opening a
different menu goes through the same animated dismiss path as Escape; its loader
is removed only when that transition completes. Pending power actions cannot be
discarded by another menu shortcut.

Volume readout uses only a 20 ms fade in/out. Percentages and bar positions update
immediately from the same reported Rust value, with no bar interpolation. The
native 3% step, 100% cap, mute readout and 1.5-second expiry are preserved.
