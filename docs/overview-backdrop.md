# Niri overview backdrop

The normal wallpaper remains sharp, owned by the existing single swaybg service.
The shell adds one inert background-layer surface per connected output, namespace
`desktop-foundation-overview-backdrop`. Niri's generated `place-within-backdrop`
rule places it behind overview workspace previews; the previews are unchanged.
The surface ignores exclusive zones, has an empty input region and no keyboard
focus. It is loaded only for the Niri backend.

Native `background-effect { blur true; xray true; }` was tried first. On the
installed build the surface showed gray rather than the underlying swaybg image.
The implementation therefore uses the requested static-image fallback.
`niri_wallpaper.py` prepares a Pillow Gaussian blur (24px), then execs swaybg as
before. A 15% black QML tint separates the previews from the backdrop. No runtime
QML blur, overview watcher, polling daemon or second swaybg was added.

The source and scaling mode come from `compositor/niri/wallpaper.toml`. Cache files
live in `$XDG_CACHE_HOME/desktop-foundation/overview` (default `~/.cache`). Image
path, nanosecond mtime, size and processing version determine cache invalidation.
Fill/fit images are capped at 1920px on the longest edge, preserving aspect ratio;
center/tile retain original dimensions. The manifest is atomically replaced and
old cached images removed. To change wallpaper, edit the TOML and restart
`desktop-foundation-wallpaper.service`. The shell watches the manifest; a bounded
five-second startup retry handles parallel wallpaper/shell startup.

`python-pillow` is included in the core installation manifest. Cache files are
regenerable and stay outside Git. Rollback uses normal deployment backups; removing
the backdrop loader and generated rule restores the plain overview backdrop.

## Validation on Tops

- Generated Niri config and repository checks passed (including nine Rust tests).
- Live screenshots showed sharp normal desktop wallpaper, full-output blurred
  overview surroundings and sharp workspace previews; no seams on the connected
  1920x1080 output. Multiple-output behavior is structurally per-screen but has not
  been physically tested with a second display.
- Overview workspace navigation worked; launcher and clipboard reopened with
  input focus and no errors. The backdrop has no input region; Niri reports no
  keyboard interactivity. Super+Tab binding remains unchanged.
- Cache reuse retained the image mtime; mode changes updated the manifest;
  source changes generated a new image and removed the old cache.
- Wallpaper service restart succeeded; exactly one swaybg remained.
- Live shell idle measurement: 0.1% of one core over ten seconds. PSS before the
  addition was 222446KiB and afterward 225657KiB: approximately +3.1MiB. This is
  a same-process hot-reload observation, not an isolated allocation measurement;
  Qt caches and reload activity can affect it.
- Screenshot capture continued to work. Desktop/window/popup blur rules were not
  modified. Temporary captures stayed outside the repository.

Native reference:
https://niri-wm.github.io/niri/Configuration%3A-Layer-Rules.html
