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
| Spotify | NEXT-LAUNCH | native refresh --no-restart; optional native UI reload |
| Brave | RELOADABLE | opt-in approved CDP reload; native manual import fallback |

Wallpaper uses the existing single swaybg service; wallpaper changes restart
that service, not Niri or user applications. The overview reads the matching
blurred image. This shared transaction is ready for a future wallpaper picker.

## Brave installation and limits

Open `brave://extensions`, enable Developer mode, then Load unpacked and select:
`~/.local/state/desktop-foundation/theme/brave` (respect XDG_STATE_HOME if set).
The theme stays in this physical folder: Chromium derives unpacked identity from
its path, so loading a new palette returns the same installed ID. Themes have no
permissions/JavaScript and do not recolor arbitrary web pages.

### Approved DevTools reload

Brave Origin Nightly 154.1.98.33 exposes
`brave://inspect/#remote-debugging` → Allow remote debugging for this browser
instance. This is Chromium's modern approval-based connection, not a launch flag
or unrestricted debugging port. The browser's UI warns that DevTools capability
is broad; our helper restricts its own requests to extension lifecycle operations.
Do not enable it for untrusted local applications.

Opt in explicitly (the root is the actual Origin Nightly user-data directory):

```sh
scripts/brave-theme-reload --configure-root "$HOME/.config/BraveSoftware/Brave-Origin-Nightly"
# Enable the setting in Brave's inspect UI, then:
scripts/wallpaper-set /path/to/image
# Approve Brave's native connection request.
```

The one-shot Node 22+ helper reads ONLY `DevToolsActivePort` discovery metadata,
its own binding and generated manifest. It connects to `127.0.0.1`, requires the
already-installed enabled theme at the exact stable path, calls
`Extensions.getExtensions` → `Extensions.loadUnpacked` →
`Extensions.getExtensions`, verifies unchanged ID and the new version, then closes.
It never requests tabs, attaches to pages, evaluates scripts or reads browser data.
There is no helper daemon, keepalive connection, polling or launch-flag change.
Node is a personal installer dependency; the core desktop does not require it.

On this real session, blue → orange via DevTools changed colors without browser
restart or Load unpacked, preserving ID `jbnhigkccdgpbopbpdobjhahlaaepkog`.
Integrated rollback restored blue through the same adapter and verified version.
**Each new connection required native approval.** Thus reload is automated after
approval, not unattended. Repeated wallpaper changes prompt again; do not bypass
that security boundary to promise zero-click operation. Approval/command timeout
is 45 seconds. Failure causes the existing transaction to restore prior runtime
outputs and retry native reloads; if recovery approval fails, the restored files
remain safe but the browser may need a later explicit reload.

The mutable opt-in binding is `theme/brave-devtools.json` under XDG state; it
contains only the selected browser root. Disable with:

```sh
scripts/brave-theme-reload --disable
# Also turn off Allow remote debugging in Brave's inspect UI.
```

Without opt-in, wallpaper-set still generates the theme safely and reports that
native import/reload is required. With opt-in, unavailable/refused debugging is
an explicit reload failure, never silently reported as applied. The helper can
also be run directly to retry a pending refresh. Rollback restores the stable
manifest first and invokes the same adapter. No profile files, databases, browser
launch flags or browsing state are edited by this integration.

Spotify is now integrated through its existing Marketplace theme color layer.
It is NEXT-LAUNCH, using supported `refresh --no-restart`; native UI reload can
show colors sooner. Personal installation provisions/configures it automatically.
See [Spotify provisioning and ownership](../apps/spotify/README.md). Steam is
investigated but deferred: see [Millennium design](../docs/steam-theme-plan.md).
GTK uses the supported native discrete accent setting; arbitrary toolkit surface recoloring remains outside the adapter.

## Validation

Runtime tests cover runtime staging, rollback, reload failure recovery, invalid
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

## Native GTK accent in v0.12

Supported GNOME/libadwaita `accent-color` values are selected by nearest semantic
accent hue. This uses the native settings interface and the reversible preference
journal; unsupported schemas keep the neutral dark appearance. It does not add
CSS overrides or a watcher. Toolkit and portal support determine live propagation.
See [release validation](../docs/releases/0.12-validation.md).
