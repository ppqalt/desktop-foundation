# Application modifications

`config/app-mods.json` enables Spicetify and Geode. The full desktop installer runs
this layer after installing the base applications. Core-only installation keeps
it separate. Override individual entries in
`$XDG_CONFIG_HOME/desktop-foundation/app-mods.json`:

```json
{"geode": {"enabled": false}}
```

```bash
scripts/install-app-mods
scripts/install-app-mods plan
scripts/install-app-mods check --skip-unavailable
scripts/install-app-mods install geode
scripts/install-app-mods restore geode
```

The Rust coordinator calls each add-on's existing setup and rollback adapter.
Explicit names select that add-on; without names, enabled entries are selected.
Disabling an entry skips future installation and does not remove an existing mod.

Spicetify setup includes the project's portable Spotify client, Marketplace and
wallpaper color adapter. First login and a normal quit/reopen complete patching.
Its source versions remain in `apps/spotify/sources.json`.

Geode uses `apps/geode/sources.json` and applies to Steam Geometry Dash after its
first launch. The default installer reports it as pending when the game or prefix
is unavailable. After installing the game, rerun this script. The full installer
provisions Protontricks and libarchive when Geode is enabled; standalone use needs
those tools already installed.

`--no-packages` on the full installer checks existing add-ons instead of downloading
them. Each add-on retains its own backups and restoration rules:
[Spotify](../apps/spotify/README.md), [Geode](../apps/geode/README.md).
