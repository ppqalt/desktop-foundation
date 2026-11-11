# Motion and the gallery

Foundation uses Google's [Material 3 Expressive motion system](https://m3.material.io/styles/motion/overview/how-it-works).
Spatial motion may overshoot; effects such as opacity and color must remain bounded.
The spring constants in `Material.springs` are the unit-mass stiffness and damping
ratios from Google's [AndroidX ExpressiveMotionTokens v0_14_0](https://github.com/androidx/androidx/blob/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/ExpressiveMotionTokens.kt),
consulted 2026-11-11. They are physical parameters, not millisecond durations.
This reference does not introduce an Android runtime dependency or alter 0BSD licensing.

## Shared implementations

| Interaction | Production implementation | Gallery coverage |
| --- | --- | --- |
| Button press/release shape | `ControlAction` + `SpringValue` | Actions, Motion buttons |
| Switch state, pressed size and thumb travel | `SwitchIndicator` + `SpringValue`, used by `ControlSwitch` and setting `ApplicationRow` | Actions, Motion switch, desktop results |
| Slider pressed handle shape | `FoundationSlider` / `FoundationRangeSlider` + `SpringValue` | Input, volume, replay selection |
| Selected/hovered row colors | `SurfaceColorAnimation` in `ApplicationRow` / `ClipboardRow` | Desktop examples |
| Button/dropdown hover/press/focus state layer | `StateLayer` + `SurfaceAnimation` | Every gallery action and choice |
| Menu fade, interrupted close/reopen | `MenuPopup` + `SurfaceAnimation` | Every shared dropdown |
| Dropdown chevron open/close/reversal | `ControlChoice` + `SpringValue` | Input choices and Recording selector |
| Centered popup fade/scale and retirement | `PopupCard`, `PopupLifecycle`, `SurfaceAnimation` | Motion preview |
| Sidebar entry, exit and reversal | `SidebarMotion` | Motion preview |

Recording and audio device selectors use the shared dropdown's native `flat` variant: its base is transparent
so the surrounding group's wallpaper-derived surface shows through. It retains
shared hover/pressed feedback and keyboard focus, without a permanent field fill
or underline. Other filled dropdowns keep their existing appearance. Menu enter/exit uses the same
`MenuPopup` (100ms entry, 120ms exit) everywhere. During exit, the menu retains its
contents but cannot accept selection. Reopening reverses the current opacity.
Keyboard activation is owned on press; the corresponding release cannot trigger
ComboBox's native stale-highlight commit. Focus restoration occurs when dismissal
begins, so a delayed close cannot steal Tab focus. Plain string and textRole/object
models both render actual option labels. Narrow recording rows stack their action
below the selector. `ControlAction.minimumContentWidth` accounts for the actual
label, badge and style padding; recording/replay actions use it to avoid clipping
large text. Replay labels get a full row on narrow layouts, while Save and the
switch remain together and aligned to the right.

The gallery imports the production components. It must not copy their drawing,
spring parameters, curves or lifecycle controllers. Its sample state has no
system backend. Use Jump to section → Motion. Try toggling during entry/exit,
pressing and releasing controls quickly, and enabling reduced motion mid-flight.
The existing palette, text scale, layout direction and narrow-window previews
apply to this section too.

`SpringValue` initializes at the requested value without an unsolicited startup
animation. A new target retains the current rendered position **and velocity**;
there is no queue of obsolete destinations. Analytical integration uses actual
elapsed time, so dropped frames do not stretch the spring or make it unstable.
Completion snaps to the latest destination with zero velocity and emits once.
Changing reduced motion during a running spring settles it immediately.

Only presentation shape is animated on sliders. Pointer position, value, keyboard
repeat, range selection, and preview/commit signals remain immediate. Switch
animation follows confirmed `marked` state; clicking never claims backend success.
The setting-row indicator and the interactive switch use the same drawing code,
while their original row/button retains input and backend ownership.

## Desktop adaptations retained deliberately

Centered surfaces retain `Theme.timing` (160ms entry, 120ms exit), and the sidebar
retains its verified bounded Hermite slide (240ms entry, 180ms exit). These are
Foundation desktop adaptations, not Google's spring tokens. Long-lived sidebar
geometry must not bounce outside the display or regress cold opening. The external
Kitty/btop renderer retains its matching native implementation; the QML preview
does not pretend to render a terminal or certify native timing parity by itself.
Color interpolation and state-layer effects retain bounded shared duration/Bezier
transitions; they are not claimed to be physical springs.

An interrupted animation follows the latest intent rather than playing every old
request to completion. Full lifecycle means reaching the correct final state,
retiring only after exit/cleanup, and never allowing a stale close to dispose a
reopened view. The shared popup remains allocated if backend cleanup blocks its
retirement. Reduced motion skips presentation delay, never cleanup or confirmation.

## Click feedback and power confirmation

`StateLayer` retains a fast tap for 90ms, then animates to the current hover/focus
state. Rapid clicks replace the active pulse rather than queueing animations.
Reduced motion and disabled controls cancel it immediately. Actions, list rows,
clipboard pin/confirmation controls, switches, wallpaper tiles, dropdowns/menu
options, search/time inputs, replay timelines, sliders and footer actions use the
same feedback primitive. Scrollbar presses use the shared color effect. Passive
surfaces do not flash when clicking empty space; actions and value updates stay
immediate.

Flat selectors retain a quiet underline, emphasized for keyboard focus. Wallpaper
selection reflects the applied wallpaper; only keyboard focus draws the stronger
ring. A pointer click must not leave a keyboard-focus outline behind.

`PowerCard` and `PowerActions` own the same circular confirmation and two-stage interaction in Power and the gallery. First
activation isolates/enlarges that action's icon and hides other actions. A second
activation confirms. Escape first cancels the confirmation; Escape again closes.
Outside click closes without execution. Confirmed logout skips Niri's second prompt. Held activation keys never confirm, and
busy system actions remain protected. The component emits an action ID; only the
Power surface invokes the existing system helper. Gallery confirmation is inert.

## Verification

`tests/test-motion` exercises the production components and gallery: all six spring
presets; exact endpoint and one completion; reversal with preserved velocity;
30/60/144Hz sampling; delayed frames; non-bouncing effects; reduced motion during
spring, fade and sidebar entry; held slider dragging; popup reopen and blocked
retirement; and rapid gallery interaction. `tests/test-material-widgets` includes
this suite alongside the existing controls, modal handoff and Control Center checks.

Offscreen tests establish logic and shared rendering. Private Niri tests establish
Wayland loading/input/lifecycle behavior. Neither proves frame pacing under gaming
load on the owner's physical GPU. No live shell reload is required for these tests.
