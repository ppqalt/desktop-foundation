# Niri screenshots: clipboard only

Super+Shift+S starts a quiet graphite Wayland region selector, with a thin cyan
outline and compact dimensions. Every invocation starts empty: there are no
remembered areas or initial window boxes. Release the drag to copy the selection
immediately; Escape cancels without changing the clipboard. No Enter step,
editing toolbar or resident screenshot process is needed.

The selector is slurp, configured in compositor/niri/screenshots.toml. Grim reads
Niri's Wayland screencopy buffers after the selector closes and sends PNG bytes
through memory to wl-copy. This replaces the built-in selection UI for the
region shortcut and adapter action; it does not patch Niri. The live installed
Niri build was verified to support this capture path. Output geometry and scale
are handled by grim, without machine-specific coordinates.

Print copies the focused screen through Niri's native screenshot-screen action,
with write-to-disk=false. Adapter output/window actions also copy only.
Deployment sets native screenshot-path null, so the optional built-in Niri UI
also does not persist screenshots. No screenshot directory is created by Niri
deployment. The clipboard history can retain copied images as it does for any
other clipboard item; these shortcuts do not create screenshot files.

The common driver exposes region/window/output actions. A runtime advisory lock
ignores duplicate region requests while a selector is open. Cancellation or a
capture failure never publishes clipboard data. The selector is started only on
demand, never at session startup. Hyprland's retained storage policy is documented
below; its screenshot path has not been changed by this Niri-specific request.

Live validation: actual Super+Shift+S drag-release copied a 441 × 321 PNG without
Enter, Escape preserved the previous clipboard, and Print copied 1920 × 1080.
The screenshot directory was unchanged throughout. Previous focus, workspace and
clipboard were restored after testing.

# Retained Hyprland screenshots

Super+Shift+S selects an arbitrary rectangular region with slurp. Escape cancels
without creating an image. Print captures the focused screen/output. Both save a
PNG under ~/Pictures/Screenshots and copy the same PNG bytes to the clipboard.
There are no additional screenshot bindings, editing tools, notifications or
resident capture processes. The normal clipboard watcher may retain the image.

config/screenshots.toml is the single save-directory and filename configuration.
Directories are created automatically after a successful capture. Default names
are Screenshot_YYYY-MM-DD_HH-MM-SS_microseconds.png in local time. Files are 0600.
A private runtime temporary image is validated before publishing a complete file
atomically; collisions never overwrite older images. Cancellation/capture failure
leaves no image, and temporary files are removed. A clipboard failure preserves
the valid saved screenshot and reports its path on stderr. Selection errors are
reported on stderr without requiring a notification daemon. A runtime advisory
lock ignores duplicate capture requests while a selection is in progress.

Common logical actions are screenshotRegion(), screenshotWindow(), and
screenshotOutput(). The shell root delegates to its compositor adapter. The
Hyprland adapter invokes the shared driver with an explicit backend. Direct CLI:

    scripts/screenshot --backend hyprland region
    scripts/screenshot --backend hyprland window
    scripts/screenshot --backend hyprland output

Only scripts/screenshot_backends/hyprland.py knows grim, slurp, hyprctl, monitor
IDs and active-window geometry. Window capture is a screen crop of the visible
focused window on its owning output, so overlapping surfaces are included and
scrolled-off columns are clipped. It does not extract hidden window buffers.
Output and window snapshots are queried once per capture; there is no polling.

Basic live smoke check, without expanding the automated test suite: actual
Super+Shift+S selection and cancellation, Print output capture, direct window
action, valid PNG dimensions, and byte equality with the clipboard. Used an owned
preview window on a temporary workspace; restored the previous workspace/focus
and primary clipboard format. Smoke images are in the screenshot folder.
