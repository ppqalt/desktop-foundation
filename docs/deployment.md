# Deployment and rollback

From the checkout: `scripts/deploy.py install --profile Tops`, then `hyprctl reload`. Use `default` on another host or add a complete host profile. The deployment verifier runs before changing config. Installation modifies exactly:

- `$XDG_CONFIG_HOME/hypr/hyprland.lua` (defaults to ~/.config): a link to the generated wrapper.
- `$XDG_CONFIG_HOME/quickshell/desktop-foundation`: a link to the repository shell.
- `$XDG_STATE_HOME/desktop-foundation` (defaults to ~/.local/state): wrapper, lock, manifest and renamed original-path backups.

No config directory is replaced wholesale. Existing files/links at those two paths are backed up. Repeated installs preserve the original backups; changed deployed paths cause a clear error. Operations serialize with a lock. The backup manifest stays outside Git. `scripts/deploy.py restore` removes only owned links and puts originals back; it refuses conflicting replacement paths. Run `hyprctl reload` afterward. Stop the foundation Quickshell instance with `quickshell kill --path "$PWD/shell"`. Do not move/remove a deployed checkout without restoring first. Changes in linked repository files are live; run checks before editing/deploying.

`scripts/bootstrap` is optional on Tops: it installs missing packages with pacman's full-upgrade operation (avoiding a partial upgrade), selects Rust stable and installs rustfmt, Clippy and rust-analyzer. It needs interactive sudo when packages are missing. It does not deploy configs or touch COSMIC. No packages are removed. It modifies package/toolchain state and downloads Rust tools to the standard rustup directories.

The phase-one deployment/startup does not globally enable polkit, change login-manager configuration, remove COSMIC, change portal preferences, or install new system services. `session-start` exports the active unmanaged Hyprland display/desktop variables to DBus and the user systemd manager (UWSM handles this for managed sessions), then starts the packaged polkit user service and one invisible Quickshell instance. UWSM tracks its child applications and graphical-session lifecycle. The unmanaged path supports temporary testing; use the documented exit binding so its shell/polkit are cleaned up. A killed unmanaged compositor may require manual cleanup of its user services.
