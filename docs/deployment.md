# Deployment and rollback

Portable default: `scripts/deploy.py install --compositor niri --profile default`.
For another host use `default` or an optional host profile. Hyprland remains
selectable with `--compositor hyprland`. Selection installs configuration, not a
new running session. Choose the compositor at physical login.

The Niri installer validates native KDL then atomically publishes a wrapper in
`$XDG_STATE_HOME/desktop-foundation/niri.kdl` and links
`$XDG_CONFIG_HOME/niri/config.kdl` to it. Hyprland validates Lua and owns only
`hypr/hyprland.lua`. Both share one `quickshell/desktop-foundation` link. Config
and state default to ~/.config and ~/.local/state. No whole directory is replaced.
Installing another backend preserves the first backend's manifest and original
backups. Config paths modified outside deployment are refused.

Original files/links are renamed into durable backups, with write-ahead manifest,
inode identity and exclusive lock. `scripts/deploy.py restore` restores **all**
foundation-owned config paths, not just one backend. It refuses foreign replacements.
Never delete the manifest/backups or move a deployed checkout without restoring.
Niri watches config changes; `scripts/reload` validates and explicitly reloads the
active compositor. Switching sessions does not require restoring the other config.

`session-start` is invoked at Niri startup, imports native session environment,
starts the packaged polkit agent and runtime-only shell/clipboard services. Run it
manually after deploying into an existing session. Niri manages XWayland through
xwayland-satellite and its systemd graphical session; no extra satellite startup is
added. The stock Niri portal preference selects GNOME and GTK. No portal preference is replaced. Optional login-manager/PAM configuration uses
the default `scripts/install` (or skip with `--no-greeter`) and the separate reversible system installer.

`bootstrap` installs genuinely missing packages using a full pacman upgrade when
needed; it never removes COSMIC/Hyprland. Required Niri packages already existed
on Tops during this migration, so no package installation was necessary.

Startup units are generated at deployment, not login. See [startup architecture and measurements](startup.md).
