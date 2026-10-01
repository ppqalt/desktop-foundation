# Publication preparation

The checkout has no Git remote configured. Publishing/pushing is a separate action;
installation documentation uses a cloned checkout without inventing a GitHub URL.
After choosing repository owner/name, add its actual clone URL to the README.

No top-level source-code license has been chosen by the owner yet. Choose one
before advertising this as an open-source project. Font families retain their
upstream OFL/trademark notices; see `fonts/sources.json`. Optional app installers
pin external downloads and do not commit Spotify client binaries or account data.

`wallpapers/wallhaven-135w7w.png` is the wallpaper supplied by the user. Its upstream
redistribution license has not been established; do not imply a code license covers
that image. Before public publication, establish redistribution permission or
replace the bundled wallpaper with a redistributable asset. The user-facing path
remains easy to swap in `compositor/niri/wallpaper.toml` and deployment.

Ignored `work/` contains local validation artifacts and private captures; it is not
published. Fontconfig `.uuid` metadata is ignored. Historical validation docs refer
to the original machine; reusable runtime/install code uses discovered paths.

GitHub Actions runs portable checks. Native Niri config, live shell/keyboard/portal
behavior, graphics effects and fresh reboot acceptance require the documented
Arch/CachyOS checks. A green hosted job alone does not certify a working desktop.
