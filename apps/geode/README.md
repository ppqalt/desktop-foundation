# Geode for Geometry Dash

Install Geometry Dash through Steam and run it once to create its Proton prefix.
Then close the game and run:

```bash
scripts/geode-setup install
scripts/geode-setup status
```

Geode is enabled in `config/app-mods.json`. The full installer applies it when
the game is ready; `scripts/install-app-mods install geode` runs it independently.

The Rust backend locates Steam app 322170 through Protontricks, verifies the game's
executable against the supported release, and downloads the pinned Windows Geode
archive. It verifies SHA-256 before copying the loader files into the game folder.
The version, download and compatibility pin are in `sources.json`.

The installer sets `xinput1_4` to `native,builtin` through Wine's supported,
application-specific registry override using the game's selected Proton runtime.
This is the equivalent of the official Steam launch option:

```text
WINEDLLOVERRIDES="xinput1_4=n,b" %command%
```

Existing launch options can override Wine's registry settings. If you explicitly
disabled `xinput1_4` in Steam, remove that conflicting override or use the official
launch option above.
Geode downloads its resources on the first normal game launch.

Requirements: `curl`, `bsdtar` (Arch package `libarchive`) and `protontricks`.
Install missing tools with `sudo pacman -S --needed curl libarchive protontricks`.
Geode is an optional add-on for an existing Geometry Dash installation. The game
uses its selected Proton runtime; a Geode development SDK is unnecessary.

Use `--game-dir /absolute/path` after an action to select a specific game directory.
The override still uses Steam app 322170's prefix. Keep the game closed while
installing or restoring. Repeating installation verifies the managed files and
keeps the original rollback.

```bash
scripts/geode-setup restore
```

Backups and the transaction are stored under
`$XDG_STATE_HOME/desktop-foundation/geode` (normally `~/.local/state`). Restoration
only changes the loader files and the managed DLL override, and refuses to
overwrite loader files changed afterward. Downloaded mods, Geode settings and
game saves remain in place.

[Official Linux instructions](https://geode-sdk.org/faq#how-do-i-install-geode-on-linux),
[pinned release](https://github.com/geode-sdk/geode/releases/tag/v5.10.1),
[Wine application overrides](https://gitlab.winehq.org/wine/wine/-/wikis/Wine-User%27s-Guide#dll-overrides).
