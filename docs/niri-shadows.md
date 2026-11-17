# Window shadows and focus

## Overview workspace previews

`patches/niri/overview-feather.patch` rounds all workspace previews in overview.
Ordinary and maximized content has a crisp rounded edge with one physical pixel
of antialiasing. Only a preview containing confirmed fullscreen content also
feathers inward from that rounded boundary into its actual overview backdrop.
The policy follows each workspace's displayed scrolling column, independently
of global focus and overview/view movement. A fullscreen request alone does not
activate feathering; the client must commit its fullscreen state.

The default corner radius is 24 logical pixels in the overview; the fullscreen
feather width is 16. Both are capped at a quarter of the preview's shorter side
and follow the existing overview animation, including reversal and reduced motion.
The preview center, full-size desktop, individual window appearance, hit testing
and Ctrl+Scroll remain unchanged. The shared config disables the external
workspace shadow to avoid a rectangular rim. Normal window shadows stay independent.

For fullscreen content, the outer rounding grows by the feather width so the
opaque core keeps rounded corners instead of becoming square as the fade widens.

The mask composites the cached backdrop over the perimeter using the inverse of
the rounded/feathered alpha mask. Overlapping windows therefore remain composed
together, rather than fading or rounding every individual window separately.

Optional managed-compositor settings accept values from 0 to 128:

```kdl
overview {
    workspace-corner-radius 24
    workspace-feather 16
}
```

`workspace-feather 0` disables fullscreen feathering while retaining rounding;
`workspace-corner-radius 0` disables rounding. Setting both to zero disables the
mask. Stock Niri does not accept these extensions. The shared config omits them,
keeping stock recovery valid; defaults live in the patched compositor.

The effect reuses Niri's existing per-output/per-capture-target backdrop cache
and does not add a blur pass, full-workspace offscreen copy, polling or a service.
Damage covers the maximum feather perimeter, including the previous wider mask
when the animation closes. Missing shader/cache preparation falls back to the
existing sharp-edge rendering with the compositor's error log.

The package is 26.04-7. Its build checks include software EGL pixel tests at 100%,
150% and 200% scale with an asymmetric backdrop, window content reaching an edge,
rounded corners, sharp ordinary edges, feathered fullscreen edges and an unchanged
center. The complete compositor library suite runs during packaging. Layout checks
cover open, close, reversal, reduced motion, maximized versus fullscreen state and
client acknowledgement; parser checks cover disabled, custom and invalid settings.
These are actual shader renders, not physical-session visual acceptance.
Install through `scripts/niri-package install --gui-auth`; a normal fresh login
activates new compositor code. Never restart the active session for this test.

## Overview shadow containment

Niri intentionally leaves normal workspace content horizontally unbounded in
its overview. Reusing that bound for window shadows allowed dark rims beyond
the workspace wallpaper. The 26.04-6 update gives static tiled and floating shadow
elements their own finite workspace bound during overview entry, display and
exit. It preserves ordinary window content outside that bound, scrollability,
shadow strength, normal-desktop rendering and existing shadow stacking.

Only shadow elements receive this crop, avoiding Niri's documented problems
when general pixel-shader content is cropped horizontally. The EGL regression
compares shadow-on and shadow-off frames at 100%, 150% and 200% scale and 20%,
50% and 75% zoom: no shadow difference is allowed outside the wallpaper;
inside shadows and out-of-wallpaper window content must both remain present.
Disabling the fix reproduces the outside-pixel failure. Opening/closing and
alpha-animation snapshots remain atomic, retaining their existing renderer path
rather than clipping the application content embedded in those snapshots.

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
The stacking correction first shipped in 26.04-3 and remains in 26.04-7. Existing
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
