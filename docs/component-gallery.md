# Component gallery

Run `./scripts/component-gallery` from a Desktop Foundation checkout to open an
on-demand preview of the existing shared controls. It uses the real theme and
components but only local sample state; it does not send audio, recording,
update, power or other system commands. The shell directory is deployed as a
unit, so the gallery source and its component imports accompany the installed
shell. The launcher script remains in the owning checkout.

The initial gallery covers action variants, switch states, the shared slider,
choice, search input and status text. It is not yet a complete gallery: popup,
row, wallpaper and additional controls and light/multiple-palette test cases
still need examples and verification. `tests/test-component-gallery` checks that
this gallery loads in an isolated offscreen Quickshell host; it is not a visual
or accessibility audit.

## Building an interface

Import the existing components and theme, use a semantic role, and keep
confirmed system state in the backend owner. For example:

```qml
import QtQuick
import "../theme"
import "../components"

ControlSwitch {
    text: "Example setting"
    marked: controller.confirmed
    pending: controller.pending
    enabled: controller.available && !pending
    onClicked: controller.requestEnabled(!marked)
}
```

Here `controller` is supplied by the enclosing interface. A click requests a
change; it does not claim the backend accepted one. For the existing API and
keyboard, focus and visual contracts, see [the UI standard](FOUNDATION_UI_STANDARD.md)
and [the current inventory](architecture/design-system-inventory.md). Avoid
adding a second component with equivalent behavior; extend an existing control
and test its callers instead. `FoundationSlider` emits preview requests during a drag and one commit on release;
keyboard steps commit immediately. Changing its value from backend readback does
not emit a request. Existing audio uses `ControlVolume`, which extends this
slider and keeps its existing `moved` handler. Replay duration and cancellation
handling are not implemented yet.

The shared `Material.control` measurements apply
to the migrated action, switch and volume slider; other dimensions and tokens
are still being consolidated.
