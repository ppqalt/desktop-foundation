# Wallpaper colors on graphite

Run these from the checkout (no sudo):

```sh
scripts/theme-generate                 # JSON preview/cache only; wallpaper.toml
scripts/theme-apply                    # apply palette from wallpaper.toml
scripts/theme-apply /path/to/image      # preview another image's theme, not wallpaper
scripts/theme-reset                    # restore v0.11 static graphite colors
```

`compositor/niri/wallpaper.toml` remains the wallpaper source of truth. Applying a
palette never changes the wallpaper path. Matugen is a packaged core dependency;
no Matugen code or daemon is copied into this repository. The installed 4.x CLI
runs with an isolated empty config, `--dry-run --json hex --mode dark` and source
index zero, so user templates, hooks and wallpaper actions cannot run. Generation
exits after one invocation; cache hits do not invoke Matugen. Cache keys use the
image SHA-256 and mapping policy version in `~/.cache/desktop-foundation/themes`
(or XDG_CACHE_HOME). Missing/failed Matugen, invalid colors or failed validation
leave the current rendered theme intact. Corrupt cache entries are rejected;
delete the affected cached palette to regenerate it.

## Mapping

`theme/generated.json` is the semantic source consumed by QML and translated into
terminal palette and window-border Lua. It records the wallpaper/hash and policy.
Background/elevated/icon/hover/selected/border roles mix the fixed graphite base
with Matugen's dark primary at 6/10/12/12/17/18 percent respectively. Surface HSL
saturation is capped at 20–24 percent. Text remains near-neutral, error colors
remain semantic red, shadows/scrim remain neutral. Accents use primary and an
80/20 primary/secondary blend, lightened only if needed for 4.5:1 contrast.
Foreground is checked at 7:1, muted text at 3:1 against panel/selected surfaces.
These checks are on opaque palette colors; glass readability still depends on
wallpaper, blur and content. ANSI semantic colors stay at their proven static
values rather than converting errors/success into wallpaper colors.

Fallback snapshots preserve original semantic, terminal, Fastfetch and Mako
inputs under `theme/fallback/`. `theme-reset` needs no Matugen. QML also retains
its inline fallback if generated JSON is absent/invalid; compositor Lua has
fallback border values. Do not edit generated outputs independently: adjust the
mapping or fallback source, then apply/reset. Regeneration is expected to modify
tracked generated files, so commit a chosen final palette deliberately.

## Outputs and reload

- Theme.qml watches semantic JSON; launcher, clipboard, Bluetooth, power and
  volume already consume its shared roles. No Quickshell restart is needed.
- terminal/palette.json renders Kitty colors, Fish theme and Fastfetch accents.
  Kitty instances owned by this user reload with their native SIGUSR1 mechanism;
  no remote control is enabled. Fish colors load in new interactive shells;
  existing shells can `source ~/.config/fish/theme.fish` once.
- theme/window-colors.lua is read by config/window-appearance.lua for Niri and
  Hyprland rendering. Current Niri border colors are regenerated preserving its
  deployed hardware profile and other settings, then reloaded via IPC.
- Mako notification colors are rendered from the static layout and reloaded.
  Opacity, blur, spacing, geometry, timing and focus policy are preserved.

All outputs are staged and Niri/Fish validated before replacement. File writes
are atomic per file, under a single process lock; write failures restore previous
files. Semantic JSON is published last. This is not a globally atomic filesystem
transaction across every component, nor crash/power-loss transactional storage.
No user app is restarted and no desktop logout is requested.

For a future wallpaper setter, the integration points are: update the TOML,
restart desktop-foundation-wallpaper.service (updates the cached overview image),
then call theme-apply without a path. No background polling is required.

## Validation on Tops

Real Matugen generation: blue Windows/Tux image 0.235s; orange synthetic image
0.013s. Cached apply with reloads ~0.05–0.06s; already-applied no-op 0.019s. Timings
are observed warm-system values, not cross-machine guarantees. Screenshots of the
live launcher confirmed chromatic changes with graphite surfaces retained:

| Role | Blue wallpaper | Orange test image |
|---|---|---|
| background | #1f262f | #252429 |
| elevated | #2e3845 | #38363c |
| accent | #98ccf9 | #ffb599 |
| selected | #3e5065 | #4d4c57 |
| foreground | #eef1f6 | #eef1f6 |
| muted | #939eae | #939eae |

Static reset reproduced the original terminal palette and Mako styling. Automated
checks cover contrast/saturation across five chromatic primaries, cached reuse,
missing/failed generator, invalid colors and atomic replacement failure. The blue
wallpaper theme is left applied; test images/captures remain outside the repository.
Matugen and the Python pipeline have no resident processes after completion,
therefore zero resident/idle CPU cost from generation. The existing shell gains a
file event watcher, not a polling timer/process. No boot-time theme generation was
added. A fresh installer includes Matugen and uses the committed generated theme.

References: https://github.com/InioX/matugen (packaged generator) and Kitty's
installed native `reload_conf_in_all_kitties` implementation (SIGUSR1 reload).
