# Wallpaper colors on graphite

From the checkout, without sudo:

```sh
scripts/wallpaper-set /path/to/image            # wallpaper + all theme outputs
scripts/wallpaper-set /path/to/image --mode fit # preserve full image; default fill
scripts/theme-generate                        # palette preview/cache only
scripts/theme-apply                            # colors from active wallpaper
scripts/theme-apply /path/to/image              # colors only; keep wallpaper
scripts/theme-reset                            # original v0.11 static graphite
scripts/theme-rollback                         # previous complete wallpaper/theme
scripts/theme-promote                          # deliberately update shipped palette
```

## Ownership and state

Git owns the mapping policy, adapter renderers, fallback snapshots, shipped
`theme/generated.json`, and default `compositor/niri/wallpaper.toml`. The latter
initializes a fresh installation. The effective active wallpaper source of truth
is the same TOML schema in `current/wallpaper.toml`, outside Git; once runtime
state exists, it takes precedence over the shipped default. Trials never edit the
repository TOML. To ship a different default wallpaper, deliberately edit the
repository TOML and include the image in the installation assets.

State defaults to `~/.local/state/desktop-foundation/theme` (XDG_STATE_HOME):

- `revisions/<id>/`: immutable semantic.json, wallpaper.toml, copied sharp image,
  overview.png/backdrop.json, niri.kdl/window-colors.lua, terminal configuration,
  notifications.conf, Brave manifest, metadata.json and adapters.json.
- `current` and `previous`: atomic symlink pointers to complete bundles.
- `brave/manifest.json`: physical stable directory for Chromium's unpacked-theme
  identity; mirrors the published bundle. No browser profile contents live here.
- `profile.json`: deployed hardware profile for Niri rendering.

Caches are under XDG_CACHE_HOME/desktop-foundation: `themes` holds validated
palette results keyed by wallpaper SHA-256/mapping version; `overview` holds
static blurred images keyed by content/scaling/processing version. Old revisions
and caches are retained; there is no background garbage collector. They may be
removed manually while preserving the current and previous revisions.

Deployment re-renders the selected runtime palette with updated repository
renderers, without running Matugen, and preserves the chosen wallpaper. Existing
configuration backups and deployment restore remain available. Promotion writes
only the chosen semantic palette to tracked `theme/generated.json`: review and
commit that deliberate change. Wallpaper asset/default promotion is separate.

## Transaction and fallback

The packaged Matugen executable runs once using an isolated empty configuration,
`--dry-run --json hex --mode dark`, source index zero. No copied Matugen code,
user hooks, polling, theme daemon or boot-time generation is added. Cache hits
skip Matugen; identical palette/image/mode skips publication and reload.

Image verification, palette mapping, blur preparation, adapter rendering, Niri
validation and Fish syntax validation finish in a new bundle before publication.
Missing/failed Matugen, invalid input/colors or staging failure leaves the last
known good bundle active. An atomic `current` pointer publishes complete files.
Native reloads then run sequentially; this is not a frame-atomic change across
independent applications. If a required reload fails, the old pointer and Brave
manifest are restored and previous native reloads retried. Reload recovery errors
are reported, not hidden. This is not a power-loss transaction across services.
`theme-rollback` swaps the previous/current bundles and reloads wallpaper too.

`theme/fallback/` preserves the v0.11 semantic, terminal, Fastfetch and notification
inputs. Reset uses them without Matugen. QML and compositor Lua retain static
fallbacks if runtime palette data is unavailable. ANSI semantic colors remain
static; application data and user settings are not included in theme bundles.

## Graphite mapping

Background/elevated/icon/hover/selected/border mix the fixed graphite base with
Matugen dark primary at 6/10/12/12/17/18 percent. Surface HSL saturation is capped
at 20–24 percent. Foreground/muted remain near-neutral; shadows stay neutral.
Accents use primary and an 80/20 primary/secondary blend with contrast checks:
foreground 7:1, muted 3:1, accent 4.5:1 on opaque panel/selected surfaces.
Actual glass readability still depends on wallpaper and blur.

| Role | Blue Windows/Tux | Orange test |
|---|---|---|
| background | #1f262f | #252429 |
| elevated | #2e3845 | #38363c |
| accent | #98ccf9 | #ffb599 |
| selected | #3e5065 | #4d4c57 |
| foreground | #eef1f6 | #eef1f6 |

## Adapter contract

| Adapter | Mode | Refresh |
|---|---|---|
| Quickshell: launcher, clipboard, Bluetooth, power, volume | LIVE | explicit foundation.reloadTheme IPC; semantic roles |
| Overview | LIVE | same IPC reloads prepared backdrop manifest |
| Niri | RELOADABLE | native load-config-file; existing geometry/motion preserved |
| Kitty | RELOADABLE | native SIGUSR1; no app restart/remote control |
| Fish | NEXT-LAUNCH | new shell, or source ~/.config/fish/theme.fish |
| Fastfetch | NEXT-LAUNCH | next one-shot invocation |
| Mako | RELOADABLE | makoctl reload; absent daemon uses next launch |
| Brave | RELOADABLE | manual native unpacked-theme reload/reinstall |

Wallpaper uses the existing single swaybg service; wallpaper changes restart
that service, not Niri or user applications. The overview reads the matching
blurred image. This shared transaction is ready for a future wallpaper picker.

## Brave installation and limits

Open `brave://extensions`, enable Developer mode, then Load unpacked and select:
`~/.local/state/desktop-foundation/theme/brave` (respect XDG_STATE_HOME if set).
After changing palettes, use native Reload if offered; otherwise load the same
folder again or disable/re-enable the theme. The manifest version changes with
the palette. Do not depend on browser restart alone to rebuild its cached theme.
The physical folder avoids canonicalizing a revision symlink into changing IDs.

The Manifest V3 theme has no permissions, JavaScript, profile access or watcher.
It maps supported frame, toolbar, tabs, icons, omnibox and new-tab colors to
semantic graphite roles. It cannot theme arbitrary web pages or force unsupported
border roles. On Brave Origin Nightly 154.1.98.33, the user installed this stable
folder into their real profile. After a blue-to-orange wallpaper transaction,
Brave remained blue; loading the same folder again applied orange. Thus changing
manifest.json alone is not an automatic refresh mechanism on this session.

The public management API has no reload-from-disk method; enabling/disabling an
extension is not a documented disk reload substitute. Themes contain no code,
so cannot watch files or invoke runtime.reload themselves. Official Chrome
DevTools MCP implements unpacked reload by reinstalling the same path through
CDP's Extensions.loadUnpacked. This is a supported programmatic route, so we do
not claim automatic refresh is fundamentally impossible. It needs an enabled
browser-debugging connection and extension-loading capability. The running
Brave session has no such connected controller. Newer Chrome also offers an
opt-in remote-debugging UI; its Brave/theme-loading coverage has not been
validated here. No broad debugging access, launch flags, profile edits or browser
restart were introduced merely to recolor the browser.

Current contract: wallpaper-set updates the stable manifest; one native Load
unpacked action on that same folder refreshes the running browser. Rollback
restores the manifest from the previous bundle, but likewise requires that native
action before Brave reflects it. The same physical path retains unpacked identity;
no per-wallpaper theme directories are installed into Brave. Only theme runtime
files are written. The normal browsing profile is accessed solely through the
user's native install/reload actions.

This pass stops after Brave. Spotify currently uses Spicetify Marketplace's
special theme; replacing it could disrupt Marketplace theme installation. Next
pass: an explicit adapter preserving Marketplace behavior and native no-restart
refresh. GTK is the next general toolkit target, with dark graphite fallback and
separate validation of GTK3/GTK4/native application coverage.

## Validation

68 Python tests cover runtime staging, rollback, reload failure recovery, invalid
images, no-op reuse, permissionless Brave manifest, deployment migration and
existing regression cases. Native Niri validation/IPC and repository checks run
separately. Blue/orange live wallpaper transactions and rollback were exercised.
Observed warm cached publication/reload was about 0.21s, unchanged apply about
0.008–0.011s. Timings vary by machine/image. No Matugen or theme pipeline process
remains afterward: zero resident generation process and idle CPU cost. Existing
QML file events plus explicit reload are used, without a polling timer.

Primary references: [Matugen](https://github.com/InioX/matugen),
[Chromium themes](https://developer.chrome.com/docs/extensions/develop/ui/themes),
[native unpacked loading](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world),
[Chromium theme lifecycle](https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/themes/theme_service.cc).

Refresh investigation sources:
[public management API](https://developer.chrome.com/docs/extensions/reference/api/management),
[official DevTools reload implementation](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/src/tools/extensions.ts),
[CDP extension loading](https://chromedevtools.github.io/devtools-protocol/tot/Extensions/),
[opt-in running-browser connection](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/advanced-usage.md).
