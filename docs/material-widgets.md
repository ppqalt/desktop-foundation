# Material widgets

Launcher, clipboard, Bluetooth, Nothing controls, session menu, clock and volume
share the Control Center's wallpaper-derived Material color roles and Google
Sans Flex. Rust continues to own clipboard, audio, Bluetooth and session actions;
QML owns presentation and input.

The shared dialog card uses a 28 px corner and `surface_container_low`, without
a decorative outline. Lists use 72 px rows, 16 px main text and 14 px supporting
text. Selected rows pair `secondary_container` with `on_secondary_container`.
Nothing's switches use 52 × 32 px tracks and distinct thumb sizes/positions.
Session actions use Material Symbols Rounded. Existing desktop placement,
blur, shadows and opening/closing animations are retained.

Navigation instructions, repeated action labels and empty-state tutorials are
hidden. Errors, connection and battery state, device values, pending operations
and clipboard deletion warnings remain. Pin/plain-copy actions retain tooltips
and accessible labels. Keyboard shortcuts continue to work without visible hints.

The volume OSD uses a speaker/mute symbol and percentage. Values change without
animation; entry and exit remain 100 ms opacity fades.

Run `tests/test-material-widgets` for isolated QML checks. It uses Quickshell's
offscreen host, inert Window transports for the session menu and volume OSD, and
no device/session actions. It checks role bindings, switch state, keyboard search
and session navigation, minimal footers, and immediate volume/mute updates.
Set `DF_MATERIAL_CAPTURE=/absolute/path.png` to save the component preview.

References:
- [Material dialogs](https://m3.material.io/components/dialogs/specs)
- [Material lists](https://m3.material.io/components/lists/specs)
- [Material switches](https://m3.material.io/components/switch/specs)

Additional session symbols come from `google/material-design-icons`, revision
`10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7`, under the existing Apache-2.0 license
in `shell/theme/material-symbols-LICENSE`.
