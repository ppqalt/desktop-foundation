# Physical screen corners

`ScreenCorners` keeps four small black cutouts at each output's physical corners.
The default radius follows `Material.shape.extraLarge` (28 logical pixels), with Qt handling
output scaling. Override `[screen_corners] enabled` and `radius` in user
`desktop.toml`; see [configuration](configuration.md). The component gallery and desktop render the same independent
`ScreenCornerMask` component; there is no copied gallery drawing.

This follows the approach examined in iNiR's
[ScreenCorners.qml](https://github.com/snowarch/iNiR/blob/main/modules/shared/screenCorners/ScreenCorners.qml)
and [RoundCorner.qml](https://github.com/snowarch/iNiR/blob/main/modules/common/widgets/RoundCorner.qml):
small layer-shell corner surfaces. DF's implementation is independent and uses
a standard cubic quarter-circle construction, retaining DF's existing license.
No iNiR shell, services, configuration or interactive corner actions are installed.

The surfaces have empty input regions, no keyboard focus and no exclusive zone.
They do not cover Niri's hot-corner input. Output Variants create and retire them
on display changes without polling or a new service. They use the Top layer:
normal windows are rounded visually at the physical screen edge, while native
fullscreen content and the secure lock screen cover them. This keeps the
conventional fullscreen/direct-scanout path rather than adding a permanent
Overlay-layer obstruction. Borderless apps that do not request compositor
fullscreen retain the ordinary desktop corner treatment.

This is separate from the compositor's rounded overview workspace previews and
fullscreen-only feathering. The screen corners stay at the physical output edge
while the overview zooms. No individual application window radius or shadow
strength is changed by this component.

`tests/test-screen-corners` renders all four production mask orientations and
checks black outer corners, unobstructed inner pixels and passive focus behavior.
It also loads the production output/corner Variants with only Wayland transport
replaced in an offscreen fixture. `tests/test-component-gallery` checks the shared
example in wide and narrow layouts. These fixtures do not establish physical
pointer passthrough or game scanout; the empty-region/Top-layer contract and live
layer placement must be distinguished from those hardware acceptance checks.
