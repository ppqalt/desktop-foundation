# Niri-native screenshots

Niri is the default backend. Super+Shift+S opens native region selection; Escape
cancels and Enter confirms. Print captures the current output. The adapter exposes
screenshotRegion, screenshotWindow and screenshotOutput directly through Niri IPC.
Niri owns PNG saving and clipboard copying. The shared driver supports the same
logical actions: `scripts/screenshot --backend niri region|window|output`.

Deployment translates config/screenshots.toml to native screenshot-path. Niri
strftime filenames use seconds (Python %f is omitted); native file permissions and
collision handling belong to Niri, rather than the Hyprland atomic storage driver.
No resident capture worker is added. Hyprland's original grim/slurp pipeline and
its stricter storage behavior remain unchanged below.

# On-demand screenshots

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
