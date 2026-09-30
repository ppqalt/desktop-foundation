# Niri migration validation — 2026-09-30

Installed/live Niri 26.04 on Tops is now the primary/reference compositor.
Niri configuration is deployed and native KDL validates for Tops, default and a
missing host profile. Hyprland configs still validate for the same profiles.
COSMIC and Hyprland packages/configuration remain present; no session restart was
forced. Required packages were already installed. PipeWire Pulse was started.
GNOME/GTK portals, packaged polkit, clipboard watchers and one shell instance are
healthy. The stock Niri session owns xwayland-satellite.

## Live checks

Temporary uinput events passed through real Kitty and native Niri bindings:
ä, ö, å, punctuation, AltGr €, @, braces/brackets/backslash, and Shift+7 slash.
The live Wayland keymap confirms Finnish. Super+Shift+7 consumes the cheatsheet
reservation without switching or moving to workspace 7. These tests verify client
key processing, not physical keyboard labels.

Actual shortcuts opened Kitty, overview, launcher and clipboard. Direction focus,
column movement, width cycling, centering, maximize, floating and fullscreen were
exercised with owned real windows. The remaining single column measured midpoint
959.5 on a 1920 px output after opening, closing the second column, floating and
fullscreen round trips, and workspace switching. Multiple columns resumed scrolling.
These are measured dimensions, not hardcoded output configuration.

The shared launcher query worked and Escape destroyed its lazy surface. Clipboard
selection copied the exact existing history payload and closed the surface.
No private clipboard payloads are included in reports. Native window screenshots,
actual Print output and region confirm/cancel passed; saved PNG equalled clipboard
bytes, cancellation made no file. Region confirm used the native UI's selection;
an arbitrary mouse-drag region was not separately automated in this pass.

One shell remained after repeated startup. Stopping it did not disable Super+Enter
Kitty recovery. A separate supervised SIGKILL test confirmed one-second restart,
all existing application windows surviving, adapter readiness and unsupported/
unavailable backend errors. User focus and primary clipboard format were restored;
temporary application/background processes were removed. Closed shell surfaces
leave no permanent UI.

Brave and ChatGPT were already running and were observed operating normally in the
Niri session. ChatGPT is XWayland via native satellite. Existing user windows were
not restarted merely to claim a launch test. Their visual validation against the
user's wallpaper is a subsequent pass.

## Actual blur check

A temporary background checkerboard and an actually opaque native Qt application
at whole-window opacity .90 proved the effect. With blur disabled, a wallpaper
edge changed red by 10 between adjacent pixels; with blur enabled the maximum
change was 1, while the full tonal range remained 10. Thus actual window opacity
and native blur both work. The temporary checkerboard was removed afterward.
Main windows previously sat over uniform gray, which cannot visibly demonstrate
blur. Popup blur is intentionally off; popup translucency can show sharp content.
Kitty uses native .90 background alpha and compositor opacity 1 for opaque glyphs.
Active/inactive normal content opacity is equal; only border/depth denotes focus.

## Verification and limits

Static checks and all three native config variants pass; all 13 existing unit tests
pass using the bundled Node executable. The system Node executable currently has
an unrelated missing libsimdjson.so.33 dependency. No system ABI symlink was added.
Deployment recovery tests cover interruption and foreign replacements. Niri IPC
has no authoritative fullscreen/maximized flag; normalized values remain null and
idempotent setters advertise unsupported. Native toggles and column maximize work.
Tiled viewport coordinates are similarly nullable. Fullscreen opacity is native
Niri behavior rather than a ported Hyprland fullscreen-state selector.

No full login/logout, multi-output/GPU, physical keyboard, arbitrary region mouse
drag or sustained compositor socket-loss test is claimed. Hyprland was parsed,
not live-retested while Niri owned the seat. No new OSD, menu, bar or control UI.
