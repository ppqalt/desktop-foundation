# Wallpaper colors on graphite

```sh
scripts/wallpaper-set /path/to/image            # wallpaper and colors
scripts/wallpaper-set /path/to/image --mode fit # default: fill
scripts/theme-generate /path/to/image          # preview and cache a palette
scripts/theme-apply                            # colors from the active wallpaper
scripts/theme-apply /path/to/image              # colors only
scripts/theme-reset                            # static graphite palette
scripts/theme-rollback                         # previous wallpaper and colors
scripts/theme-promote                          # save the active palette in Git
```

## Palette

The packaged Matugen executable extracts wallpaper colors once and exits. The
Rust backend hashes the image, runs Matugen, derives semantic graphite roles,
checks contrast and caches the result. Unchanged inputs reuse the cache. Rust
also maps the accent to GNOME's supported enum and records owned preference
changes for rollback; Python keeps installation and theme transaction orchestration.

Background, elevated, icon, hover, selected and border roles use the wallpaper
primary's hue at fixed graphite lightness. Neutral bases remove the old blue cast;
surface saturation is capped at 18–25%. Wallpaper brightness cannot brighten the
panels. Foreground and muted text stay near-neutral; shadows stay black.
Accents use primary and an 80/20 primary/secondary blend. Contrast targets are
7:1 for foreground, 3:1 for muted text and 4.5:1 for accents on opaque panels.

| Role | Blue source | Orange source |
| --- | --- | --- |
| Background | `#161b20` | `#201916` |
| Elevated | `#212b33` | `#332621` |
| Selected | `#304150` | `#503930` |
| Accent | `#98ccf9` | `#ffb599` |
| Foreground | `#eef1f6` | `#eef1f6` |

`theme/generated.json` is the shipped palette. `theme/fallback/` contains the
static graphite palette and terminal/notification inputs used by reset.

## Runtime state

`compositor/niri/wallpaper.toml` supplies the installation default. Once a runtime
revision exists, its `wallpaper.toml` supplies the active wallpaper. Runtime
changes leave the repository default unchanged.

Theme state lives under `$XDG_STATE_HOME/desktop-foundation/theme`, defaulting to
`~/.local/state/desktop-foundation/theme`:

- `revisions/<id>/` holds a complete palette, copied wallpaper, blurred overview,
  compositor colors, terminal configuration and application adapters.
- `current` and `previous` point to the active and rollback revisions.
- `brave/` is the stable unpacked-theme directory.
- `profile.json` records the deployed hardware profile.

Images, renderers and native configuration are checked before the active pointer
changes. A generation or staging failure keeps the last working revision. A
required reload failure restores the previous outputs and retries their reloads.
`theme-rollback` switches to the previous complete revision.

Deployment renders the active palette with updated repository renderers while
keeping the chosen wallpaper. `theme-promote` saves its semantic palette to the
tracked `theme/generated.json`; changing the shipped wallpaper is a separate edit.

Cache maintenance retains six theme revisions, 32 palettes and 16 overview images,
protecting current and previous references. Use `scripts/foundation cache plan`
to inspect candidates or `scripts/foundation cache prune` to remove eligible files.
The hue-based palette uses the `graphite-v2` cache namespace; existing theme
revisions and `graphite-v1` palettes remain available for rollback.

## Refresh

| Component | Refresh behavior |
| --- | --- |
| Shell menus, volume and overview | Theme reload through shell IPC |
| Niri | Native configuration reload |
| Wallpaper | Restart the swaybg session service |
| Kitty | Native configuration reload via SIGUSR1 |
| Fish | New shell, or source `~/.config/fish/theme.fish` |
| Fastfetch | Next invocation |
| Mako | Native reload, or next launch |
| GTK | Native dark preference and nearest supported accent |
| Spotify | Refresh generated assets; native UI reload or next launch |
| Brave | Managed BrowserThemeColor policy and native live refresh; unpacked fallback |
| btop sidebar | Owned panel reload and native btop theme refresh while open |

## btop

New theme revisions include `btop/desktop-foundation.theme`, derived from the same
semantic colors as the shell and terminal. Background, text, borders, selection
and graph colors use graphite roles; high temperature and resource usage use the
error color. Reset renders from the static palette. The file is staged before the
active pointer changes, so failed generation preserves the working revision.

Super+X opens the [btop sidebar](../docs/monitor.md). Its isolated configuration
uses this theme; normal `~/.config/btop` settings are preserved. Theme updates
refresh only the owned sidebar process. The [sidebar comparison](../docs/sidebars.md)
describes the host and animation choices.

## Brave

The full installer uses a root-owned `BrowserThemeColor` policy and Brave Origin
Nightly's native policy refresh. It updates a single validated color while
preserving active browser preferences and the previously selected extension
theme. See [managed browser colors](../docs/browser-policy.md) for setup, scope,
verification and restoration.

Without the policy writer, use the unpacked theme fallback below. Policy mode
does not use the DevTools adapter.

Open `brave://extensions`, enable Developer mode and use **Load unpacked** with:

```text
~/.local/state/desktop-foundation/theme/brave
```

Keep this path stable so palette changes retain the same theme identity.

For approved DevTools refresh in Brave Origin Nightly:

```sh
scripts/brave-theme-reload --configure-root "$HOME/.config/BraveSoftware/Brave-Origin-Nightly"
```

Enable **Allow remote debugging for this browser instance** at
`brave://inspect/#remote-debugging`. Wallpaper changes then request a native
connection approval. The finite helper uses the CDP Extensions domain to reload
the installed theme, verifies its identity and version, and disconnects. Each
connection requires Brave's approval.

To disable the adapter:

```sh
scripts/brave-theme-reload --disable
```

Also disable remote debugging in Brave. Without the opt-in, reload the same
folder through **Load unpacked** after a palette change. Rollback restores the
stable manifest and uses the same refresh method.

## Spotify

The DesktopFoundation scheme maps background/sidebar/player to graphite,
card to elevated, selection to selected and buttons to accent. Refresh uses
Spicetify's `refresh --no-restart`; a running UI picks it up through its native
reload, usually Ctrl+Shift+R. See [Spotify setup](../apps/spotify/README.md).

References: [Matugen](https://github.com/InioX/matugen),
[Chromium themes](https://developer.chrome.com/docs/extensions/develop/ui/themes),
[CDP Extensions](https://chromedevtools.github.io/devtools-protocol/tot/Extensions/).
