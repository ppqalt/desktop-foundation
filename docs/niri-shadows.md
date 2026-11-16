# Window shadows and focus

## Overview workspace previews

`compositor/niri/config.kdl` independently styles the whole workspace preview's
shadow: softness 80, spread -8, centered offset and black at 16% opacity. Niri
normalizes these dimensions to output height and scales them with overview zoom.
This replaces the default expanded, downward-offset dark rectangle with a wider,
quieter transition into the overview backdrop. Individual window shadows, full-size
wallpaper, workspace content and Ctrl+Scroll behavior remain unchanged.

This is shadow blending, not an alpha feather applied to workspace content: the
wallpaper itself still ends at the workspace bounds. It uses existing compositor
configuration, without another blur pass or a compositor package replacement.

The owner-requested window-shadow experiment keeps outlines off and uses equal
active/inactive black shadows. UI cards and sidebars retain their existing style.
The broad, soft shadow configuration is restored; the narrow-gap workaround was
rejected because it looked harsh.

## Rendering correction

Upstream Niri 26.04 emits each tile's content and shadow together, in focus-first
render order. A wide shadow can therefore land above a neighbouring window's
content; clicking reverses which window receives that halo. Equal shadow colours
alone cannot prevent this.

`patches/niri/shadow-stacking.patch` defers static shadow elements until all
content in the corresponding tiled/floating space has been emitted. Elements
are front-to-back, so this places the shadow plane below that space's windows.
Floating windows still remain above tiled windows, and focus/hit testing still
uses the original window order. Opening/closing snapshots keep their existing
atomic animation path; this change does not split those animations or remove
shadows from individual-window captures.

The package recipe applies this independently of the Ctrl+Scroll overview patch.
Both patches are checksum-verified and contribute to the managed recipe identity.
The managed package release is 26.04-3. Existing signed stock recovery is retained.

## Evidence and activation

A real surfaceless EGL frame comparison on the pinned upstream revision changed
65,366 pixels when only tiled focus changed before the correction, and zero
afterward. Fixed regression cases also cover translucent tiled windows and
adjacent floating windows. The external-shadow pixel count stays 77,193 before
and after; the fix does not obtain equality by deleting shadows.

All 199 Niri library tests and 18 configuration tests passed in isolation. The
three EGL pixel tests also run from the package's check stage using software
rendering, without connecting to the live display or importing session state.

Updating a package cannot replace the code inside the running compositor. A
normal fresh Niri login activates the rendering correction. Do not terminate
the owner's session to validate it. Config reload alone changes shadow styling,
not the compositor's compiled rendering order. These offscreen fixtures do not
claim physical-session acceptance or cover every client-drawn decoration.
