# Sidebar architecture

Desktop Foundation's left control center will use QML for presentation and Rust
for behavior. The right monitor uses a Kitty layer-shell panel running genuine
btop, controlled by the Rust backend. Super+X toggles it. The left control center
remains groundwork.

The existing shell supplies the semantic palette, focused-output selection,
layer-shell surfaces, lazy loading, keyboard navigation and finite Rust actions.
Volume steps, Bluetooth mutations, application launch and session actions can be
reused. Bluetooth and PipeWire display state currently live in QML; shared Rust
subscriptions and absolute volume/mute controls would need further work.

## Right monitor options

The reference versions are Niri 26.04, Kitty 0.49.2, btop 1.4.7 and Quickshell
0.3.1. Performance comparisons below describe architectural costs; they are not
benchmark results.

| Host | Integration and maintenance | Animation | Resource cost |
| --- | --- | --- | --- |
| Ordinary floating Kitty + btop | App-id rules and Niri IPC position a real terminal. It belongs to a workspace; hidden-workspace and focus handling add complexity. | Niri animates window opening/closing and movement. Position clamping prevents simply sliding it fully offscreen to hide it. | Kitty and btop run while open; keeping them hidden does not stop btop sampling. |
| Kitty's built-in layer-shell panel + btop | Selected host; dedicated namespace, overlay layer and isolated configuration. | Built-in show/hide is immediate. Desktop Foundation slides the real fixed viewport using supported panel-position RPC. | Kitty and btop exist only while the sidebar is open; the owned service cleans up both on close. |
| Native QML monitor + Rust collectors | Reuses the shell and semantic theme, but owns monitoring code. It is a custom monitor, not btop. | QML animates content translation and opacity in one fixed surface, matching the control center. | No second terminal renderer; collectors can exist only while visible. |
| btop rendered inside QML | Requires a PTY plus a terminal renderer covering ANSI, truecolor, alternate screen, input and resize. No such component is present. | QML can animate the host item once a renderer exists. | Adds a dependency and terminal integration maintenance. Foreign Wayland Kitty cannot simply be embedded as a QML item. |

The selected Kitty host keeps btop's native behavior. Its panel remains anchored
to the output with a fixed terminal viewport; Rust animates its positive left
margin from beyond the right edge to the final inset. This uses Kitty's public
`resize-os-window --action=os-panel` interface and Niri's layer placement, rather
than window hiding or terminal reflow. See [monitor behavior](monitor.md).

btop needs an interactive terminal and has no supported JSON/export API. Parsing
its screen would not create a stable Rust monitoring backend. Its native
`--config` and `--themes-dir` options allow isolated configuration and colors;
Ctrl+R or SIGUSR2 reloads configuration and theme without restarting it.

btop's CPU box requires at least **60 columns**. Showing CPU, memory, network and
processes requires **80 columns**: memory/network use 36 beside the 44-column
process box. A narrow drawer therefore needs a reduced-box preset, smaller font
or more width. It cannot assume that the full btop layout fits at the desktop's
normal terminal font size.

## Shared lifecycle contract

Future hosts should share these behaviors without making presentation depend on
a particular monitoring engine:

- Resolve the focused output when opening. Keep it stable until closing, and
  handle output removal explicitly.
- Track closed, opening, open and closing states. Repeated toggles reverse an
  active transition instead of launching duplicates or stacking animations.
- Animate QML content within a fixed anchored surface; begin after its first
  presented frame. Keep the surface alive until the exit animation finishes.
- Use the overlay layer when interaction must work over fullscreen applications.
  An overlay drawer should not reserve work area or resize tiled windows.
- Release keyboard focus on close, restore the prior application where possible,
  and preserve Escape and full keyboard operation. Decide outside-click behavior
  alongside the control center design.
- Start sampling only while the monitor is visible. Bound worker lifetime and
  terminate owned children on failure; hiding a terminal alone is insufficient.
- Reuse semantic palette roles and transactional theme revisions. A failed
  adapter render must leave the last complete revision active.

Theme bundles contain `btop/desktop-foundation.theme`. The sidebar uses an isolated
btop configuration and updates the owned process when colors change. Its Rust
actor has no idle timer while fully open and exits with its children on close.

Sources: [Kitty panels](https://sw.kovidgoyal.net/kitty/kittens/panel/),
[Kitty Wayland visibility](https://github.com/kovidgoyal/kitty/blob/v0.49.2/glfw/wl_window.c#L1688),
[Niri layer lifecycle](https://github.com/niri-wm/niri/blob/v26.04/src/handlers/layer_shell.rs#L171),
[Niri floating positions](https://github.com/niri-wm/niri/blob/v26.04/src/layout/floating.rs#L128),
[Niri layer focus](https://github.com/niri-wm/niri/wiki/Layer%E2%80%90Shell-Components),
[btop theme roles](https://github.com/aristocratos/btop/blob/v1.4.7/src/btop_theme.cpp#L45),
[btop CLI](https://github.com/aristocratos/btop/blob/v1.4.7/src/btop_cli.cpp#L232),
[btop lifecycle](https://github.com/aristocratos/btop/blob/v1.4.7/src/btop.cpp#L958),
[btop dimensions](https://github.com/aristocratos/btop/blob/v1.4.7/src/btop_draw.cpp#L483),
[btop layout sizing](https://github.com/aristocratos/btop/blob/v1.4.7/src/btop_tools.cpp#L110),
[Qt foreign-window platforms](https://doc.qt.io/qt-6/qtdoc-demos-windowhosting-example.html).
