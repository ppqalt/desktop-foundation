# Spotify, Spicetify and Marketplace

The full installer provisions a user-local Spotify client, Spicetify and
Marketplace. Their x86_64 versions, download URLs and SHA-256 hashes are pinned
in `sources.json`.

```sh
scripts/spotify-setup install
scripts/spotify-setup check
```

Spotify lives under the XDG data directory. The managed launcher selects native
Wayland and supplies its bundled tray libraries to the client. The installer
creates command links and a desktop entry.

Open Spotify, sign in and leave it open for a minute. Quit normally, then reopen.
The launcher applies Spicetify after the first launch and selects Marketplace's
DesktopFoundation color scheme. Use `scripts/post-install spotify` to retry.

## Colors and updates

Wallpaper changes publish `spotify/color.ini` and merge the DesktopFoundation
section into Spicetify's Marketplace scheme. Background/sidebar/player use
graphite; cards use elevated; selection uses selected; buttons use the accents.

The adapter refreshes assets with `spicetify refresh --no-restart`. In a running
client, use Spotify's native reload, usually Ctrl+Shift+R, to display the new
colors. The next launch also loads them.

Run `scripts/spotify-setup apply` to reapply a configured installation. Client
and tool upgrades follow the pinned source manifest.

## Restore

- `spicetify restore` restores stock client resources.
- `scripts/spotify-setup restore` restores the prior command links, desktop entry
  and Spicetify configuration.
- `python3 scripts/spotify_theme.py restore` detaches the color adapter and restores
  its original scheme settings.

These application commands are separate from desktop configuration rollback.

References: [Spotify for Linux](https://www.spotify.com/download/linux/),
[Spicetify setup](https://spicetify.app/docs/getting-started),
[Marketplace installation](https://github.com/spicetify/marketplace/wiki/Installation).
