# Spotify, Spicetify and Marketplace

Optional, user-local application setup. This is separate from foundation config
deployment and is never silently installed when deploying the desktop.

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
Spicetify's first-run instructions require. Then close Spotify and run:

    scripts/spotify-setup apply

This uses Spicetify's native backup/apply mechanism. Marketplace is registered as
a custom app with its required upstream placeholder theme. No decorative theme,
extensions or snippets are selected.
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
