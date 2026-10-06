# Screenshots

Print copies the focused output. Super+Shift+S lets you drag a region and copies
it on release. Niri uses its native capture path. The Hyprland helper also saves
captures in `~/Pictures/Screenshots`.

```sh
scripts/screenshot output
scripts/screenshot region
scripts/screenshot window
```

The Rust backend manages capture, cancellation and clipboard handoff. The
region selector follows the shared font and theme colors.
