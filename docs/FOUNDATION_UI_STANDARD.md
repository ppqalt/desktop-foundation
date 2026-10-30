# Desktop Foundation UI engineering standard

This is normative guidance for developers and coding agents. Read it before
changing a UI component. “Must” describes an acceptance requirement. A component
that does not satisfy it has a migration defect; documenting the requirement does
not make the component compliant. Keep this document synchronized with code.

## Scope and precedence

See [the official Google reference index](MATERIAL_REFERENCES.md) for the full
canonical foundation/style/component link catalogue, including specifications,
guidelines and accessibility pages.

All Foundation-owned windows, widgets, lists, menus, dialogs, popups, overlays,
notifications and readouts form one UI. Supported platform: CachyOS + Niri.
CachyOS + Niri is the officially supported reference configuration; unsupported
environments are not prohibited. Prefer capability checks over distribution-name
blocks, and do not promise another supported compositor integration. Existing
legacy gates are documented in the root handbook as a migration gap.
QML owns presentation and immediate interaction; Rust owns actual system actions,
validation, persistence and backend data. No new Python code or tests.

User requirements override this standard. Material specifications govern design
where there is no explicit desktop adaptation below. Existing external application
UIs (including btop in Kitty) are integration boundaries, not QML components.

## Authoritative design references

Consult the relevant **official** page when implementing a control; retain its
link in the change description or component comment when copying measurements.
Do not label a visually similar custom control a specification-exact component.

- [Material 3](https://m3.material.io/)
- [Expressive motion and springs](https://m3.material.io/styles/motion/overview/how-it-works)
- [Motion specifications](https://m3.material.io/styles/motion/overview/specs)
- [Color roles](https://m3.material.io/styles/color/roles)
- [Typography](https://m3.material.io/styles/typography/overview)
- [Shape](https://m3.material.io/styles/shape/overview)
- [Buttons](https://m3.material.io/components/buttons/overview)
- [Sliders](https://m3.material.io/components/sliders/overview)
- [Switches](https://m3.material.io/components/switch/overview)
- [Lists](https://m3.material.io/components/lists/overview)
- [Menus](https://m3.material.io/components/menus/overview)
- [Dialogs](https://m3.material.io/components/dialogs/overview)
- [Accessibility](https://m3.material.io/foundations/accessible-design/overview)

The current Expressive motion system is spring-based, with spatial and effects
families. Legacy duration/easing tokens are not the entire current specification.
Our existing Bezier curves are desktop approximations, not physical springs.
Some Google pages require JavaScript: if a page cannot be inspected, say so and
verify measurements through Google's official implementation rather than guessing.

### Explicit desktop adaptations

- Logical QML pixels are the desktop counterpart of dp; let Qt/Niri handle scaling.
- No drop shadows or simulated elevation shadows. Tonal surfaces, spacing and
  outlines express hierarchy. Niri window and layer shadows remain off.
- Google Sans Flex is UI type; Google Sans Code is for code/tabular diagnostic text.
- Keep the graphite base with wallpaper-derived Material roles; never substitute
  an arbitrary blue or pink for a generated semantic role.
- Left control center and right monitor use existing mirrored 720px/full-height
  geometry, 12px padding, 14px inner corners and 75% panel opacity. Their existing
  240ms/180ms reversible slide is a desktop adaptation, not a Google spring token.
- Ordinary centered overlays use the common fade/scale presentation. Volume
  values and bar width update immediately; its only animation is a 100ms fade.
- The barless desktop and assigned shortcuts stay intact. No persistent toolbar
  or extra explanatory footer is needed to advertise standard keyboard behavior.
- WASD aliases supplement arrows only outside text editing.

## Architecture and ownership

```
Niri binding -> Foundation Rust command -> shell IPC
  -> shell/shell.qml -> SurfaceRouter -> lazy surface
      -> shared QML controls -> validated Rust requests
      <- backend snapshots/results <- native service or process
```

`SurfaceRouter` arbitrates mutually exclusive launcher, clipboard, Bluetooth,
session and control-center surfaces. Clock/volume are non-modal readouts. The
right btop panel remains an explicitly focused, non-modal external panel.

The composition root owns loader lifetime and target output. A surface owns its
current view, focus bookmark, pending presentation request and close state.
`superseded` hides an outgoing window and releases input without killing its
backend owner. `ModalBackdrop` owns the centered-popup session blur and dim layer independently
of card windows. It remains mapped across handoffs; popup windows use transparent
local scrims and no independent compositor blur. It is input-transparent on the
Top layer beneath Overlay cards, so it cannot blur an incoming card. Only the
incoming content animates. Hidden sidebar exits settle without relying
on a mapped frame clock.
A shared control owns local hover/press/drag state only. Never write backend truth
from a delegate or infer action success from a click.

A view can disappear while an operation finishes. Separate “not visible” from
“safe to destroy.” Retain an owner until the native response is collected or
cancelled. Never launch duplicate device operations on rapid activation.

## Tokens and layout

Use `Material.colors`, `Material.type`, `Material.shape`, `Material.motion` and
`Theme.typography`, `Theme.spacing`, `Theme.timing` rather than local constants.
Rust generates palette roles; QML must not independently tint wallpaper colors.
`theme/current/semantic.json` is the runtime palette, with a static fallback.

| Purpose | Role/token |
| --- | --- |
| Panel/card | `surface_container_low` |
| Filled field | `surface_container_highest` |
| Selected row | `secondary_container` / `on_secondary_container` |
| Primary action/checked switch | `primary` / `on_primary` |
| Main/supporting text | `on_surface` / `on_surface_variant` |
| Divider/subtle outline | `outline_variant` |
| Error | Material error role; meaningful text as well as color |
| UI font | `Theme.typography.family` |
| Large rounded card | `Material.shape.extraLarge` |

Existing type tokens: headline small 24/32, title large 22/28, title medium 16/24,
body large 16/24, body medium 14/20, label large 14/20, label small 11/16.
Do not change per-control type to compensate for a layout error.

Use common 4/8px rhythm, established 12/16/24px content insets and Material
component measurements. Current standard list rows are 56px one-line and 72px
two-line; compact controls have a 48px input target around a 40px visual.
Keep touch/pointer targets larger than small icons. Constrain sliders to a usable
width rather than filling all available space. Clip scroll content, not focus
rings. Size against available output geometry; long labels elide with full
accessible names, and long lists scroll instead of extending offscreen.

### Minimal headers

Centered menus omit generic application titles and item/availability counters.
Search menus begin with the search field; the session menu begins with actions.
Bluetooth keeps its radio toggle without an icon/title heading; device settings keep device identity,
subpage context, battery and codec status. The Control Center overview omits its
redundant title and close-button row; detail pages retain orientation and a back
action. Escape or the panel shortcut closes it. Bluetooth places the radio toggle
to the right of Manage below the device list, without an empty heading band.
Do not remove useful values, errors, safety notices or action labels to achieve
visual minimalism; reclaim the empty header space when deleting text.

## Shared component contract

Use these existing APIs before introducing another control. Extend one common
primitive when multiple callers need the same change.

| Component | Contract |
| --- | --- |
| `SurfaceCard` | Tonal rounded surface, no shadow/offscreen effect layer |
| `FocusIndicator` | Shared primary outline decoration; `cornerRadius`, caller-owned visibility; never captures input |
| `SurfaceAnimation` | Retargetable numeric Behavior animation, common default timing/easing; callers choose spatial/effects tokens where applicable |
| `SidebarMotion` | Reversible position/velocity slide; `move(open)`, `position`, `settled(open)`; reduced motion settles immediately |
| `SurfaceRouter` | `surfaces` adapters (`item/alive/open/dismiss/present`, optional `retiring`; `open(handoff)`), `request(name, showOnly)`; last intent wins; immediate visual handoff with retained backend ownership |
| `ControlAction` | Button/list action: `text/detail/badge/symbol`, `marked/toggle/compact/iconOnly`, `clicked`; checked state is backend-owned |
| `ControlChoice` | Keyboard dropdown: `model/textRole/currentIndex`, `activated`; private `menuIndex` until committed |
| `ControlVolume` | Shared audio slider/mute presentation and directional input; backend callbacks own actual volume |
| `ControlGroup` | Common group surface and layout |
| `ApplicationRow` | Shared menu/device setting row, explicit selected/pending/confirmed/setting states, activation signal |
| `ClipboardRow` | Clipboard representation and pin action; no independent storage or content parsing |
| `SearchInput` | Real text input with selection, caret and keyboard editing; never consume WASD/Space as global navigation |
| `SelectionWheel` | A single selection-list wheel owner; accumulates steps within a gesture, discards residue when disabled/gesture ends |
| `SurfaceFooter` | Hidden unless actionable/status content is necessary; not a permanent instruction legend |
| `WallpaperGrid` | Three-column preview grid and keyboard selection; Rust supplies cached previews and apply results |

Shared dialog/menu/scroll/focus behavior must follow the sections below even
where implemented using Qt Quick Controls and these existing primitives. Do not
create an unused parallel component library or migrate a control by renaming it.

## Keyboard and focus

1. On open, place focus deliberately: search in search surfaces; first useful
   enabled device/action elsewhere. Do not default to Manage over a device.
2. Arrows navigate according to visible geometry. W/A/S/D are aliases outside
   editors. Enter and Space activate a focused action exactly once.
3. In an editor, preserve typing, Space, selection, copy/paste and Ctrl shortcuts.
   Arrow interception must match the editor's documented list-search behavior.
   Never install a parent `Keys.BeforeItem` WASD handler around arbitrary editors.
4. A horizontal slider uses Left/Right or A/D to adjust; Up/Down or W/S leave the
   slider. Tab/Shift+Tab must always offer an escape route. Space must not also
   trigger mute/a parent action when adjusting the slider.
5. Menus move a provisional selection; activation commits; Escape cancels and
   restores the invoking control. Clamp at boundaries, no unintended wrap.
6. Escape returns one internal level first, then dismisses the window. Ignore
   auto-repeat for activation, dismissal, destructive actions and toggles.
7. Save focus by stable device/action/item identity, not a mutable list index.
   On return, restore that identity; if removed, choose the nearest valid action.
8. Every interactive control must be reachable through Tab and a logical spatial
   route, including pin, clear, cancel, dropdowns and secondary actions. Hidden
   and disabled controls must not receive navigation focus.

Use a common primary-colored visible keyboard focus indicator and Material state
layers. Focus, selected, checked, pending and hovered are different states. A
checked setting needs a switch/check/value marker, not color alone. Icon actions
need accessible names. Readouts must not steal keyboard focus.

## Pointer, wheel and scroll ownership

One event has one owner. A selection list consumes wheel input **only in its
bounds**; its enclosing card must not install another handler for the same list.
Headers, switches and unrelated controls must not navigate a hidden list. A wheel
step changes selection/scroll, never a setting value without explicit adjustment.

Discard partial gesture accumulation when a handler is disabled or its gesture
ends; never carry fractional wheel input into another view. Ignore horizontal
wheel input when only vertical selection is supported. Nested scrolling must
have a declared boundary policy: consume inside the active list, never forward
to an unrelated sibling. Qt Quick controls retain their own popup/drag ownership.

Moving content under a stationary pointer must not replace keyboard/wheel
selection through a newly created hovered delegate. Enable pointer selection on
actual pointer movement; reset that guard when entering/restoring a view. A drag
grabs its initiating control through release/cancel. Releasing over another row
must not activate that row.

Bluetooth/Nothing controls previously had both card-wide and list wheel handlers;
only the list should own selection scrolling. Keep that regression covered.

## Motion and lifecycle

States are hidden -> opening -> open -> closing -> hidden. Internal navigation
has one current view and one destination; new input replaces an obsolete intent.

- Input works during opening. Never wait for animation completion to dispatch a
  legitimate control action.
- Same shortcut during opening requests close. During closing it reopens from
  the current rendered value. Holding the shortcut must not synthesize toggles;
  Niri bindings use `repeat=false`.
- Different modal requests replace the visible/input-owning surface immediately.
  Hide the outgoing window before opening the next; do not chain its full exit
  and the incoming entrance. Retain hidden owners only until their backend work
  and cleanup finish. A requested target still shutting down reopens after its
  own shutdown. Rapid requests select the most recent destination.
- A busy session action cannot be replaced. Do not cancel system operations by
  destroying their process owner. Normal transitions must not lock input globally.
- Use `Behavior`/`SurfaceAnimation` retargeting. Do not restart an animation with
  `from: 0` while the current opacity is 0.6. Never unload before exit completes.
- Use a single close completion owner. Stop obsolete close timers on reopening.
  Do not leave a timer that later destroys a reopened view.
- Outgoing/incoming internal views must not both accept input. Prefer staged
  replacement or one explicitly owned transition, never independent overlapping
  fades plus independently resized cards.
- Changes to status/value are immediate. Do not animate volume/progress values
  that must follow keyboard repeat. Animate emphasis/presentation separately.

`Theme.timing` holds desktop presentation durations; `Material.motion` holds
component approximation durations. `Theme.reducedMotion` reduces both to zero.
The native `ui-preferences` command reads `enable-animations` on demand; the shell
refreshes it at startup and on surface requests, without a preference-watcher
process. `DF_REDUCED_MOTION=1` is an explicit override. Unknown preference reads
retain the last known value. Sidebar motion settles immediately in this mode.
No replacement action/confirmation is skipped merely because its animation is off.

The external Kitty monitor has native Rust motion; keep its behavior aligned with
shared sidebar settings without reimplementing btop in QML. Its independent
renderer is an explicit boundary, not proof that QML reduced motion covers it.

## Backend requests and changing state

Rust validates command arguments, device identity and capabilities. QML tracks
pending action IDs and displays the confirmed result. Do not optimistically mark
a toggle successful until backend confirmation, unless rollback is explicit and
tested. Errors keep a meaningful retry path and are not swallowed by a fade.

Use stable IDs, bounded queues and one owner per response. Discard stale readbacks
by revision/request ID. Loading must reserve stable layout and commit coherent
snapshots; do not successively reveal partially loaded setting rows. Disconnects
must disable invalid controls, cancel stale presentation and preserve a useful
back/close path. A removed device must not make an old index target another device.

Backends should be on demand; lazy surfaces must release polling/processes when
closed, except an already-owned request finishing safely. No new permanent daemon
for UI convenience. Color mapping, file parsing, system calls and persistence
belong in Rust; presentation-only navigation can remain in QML.

## Window behavior and states

- Centered menus: deliberate target output, constrained size, scrim, outside click
  dismisses, Escape returns/closes; no window shadows.
- Sidebars: full-height flush edge, shared geometry, on-demand layer-shell focus;
  other windows remain usable when the panel is not explicitly focused.
- Dropdown/dialog: close/cancel restores invoking focus; underlying input blocked
  only for the duration of that modal view.
- Loading: stable geometry and a quiet status, no fake selectable placeholders.
- Empty: one useful explanation; keep navigation/close available.
- Error: readable cause and appropriate retry/back; do not hide it in secondary
  debug text or clear it before it can be understood.
- Disconnected/unavailable: show confirmed status and disable invalid actions.
- Busy: feedback on the affected control; do not freeze unrelated navigation.

## Implementation examples

Correct: `color: Material.colors.surface_container_low` with `SurfaceCard`.
Incorrect: a local blue-gray literal and a new `MultiEffect` drop shadow.

Correct: `Behavior on opacity { SurfaceAnimation { duration: Theme.timing.exit } }`
with an owned close completion cancelled by `present()`.
Incorrect: delayed `Qt.callLater`/Timer destroys the loader regardless of reopen.

Correct: list owns `SelectionWheel { onStepped: delta => navigate(delta) }`.
Incorrect: identical handlers on both the list and its full card.

Correct: a row sends a stable device ID to Rust and reflects the result.
Incorrect: a delegate writes a settings file or shells out using label text.

Correct: WASD aliases in a non-text menu, Enter/Space activate only the focused
control. Incorrect: global Space/WASD handlers consume search text.

## Verification and change checklist

Use production components in an offscreen Quickshell host, replacing only Wayland
transport and real backend actions. Never test actual power/session actions,
updates, Bluetooth reconnections, or clearing the user's clipboard history.
No new Python tests. Rust tests own backend behavior and QML fixtures own input.

For each affected surface verify:

- [ ] Shared tokens/components; no shadows or local palette substitutes.
- [ ] Open/close/reopen before completion; rapid different-surface requests.
- [ ] One visible modal destination and no obsolete unload callbacks.
- [ ] Keyboard-only primary and secondary actions; editor exceptions.
- [ ] Focus entry, submenu return, removed selected item and empty list.
- [ ] Wheel over list vs header/sibling; nested scroll; partial gestures.
- [ ] Pointer movement vs content moving beneath a stationary pointer.
- [ ] Pending action, backend failure, disconnect and stale response.
- [ ] Reduced motion: no animation delay, no lost completion signal.
- [ ] Large text, long names, output constraints, accessible labels/state.
- [ ] No idle transition timer or unnecessary process when closed.

Run `tests/test-material-widgets`, relevant Rust tests, QML lint/format and changed
script checks. `scripts/check` covers repository formatting/native validation.
A component fixture does not certify all window-manager behavior. Report coverage
and remaining gaps precisely; never document planned behavior as implemented.
