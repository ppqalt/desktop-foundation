# Recovery

Run recovery commands from the checkout that owns the installation. Open a
terminal with the compositor's terminal binding, or sign in on another virtual
terminal with Ctrl+Alt+F2.

## Shell and session services

```sh
scripts/doctor
journalctl --user -b -u desktop-foundation-shell.service
scripts/shell-start
```

The shell service restarts after process failure with a one-second delay and a
limit of three starts per minute. If the limit is reached, inspect and fix the
reported error, then clear the failed state and start it:

```sh
systemctl --user reset-failed desktop-foundation-shell.service
scripts/shell-start
```

`scripts/shell-stop` stops the shell. `scripts/session-start` imports the active
compositor environment and starts the foundation session target. Run that command
inside the active desktop when recovering session services. See
[startup](startup.md) for the service layout.

Session exit uses Niri's native quit action, UWSM stop for a managed Hyprland
session, or explicit service shutdown for direct Hyprland.

## Deployment and preferences

```sh
scripts/restore
scripts/deploy --compositor niri --profile default
```

Restore completes recovery from an interrupted deployment using the saved
manifest and original backups. It also stops foundation services. An externally
changed managed path requires inspection before restoration can continue. Keep
the manifest and backups; deleting them removes the information needed to recover.

Use `scripts/uninstall` to restore managed user configuration, preferences and
application defaults together. `scripts/uninstall --greeter` also restores
journaled greetd/PAM files. Spotify setup has its own
`scripts/spotify-setup restore` command.

## Boot and login files

Boot transactions have root-owned backups under `/var/lib/desktop-foundation`.
For the combined clean-boot installation, restore the layers in reverse order:

```sh
sudo scripts/boot-optimize restore
sudo scripts/greeter-console-setup restore
sudo scripts/boot-setup restore
```

Run only the helpers used for that installation. The console helper restores its
saved generated GRUB menu. After completing restoration, regenerate GRUB with
`sudo grub-mkconfig -o /boot/grub/grub.cfg` if installed kernels changed since the
backup. The optimization helper checks its managed files, including the menu, and
reports later changes for review.

For an interrupted clean-boot transaction, inspect
`sudo scripts/boot-setup status`. Use `resume` when its saved rollback artifacts
are intact, or `restore` to recover the saved configuration, packages and boot
images. Complete recovery before rebooting after a failed boot-setup operation.

AppArmor-only setup uses `sudo scripts/apparmor-setup restore`. Greetd/PAM setup
uses `sudo scripts/system-setup restore`. Changes to kernel arguments take effect
after reboot. See [AppArmor](APPARMOR.md), [console routing](greeter-console.md) and
[boot setup](boot-optimization.md) for each helper's requirements.
