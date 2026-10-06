# Keyboard and menu controls

Shared keyboard settings are in `config/input.lua`. The default is Finnish,
with a 350 ms repeat delay and 35 Hz repeat rate. Niri bindings are in
`compositor/niri/bindings.kdl`; Hyprland bindings are in
`compositor/hyprland/bindings.lua`.

| Binding | Action |
| --- | --- |
| Super+Space | Launcher |
| Super+V | Clipboard |
| Super+B | Bluetooth |
| Super+Shift+Q | Session menu |
| Super+Tab | Niri overview |
| Super+Shift+C | Corner clock |
| Super+T / Super+Enter | Kitty |
| Super+E / Super+W | Files / browser |
| Super+Q | Close window |
| Super+D / Super+F / Super+A | Maximize / fullscreen / floating |
| Super+R / Super+C | Cycle width / center column |
| Super+arrows or HJKL | Focus window/column |
| Super+Shift+arrows or HJKL | Move window/column |
| Super+1…9 / Super+Ctrl+1…9 | Focus workspace / move window to workspace |
| PageUp / PageDown | Volume +3% / −3% |
| End | Play/pause |
| Print / Super+Shift+S | Copy screen / selected region |

Menu toggles use `repeat=false`. Volume and navigation keep key repeat.

## Menus

Arrows, Tab/Shift+Tab and the wheel move selection. PageUp/PageDown moves by a
page; Home/End reaches the ends in menus without a search field. Enter or a click
activates. Escape closes or returns to the parent page. Search fields keep their
normal text editing keys.

- Clipboard: Ctrl+Delete removes an item; Ctrl+Shift+Delete opens Clear all.
- Bluetooth: starts on the top paired device; Home selects the radio, End selects
  Manage. Space also activates. Explicit selection is retained as the list changes.
- Earbud controls: Left/Right adjusts a value; switches use Off/On. Enter/Space
  toggles or activates. Device readback confirms saved changes.
- Session: 1 Suspend, 2 Log out, 3 Reboot, 4 Power off; Enter/Space executes.

The session menu starts its entrance on its first frame and finishes its exit
before unloading. Volume has a 100 ms visibility fade and immediate bar updates.
The corner clock is click-through and shows hours/minutes.
