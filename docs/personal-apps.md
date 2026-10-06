# Optional applications

`scripts/install-all` adds Spotify, Spicetify, Marketplace, ChatGPT and Steam to
the core desktop. The application manifest is `apps/personal.json`.

## Brave Origin Nightly

The installer selects Brave Origin Nightly as the browser role. Packages come
from the configured repository or its reviewed AUR recipe. Load the generated
theme folder through the browser's **Load unpacked** control. See
[theme controls](../theme/README.md).

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
