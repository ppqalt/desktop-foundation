# Component gallery

Run `./scripts/component-gallery` from a Desktop Foundation checkout to open an
on-demand preview of the existing shared controls. It uses the real theme and
components but only local sample state; it does not send audio, recording,
update, power or other system commands. The shell directory is deployed as a
unit, so the gallery source and its component imports accompany the installed
shell. The launcher script remains in the owning checkout.

The gallery previews shared action variants, switches, slider, choice, search,
status messages, launcher and clipboard rows, device and network examples, a
shared popup, the four-column wallpaper grid, the replay row, unified volume
rows, range selection, precise time fields and a sample clip timeline. Wallpaper
inventory, previews and selection use a sample-only helper: choosing a tile does
not change the desktop. A section selector provides quick navigation. Keyboard
focus is revealed by the same `FocusViewport` used in Control Center, including
scaled content and resized views. Manual scrolling takes priority until focus
changes again. Control focus rings appear for keyboard navigation, not clicks;
pressed/selected feedback and popup surface outlines remain visible. The popup
example uses the same sized, outlined PopupCard as the desktop. The input-disable
preview covers sliders, range, volume, dropdown
and search controls. Shared editable inputs are Tab-focusable; time fields grow
with text size and use the same centisecond display as the replay editor.
Preview zoom, text size and right-to-left layout are local to the gallery. Zoom
scales the whole composition; the separate 100/125/150/200% text-size selector
changes shared typography while preserving unscaled minimum hit targets. Shared
buttons and dropdowns grow vertically to fit their larger labels. Narrow gallery
labels wrap and controls remain within the available content width. Its recording
example uses local simulated state and cannot start or save a recording. The
palette selector offers the current wallpaper and two sample Matugen palettes
in light and dark variants; changes affect only the gallery process. Reduced
motion can also be toggled locally. The samples in `shell/gallery-palettes.json`
were generated with Matugen's tonal-spot scheme from `#6750a4` and `#006b60`,
at standard contrast; they do not change the desktop or its wallpaper.

It is not yet a complete gallery: all control states, density variants and comprehensive visual and
accessibility comparisons still need examples and verification. The mirrored
preview helps inspect layout but does not establish full right-to-left support.
`tests/test-component-gallery` checks palette selection and representative
control interactions in an isolated offscreen Quickshell host. Set
`DF_GALLERY_CAPTURE=/absolute/path.png` when running that test to save a
screenshot. These checks do not certify compositor behavior or accessibility.

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
