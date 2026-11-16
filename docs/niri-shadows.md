# Window shadows and focus

## Overview workspace previews

`patches/niri/overview-feather.patch` blends the entire workspace preview's edge
into its actual overview backdrop. It composites the cached backdrop over the
perimeter using the inverse of a smooth alpha mask. This is equivalent to fading
the composed workspace over that backdrop; overlapping windows remain composed
together, rather than fading every individual window separately.

The default width is 32 logical pixels in the overview, capped at a quarter of
the preview's shorter side. The mask follows the existing overview animation,
including reversal and reduced motion. The preview center, full-size desktop,
individual window appearance, hit testing and Ctrl+Scroll remain unchanged.
The shared config disables the external workspace shadow to avoid a rectangular
rim around the feather. Normal window shadows remain independent.

The compositor accepts `overview { workspace-feather 0; }` to disable it, or a
width up to 128. This is a managed-compositor extension; stock Niri does not
accept that option. The shared config does not emit it, so stock recovery remains
valid. The patch defaults to 32 without requiring a new config node.

The effect reuses Niri's existing per-output/per-capture-target backdrop cache
and does not add a blur pass, full-workspace offscreen copy, polling or a service.
Damage covers the maximum feather perimeter, including the previous wider mask
when the animation closes. Missing shader/cache preparation falls back to the
existing sharp-edge rendering with the compositor's error log.

The package is 26.04-4. Its build checks include software EGL pixel tests at 100%,
150% and 200% scale with an asymmetric backdrop, window content reaching an edge,
all four edges and an unchanged center. Layout checks cover open, close, reversal
and reduced motion; parser checks cover disabled, custom and invalid widths.
These are actual shader renders, not physical-session visual acceptance.
Install through `scripts/niri-package install --gui-auth`; a normal fresh login
activates new compositor code. Never restart the active session for this test.

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
The stacking correction first shipped in 26.04-3 and remains in 26.04-4. Existing
signed stock recovery is retained.

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
