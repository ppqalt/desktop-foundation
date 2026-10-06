# Deployment and rollback

Deployment installs configuration from a permanent checkout. Build both release
backends, inspect the target paths, then deploy:

```sh
scripts/build-backend
scripts/build-nothing
scripts/deploy --compositor niri --profile default --dry-run
scripts/deploy --compositor niri --profile default
```

The core installer performs these builds and deployment itself. A direct deploy
requires both executable backends. Select `--compositor hyprland` for the secondary
compositor. Profiles under `profiles/` supply hardware overrides; shared input
remains in `config/input.lua`.

Niri's generated KDL and Hyprland's generated Lua are checked with the selected
compositor before their entry files are published. Deployment also generates
session units, validates Fish configuration, applies theme preferences and refreshes
the font cache. Configuration changes may reload in the running compositor.

## Ownership

The manifest, original backups and generated files live under
`$XDG_STATE_HOME/desktop-foundation`, defaulting to
`~/.local/state/desktop-foundation`.

Managed paths include the selected compositor entry file, the Quickshell link,
session units, GTK settings, fonts and Bluetooth audio configuration. Kitty, Fish
and Fastfetch configuration directories are replaced as complete units. Existing
files, directories and symlinks are retained before replacement.

The manifest records the owning checkout. Keep that checkout and state directory
in place until restoration. A different checkout or externally replaced managed
path is a conflict. Preserve the manifest and original backups while resolving it.

## Restore

```sh
scripts/restore
scripts/uninstall
```

`restore` returns deployment-owned paths to their originals and stops foundation
services. Use it after an interrupted deployment, then deploy again.

`uninstall` also restores recorded native preferences and application-role MIME
keys. Packages remain installed. See [recovery](recovery.md) for system integration
and boot-helper restoration.

Runtime wallpaper and palette revisions live separately from source deployment.
`scripts/wallpaper-set` publishes a runtime revision; `scripts/theme-rollback`
selects the previous revision and updates its managed adapters.

See [installation](installation.md), [session startup](startup.md) and
[application roles](application-roles.md).
