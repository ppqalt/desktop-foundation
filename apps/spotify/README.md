# Spotify, Spicetify and Marketplace

Optional, user-local application setup. This is separate from foundation config
deployment. The canonical `scripts/install` includes it by default in the personal
profile; `scripts/install --core` excludes it.

Run `scripts/spotify-setup install`. It downloads the official Spotify Debian
client, official Spicetify CLI release and official Marketplace release using
versions and SHA-256 hashes pinned in sources.json. Spotify lives below the user
XDG data directory and is writable by its owner. No sudo, broad permission changes
or global library overrides are needed. Small missing tray libraries are bundled
privately from pinned official Arch packages; these packages were signature
verified when the source manifest was created. The installer checks pinned hashes.

The installer creates backed-up user-local spotify/spicetify command links and a
Spotify launcher entry. Fish includes the standard user-local bin directory.
Native Wayland is selected by the tracked Spotify wrapper. The library search
path applies to the Spotify child process only. Account/prefs files are runtime
state and are never checked into Git.

Open Spotify, log in normally, and leave it open for at least a minute, as
Spicetify's first-run instructions require. Quit normally: the managed launcher
then runs native backup/apply once without restarting Spotify. Open it again to
use Marketplace and the graphite theme. Already-patched launches exec directly.
If initialization fails, retry with `scripts/post-install spotify`.

This uses Spicetify's native backup/apply mechanism. Marketplace is registered as
a custom app with its required upstream placeholder theme. The repo-owned DesktopFoundation color scheme is selected within that same
Marketplace placeholder theme. Existing CSS, extensions, snippets and custom
apps are retained.
Restart Spotify to use Marketplace. A successful install is not proof that login,
playback or the patched Marketplace UI has been tested.

Re-run apply after supported client updates. Version upgrades are explicit:
update the source pins/checksums and handle the existing installation rather than
silently replacing a patched client. Current pins are x86_64 for Tops/lucky38.
The bundled tray-library versions expect a compatible current Arch/CachyOS base.

`spicetify restore` restores stock Spotify client resources. Separately,
`scripts/spotify-setup restore` restores the previous command/launcher links and
Spicetify configuration. The generated Spicetify state is archived first; client
binaries, download caches and Spotify account data are retained. Both restoration
paths are intentionally separate from `scripts/dev restore`, which restores
foundation desktop/terminal/font configuration.

Sources:
- https://www.spotify.com/download/linux/
- https://spicetify.app/docs/getting-started
- https://github.com/spicetify/marketplace/wiki/Installation

## Wallpaper adapter

Personal installation automatically provisions the pinned Spotify client,
Spicetify 2.45.3 and Marketplace 1.0.11, detects matching repo-owned tools and
configures the color adapter. Existing matching Spicetify upgrades are reused.
Downloads have explicit official URLs and SHA-256 hashes in sources.json.
Marketplace extraction stages a complete validated archive before publication;
unknown/incomplete existing installs fail clearly rather than being overwritten.
First login is the remaining account action: sign in yourself and quit normally.
The managed launcher initializes Spicetify after that first unpatched launch.
Apply uses --no-restart; no account preferences are read by the launcher.

`scripts/wallpaper-set` stages `spotify/color.ini` in each runtime theme revision.
Publication merges only its DesktopFoundation section into
`$XDG_CONFIG_HOME/spicetify/Themes/marketplace/color.ini`, preserving other schemes.
The adapter owns only `color_scheme=DesktopFoundation` and `replace_colors=1`
in config-xpui.ini; their original values are journaled before mutation in
`$XDG_STATE_HOME/desktop-foundation/spotify/theme-binding.json`.
All unrelated configuration lines remain unchanged. current_theme stays marketplace.

Mapping: main/sidebar/player use graphite background; card uses elevated;
selection/tab use selected; text/subtext use foreground/muted; buttons use
accent/accentStrong; error stays semantic red; shadow stays black. The semantic
palette retains contrast checks. No page layout or Marketplace theme ecosystem
is replaced.

Refresh class is NEXT-LAUNCH: `spicetify refresh --no-restart` updates theme
assets, including generated colors.css/user.css/spicetify-config.json, through
the supported CLI, without restarting Spotify or refreshing extensions/custom
apps. It uses the existing theme.js/assets behavior configured in Spicetify.
A running UI needs its native reload (Ctrl+Shift+R), or a later normal launch.
We do not run watch/live-refresh: upstream's watch mode stays resident and can
restart Spotify with a debugging port.

Rollback restores the prior runtime palette, re-merges its scheme and refreshes
assets with no restart. An absent/uninitialized client is optional and leaves
colors staged for post-install; a genuine refresh error uses transaction rollback.
Native UI reload is also needed to see rollback in an already-open client.
Changed owned settings are refused rather than overwritten.

To detach just the color adapter, run `python3 scripts/spotify_theme.py restore`: it
restores original owned values, removes only DesktopFoundation's section, and
retains user edits in other fields/schemes. Full application rollback remains
`scripts/spotify-setup restore` and archives the Spicetify configuration as before.
Neither path deletes Spotify accounts, prefs, cache, playlists or sessions.

Live validation: user confirmed blue configuration loaded normally, then orange
accent/dark surfaces/Marketplace all worked after native reload. Transaction
rollback restored blue palette and verified blue in generated Spotify colors.css.
The running client is not silently restarted; its visual rollback awaits native
reload. Cached wallpaper apply including Spotify refresh was about 0.39s on Tops.

[Native refresh implementation](https://github.com/spicetify/cli/blob/v2.45.3/src/cmd/apply.go)
and [watch/restart behavior](https://github.com/spicetify/cli/blob/v2.45.3/src/cmd/watch.go).
