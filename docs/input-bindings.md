# Keyboard and menu controls

Shared keyboard defaults are in `config/input.toml`. The default is Finnish,
with a 350 ms repeat delay and 35 Hz repeat rate. Niri bindings are in
`compositor/niri/bindings.kdl`.

For Niri, individual settings may be overridden in
`$XDG_CONFIG_HOME/desktop-foundation/input.toml` (normally
`~/.config/desktop-foundation/input.toml`):

```toml
layout = "us"
# repeat_delay = 350
# repeat_rate = 35
```

Unspecified values retain their existing defaults. Optional profile-specific
`profiles/NAME/input.toml` is applied first, then alphabetically sorted
`input.d/*.toml` files under the same user configuration directory, and finally
`input.toml`. Inspect effective values and their source files with
`scripts/foundation config input --profile NAME`. Settings take effect when the
Niri configuration is regenerated and validated by deployment; editing a file
alone does not currently reload it. Invalid overrides stop generation before a
new configuration is published.

| Binding | Action |
| --- | --- |
| Super+Space | Launcher |
| Super+V | Clipboard |
| Super+B | Bluetooth |
| Super+X | System monitor |
| Super+Z | Control Center |
| Super+Y | Local AI |
| Insert / Delete | Next / previous track in End's media player |
| Shift+Insert | Microphone monitoring without ducking |
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
| End | Play/pause most recently playing media player |
| Home | Play/pause another available media player |
| Print / Super+Shift+S | Copy screen / selected region |

Menu toggles use `repeat=false`. Volume and navigation keep key repeat.

## Menus

Arrows, Tab/Shift+Tab and the wheel move selection. PageUp/PageDown moves by a
page; Home/End reaches the ends in menus without a search field. Enter or a click
activates. Escape closes or returns to the parent page. Search fields keep their
normal text editing keys.

- Clipboard: Ctrl+Shift+P pins an item, Ctrl+Shift+C copies plain text,
  Ctrl+Delete removes an item and Ctrl+Shift+Delete opens the clear dialog.
- Bluetooth: starts on the top paired device; Home selects the radio, End selects
  Manage. Space also activates. Explicit selection is retained as the list changes.
- Earbud controls: Left/Right adjusts a value; switches use Off/On. Enter/Space
  toggles or activates. Device readback confirms saved changes.
- Session: 1 Suspend, 2 Log out, 3 Reboot, 4 Power off; Enter/Space executes.

The session menu starts its entrance on its first frame and finishes its exit
before unloading. Volume has a 100 ms visibility fade and immediate bar updates.
The corner clock is click-through and shows hours/minutes.

`Super+Z` toggles the left Control Center. `Super+Comma` remains reserved;
`Super+X` continues to toggle the right btop sidebar.
