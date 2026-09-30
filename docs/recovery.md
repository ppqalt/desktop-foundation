# Recovery and login

Prefer **Hyprland (UWSM)** at the physical login screen. Current session remains
direct; a logout was deliberately not performed while applications are open.
Hyprland startup invokes session-start: UWSM finalize when managed, otherwise
DBus/systemd environment import; packaged polkit starts independently; shell-start
writes a user runtime unit and starts the shell. No globally enabled agent or shell
unit affects COSMIC. UWSM graphical-session shutdown stops the shell; direct
session-exit explicitly stops shell/polkit before compositor exit.

The shell service restarts on process failure after one second, at most three
starts per minute. QML syntax reload errors are logged; inspect whether the old
configuration survives. Kitty and Hyprland are independently launched and survive
shell failure. Recover with `scripts/shell-start`, stop with `scripts/shell-stop`.
Use `journalctl --user -u desktop-foundation-shell.service` and `scripts/doctor`.
Unsupported compositor/missing IPC exits clearly; never restart the compositor to
repair shell state. Portal diagnosis is read-only and does not replace the user's
portal configuration. Physical UWSM login/logout validation is still required.

Deploy uses an exclusive lock, durable write-ahead manifest and original inode
identity. It validates Lua before replacement, atomically updates generated wrapper,
and records each replacement before backup/link. After interruption run
`scripts/restore`, then deploy again. Never delete the manifest/backups manually.
Foreign replacement is refused; restore never overwrites a new user config.
Backups use rename, so custom CONFIG/STATE across filesystems can fail safely.
