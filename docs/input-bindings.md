# Finnish input and iNiR/FEN bindings

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
| Super+Arrow | Directional focus/navigation |
| Super+Tab | Reserved for overview; inactive |
| Super+Space | Reserved for launcher; inactive |
| Super+V | Reserved for clipboard; inactive |
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
