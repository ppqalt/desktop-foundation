# Deployment and rollback

`scripts/deploy --compositor niri --profile default` validates and deploys the
portable foundation. Optional host profiles contain output/hardware settings;
input remains shared. Hyprland can still be selected explicitly. Deployment does
not switch the running compositor or restart the login manager.

The ownership journal lives under `$XDG_STATE_HOME/desktop-foundation`. Existing
files, symlinks and owned directories are retained as backups before replacement.
Kitty, Fish and Fastfetch configuration directories are owned as complete units;
Niri owns its config link, Hyprland its entry file, and Quickshell the foundation
config link. Unrelated configuration outside those paths remains untouched.
Externally replaced managed paths and a different checkout owner stop deployment.
Never discard the manifest/backups or move a deployed checkout without restoring.

`scripts/uninstall` restores journaled user configuration and owned native/MIME
settings. MIME restoration preserves unrelated later associations. Packages,
application account data and libraries are retained. Separate privileged boot,
AppArmor and greetd helpers keep their own rollback journals; service enablement
is retained intentionally. Review their documented restore commands when removing
system integration. No automatic reboot or login-manager restart occurs.

Runtime wallpaper/theme revisions are separate from source deployment. Normal
`scripts/wallpaper-set` operations do not dirty the checkout. `theme-rollback`
restores a complete prior wallpaper/palette revision and its managed adapters.

Startup units are generated at deployment. Niri owns its graphical session and
Xwayland satellite; desktop-foundation adds no second satellite or portal daemon.
See [startup architecture](startup.md), [installation](installation.md) and
[application roles](application-roles.md).
