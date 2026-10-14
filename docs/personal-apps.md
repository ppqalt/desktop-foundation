# Optional applications

`scripts/install-all` adds Spotify, Spicetify, Marketplace, ChatGPT, Steam and Vesktop to
the core desktop. The application manifest is `apps/personal.json`; add-ons are
selected in `config/app-mods.json` through [the app-modification installer](app-modifications.md).

## Brave Origin Nightly

The installer selects Brave Origin Nightly as the browser role. Packages come
from the configured repository or its reviewed AUR recipe. The full installer
adds the root-owned color policy and native live refresh. See
[managed browser colors](browser-policy.md) and [theme controls](../theme/README.md).

`scripts/post-install brave` applies the bundled browser settings through the
browser configuration helper.

## Spotify

The pinned client, Spicetify and Marketplace are installed under the user data
directory. Open Spotify, sign in, leave it open for a minute, quit and reopen.
The managed launcher finishes applying Spicetify and the graphite color scheme.
Use `scripts/post-install spotify` to retry setup.

[Spotify setup](../apps/spotify/README.md) covers refresh and restoration.

## ChatGPT and Steam

ChatGPT uses `chatgpt-desktop-bin` from a configured signed repository. Steam
uses the `steam` package from multilib. Enable the required package source before
running the full installer.

Use `scripts/install-all --check` to inspect the application layer.

The configured [Geode add-on](../apps/geode/README.md) installs when Geometry Dash
is ready in Steam. Use `scripts/install-app-mods install geode` afterward if pending.

## Vesktop

The installer uses `vesktop-bin` from the configured CachyOS repository, with the
upstream-recommended AUR package as a fallback on Arch. Its `vesktop.desktop`
entry is available in the application launcher. Existing installations are
reused. [Vesktop installation](https://vesktop.dev/install/linux/) describes the
native package options.
