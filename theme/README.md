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
changes for rollback; Rust also owns theme staging, validation, publication and rollback.

Background, elevated, icon, hover, selected and border roles use the wallpaper
primary's hue at fixed graphite lightness. Neutral bases remove the old blue cast;
surface saturation is capped at 18–25%. Wallpaper brightness cannot brighten the
panels. Foreground and muted text stay near-neutral; the shell adds no shadows.
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
The hue-based Material palette uses the `graphite-v3-material` cache namespace;
older revisions remain available for rollback. Rendered application assets carry
a separate renderer version, so an unchanged cached wallpaper palette does not
keep obsolete adapter output after an update.

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
scripts/brave-theme-reload --configure-root "$HOME/.config/BraveSoftware/Brave-Origin"
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

## Terminal application colours

ANSI palette colours now follow the wallpaper too, so applications using the
terminal's 16-colour palette inherit the theme without application-specific edits.
Chromatic slots retain their red/green/blue identities: Foundation shifts HLS hue
at most 15 degrees toward the accent, then checks 4.5:1 text contrast against the
terminal background. Neutral accents leave those hues unchanged. Black remains a
dark surface slot; bright black follows muted text, and white follows foreground.
This is a compact Foundation adaptation, not Google's HCT harmonization algorithm.
True-colour applications still own their explicit colours. Reset preserves the
original fallback terminal palette exactly.

## GTK and legacy desktop migration

Theme revisions now include GTK3 named colours and GTK4/libadwaita CSS variables.
Accent/destructive background and foreground roles are paired; window, view,
header, sidebar, card and popup surfaces use the existing graphite semantics.
The adapter changes colours, not widget layout, fonts or control behavior. GTK3
receives no GTK4-only syntax. Modern GTK4/libadwaita uses native CSS properties;
[libadwaita's colour-role reference](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/css-variables.html)
and [GTK3's CSS reference](https://docs.gtk.org/gtk3/css-overview.html) describe those interfaces.

The native installer/updater adopts missing GTK CSS files or generated files with
identified Ryoku/Inir palette headers. It preserves unrecognized custom CSS and
unowned symlinks. Adopted originals enter the normal deployment backup journal,
so rollback restores the original files/inodes. Once owned, external replacements
retain the usual deployment-conflict protection.

On Lucky38, the identified `environment.d/ryoku-session.conf` is replaced by an
inactive compatibility file. Its remaining preferences move into
`session-desktop-foundation.conf`: custom editor, cursor, Qt and renderer settings stay,
while Ryoku-only policy-agent flags and the exact obsolete QML module path go.
Existing Foundation environment overrides retain precedence. Both original files
are backed up, dry-run writes no generated source, and subsequent updates preserve
the migrated preferences. Session environment changes take effect at next login;
there is no forced logout or import into the running session.

GTK's default icon theme is Adwaita, with an explicit core package dependency,
instead of requiring the old `ryoku-folders` overlay. SDDM is not migrated or
modified. Application CSS is guaranteed at next application launch; the existing
native GNOME accent refresh remains available, but this does not promise that
every running GTK application reloads its entire CSS on a revision switch.

## Qt and KDE applications

Foundation generates one dual-format colour scheme: complete active, inactive and
disabled Qt palettes (22 QPalette roles) plus KDE colour groups. It uses the same
graphite semantics as the shell/GTK adapters. Highlight foreground/background
remain paired; BrightText stays readable instead of borrowing an on-accent colour.

Clean installations include qt6ct. The generated Niri environment defaults to
`QT_QPA_PLATFORMTHEME=qt6ct`; an existing non-empty platform-theme choice is
preserved. Session startup forwards that choice to D-Bus/systemd services so
launcher applications receive the same backend as Niri-spawned apps. The installer
initializes missing Qt/KDE configuration, adopts an
unconfigured KDE palette, and migrates identified Ryoku/Inir selections. Explicit
custom colour schemes and unknown symlinks remain outside automatic adoption.
Style engines, icon themes, fonts, shortcuts and unrelated preferences are retained.

Qt assets use a separate private bundle under
`$XDG_STATE_HOME/desktop-foundation/qt/revisions/`. An atomic `qt/current` pointer
keeps the selected palette and configuration together. Failed theme activation
restores the previous Qt pointer. A colour rollback generates a new Qt bundle
from the requested colours and current application preferences, so it does not
restore obsolete font/icon choices. Preparation rejects concurrent configuration
changes; marked bundle retention is bounded, and missing palette assets prevent
the unchanged-theme fast path from silently skipping repair.

The existing deployment journal backs up adopted `qt6ct.conf`/`kdeglobals` files
and installs native palette entries into the Qt6ct picker and XDG KDE colour-scheme
catalogue. The Qt6ct appearance picker can select `DesktopFoundation` directly.
Rollback/uninstall restores original file content and identity. No application is
force-restarted; Qt/KDE colours are reliable at the next application launch.
Applications using independent palettes can retain those. SDDM is outside this
adapter and is never changed.

References: [Qt QPalette roles](https://doc.qt.io/qt-6/qpalette.html),
[qt6ct's loader](https://github.com/trialuser02/qt6ct/blob/master/src/qt6ct-common/qt6ct.cpp),
[KDE colour scheme implementation](https://github.com/KDE/kcolorscheme/blob/master/src/kcolorscheme.cpp).

The theme reload binding also accepts `Brave-Origin-Nightly` explicitly; stable
and Nightly retain independent browser roots. Managed BrowserThemeColor policy
uses the upstream shared Brave policy directory and refreshes either running
channel without opening a browser.
