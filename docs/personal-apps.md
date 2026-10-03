# Personal application provisioning

`install-all` reuses the complete core and then runs private provisioning helpers.
There are no separate mandatory per-app installer commands.

## Browser

The role manifest specifies `brave-origin-nightly-bin`, executable
`brave-origin-nightly`, entry `brave-origin-nightly.desktop`. Installed packages
are adopted; missing packages prefer configured signed repositories, then the
verified AUR name through reviewed Paru. An unrecognized existing executable or
expected desktop entry stops replacement.

The installer sets narrow browser MIME roles and generates the stable runtime
`theme/brave/manifest.json`. One native Load unpacked action installs it. Later
manifest changes require native import/reload; a restart alone is not promised to
reread an unpacked theme. Existing profiles, extensions and accounts are not read
or seeded by normal v0.12 convergence. The prior `brave-config` helper remains
explicit opt-in for the tracked allowlisted settings, with its existing backups.

The tested lifecycle-only DevTools helper is retained as explicit opt-in and is
OFF by default. No installer starts a debugging listener, sets browser debug flags,
or reads page/history/cookie/password/session data. See [theme controls](../theme/README.md).

## Spotify

[Spotify sources](../apps/spotify/sources.json) pin official client, Spicetify,
Marketplace and private runtime libraries with SHA-256 checks. User-local installs
and launcher links have ownership metadata/backups; the original account folder
is untouched. Marketplace is staged completely before publication, preserving its
CSS, other schemes, extensions and custom apps.

A known repo-owned Spicetify 2.45.1 tool upgrades to 2.45.3 with a recoverable binary
backup; unknown versions fail rather than being replaced. `spotify-setup check`
checks provenance, tool version, expected spotify_path/prefs_path and Marketplace
without reading account preference contents.

First launch/login remains yours. Keep it open a minute, quit normally and reopen:
the launcher applies Spicetify without restarting Spotify. Already-patched starts
exec directly. Native `refresh --no-restart` updates color assets on wallpaper
changes, but running Spotify needs its native Reload or next launch. No watcher
or automatic debugging is introduced. Recovery: `post-install spotify`.

## ChatGPT

The reference installation was inspected, not guessed: `chatgpt-desktop-bin`
26.930.21537-1, executable `chatgpt`, desktop entry `chatgpt.desktop`, from CachyOS's
signed package repository. `apps/personal.json` records the role/source.
Provisioning uses the configured signed package and its normal package updates;
no separately downloaded or invented unofficial Linux client is substituted.

If that package is unavailable on another Arch-family installation, the full
module reports the exact missing source and stops; the core remains usable.
It does not add CachyOS repositories to plain Arch. Credentials/login/session and
application data remain untouched. The launcher discovers the installed entry.

## Steam and Millennium boundary

Stock `steam` is installed/adopted from configured signed multilib repositories;
no multilib/repository is silently enabled. Steam executable/entry are verified.
Accounts, userdata, game libraries, Cloud, controllers and launch options are not
read or rewritten. Steam login is an ordinary user action.

**Millennium + Material + Steam palette adapter are excluded from v0.12.** The
maintained AUR route exists, but no loader/theme-selection implementation has
passed activation/live acceptance here. No generated Steam colors or live theming
are promised. [The exact investigation/design](steam-theme-plan.md) identifies
package, supported variable contract, configuration/layout uncertainty and the
upstream 1.5-second color timer. The generic installer that forces Steam stable
is not run. This limitation is explicit rather than a half-installed integration.

## Ownership and restoration

Desktop Foundation owns its links, pinned tool prefixes, narrow Marketplace color
section/settings and generated runtime outputs. Normal installation does not own
browser profiles, Spotify account prefs, Steam libraries/userdata or ChatGPT login.
Packages remain installed on `uninstall`; Spotify's optional links/config/tool
upgrade restore uses `spotify-setup restore`, retaining generated/config archives
and downloaded clients. Externally changed owned paths/keys refuse restoration.
