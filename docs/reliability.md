# Niri reliability pass — 2026-10-01

No new UI surfaces. Existing launcher, clipboard, terminal and window treatment preserved.

## Wallpaper

Two causes of this boot's missing wallpaper: the old unit lived in ephemeral
`$XDG_RUNTIME_DIR/systemd/user`, and the configured Downloads image no longer existed.
The selected image is now `~/.local/share/wallpapers/wallhaven-135w7w.png`, linked by
reversible deployment to `wallpapers/wallhaven-135w7w.png` in this checkout. The
original is `~/Pictures/Wallpapers/wallhaven-135w7w.png`. It is the user's supplied
3840×2160 Windows/Tux wallpaper. No generated replacement.

`compositor/niri/wallpaper.toml` controls path and aspect-preserving `fill` mode.
Deployment owns persistent unit and `niri.service.wants` links under
`~/.config/systemd/user`; service starts after Niri is ready, stops with Niri and
execs swaybg directly. The session-start script no longer also starts wallpaper.
Restart for a path change: `systemctl --user restart desktop-foundation-wallpaper`.
Rollback stops the owned wallpaper/persistence services and restores previous paths.
Exactly one swaybg verified. Repeated start is idempotent; full reboot pending.

## Clipboard

One upstream `wl-clip-persist --clipboard regular` owns all offered MIME data in
memory. Existing wl-paste text/image watchers remain history readers, not owners.
No polling, new framework or custom daemon. 32 MiB aggregate selection limit;
password-manager-hint offers excluded. Primary selection remains untouched.
History limits remain separate (8 MiB/item, 32 MiB total). Larger offers can remain
pasteable only while the source lives; this is a documented bound.

Installed Arch package required: `wl-clip-persist`. For today's no-sudo validation,
the official extra package 0.5.0-2 was downloaded, its detached signature verified
against the installed pacman keyring (Robin Candau), and its binary extracted to
`~/.local/state/desktop-foundation/bin`. SHA256 of the package:
`2914a8ace100676d010c2a8ceaa918179b8d4e0c598e7bef6d1487e321f3ac5d`.
The tracked wrapper prefers the package binary once installed by bootstrap.
Persistent Niri-bound systemd unit is installed and rolled back with deployment.
Never enable a second clipboard persistence manager alongside this one.

Text checks used isolated real native Wayland Brave and Kitty processes: copy,
terminate the source, paste via Ctrl+Shift+V into a target Kitty, verify the input.
Brave copied a controlled address-bar string; Kitty copied native selected output.
Both passed. A real GTK Wayland source offered PNG data, was fully terminated, and identical
PNG bytes remained pasteable. Image check passed as well.
Clipboard contents and window focus were restored; fixtures closed.

## Secret Service

This boot had two processes: a D-Bus-activated `--start --foreground --components=secrets`
process and the socket-activated `gnome-keyring-daemon.service`. The latter owned
`org.freedesktop.secrets`. Per-user XDG overrides already hid both GNOME autostarts;
these are now tracked and reversible rather than copied home-directory files.

A per-user D-Bus service override delegates activation to the existing systemd
service. The redundant D-Bus process was stopped; the owner, socket, keyrings and
Secret Service remain. One daemon verified. COSMIC packages did not cause this
specific duplicate: the conflicting D-Bus and systemd paths did.

Default collection existed and was unlocked at inspection. No passwords or
keyring data were read or modified. Actual login PAM lacked GNOME Keyring hooks:
this explains why a password-protected collection can prompt when apps first use it.
`session/greetd/pam-greetd` adds optional auth/session hooks. Apply with
`sudo scripts/system-setup install`, then password-login next boot. If the default
collection password differs from the login password, change it in a keyring manager
using its current password. No empty-password keyring or disabled Secret Service.
A prompt-free fresh login is not yet proven. Browser credential storage and other
Secret Service clients must continue working; no app password-store hacks applied.

## Greeter / system changes

Already done before this pass: greetd + tuigreet installed; greetd active/enabled,
COSMIC Greeter inactive/disabled; display-manager symlink points to greetd.
Boot journal proves this session logged in through greetd/tuigreet. Niri's installed
Wayland session entry executes `niri-session`, so it remains selectable.

The working config is tracked in `session/greetd/config.toml`. The separate root
installer backs up greetd and PAM files under `/var/lib/desktop-foundation/system`,
refuses to overwrite externally modified deployed files, and never stops/restarts
the active display manager. `sudo scripts/system-setup restore` restores files.
Recovery: Ctrl+Alt+F3, log in, restore config/PAM. For COSMIC recovery, explicitly
`sudo systemctl enable cosmic-greeter.service --force` and reboot; no live restart.
Current greetd setup needs no service switch. PAM enhancement needs sudo/fresh login.

## Cold-login acceptance (pending next reboot)

Do not run repair/start commands before taking the snapshot:

1. Run `scripts/bootstrap` (administrator password needed for missing packages),
   `scripts/deploy`, and `sudo scripts/system-setup install` before reboot.
2. Reboot normally; choose Niri in tuigreet and authenticate with your password.
3. Run `scripts/session-acceptance` and retain its boot ID/output. It only inspects.
4. Confirm wallpaper, dark preference, Finnish letters/punctuation and Shift+7 `/`.
5. Super+Space launcher: search/open Kitty. Fish prompt and Fastfetch must appear
   once; `c` and `fast` work. Check native terminal transparency/crisp text.
6. Super+V: search text/image history; clicking copies and closes immediately.
   Copy from Brave, terminate it, paste into Kitty; repeat Kitty and image source.
7. Screenshot region selects from no initial area, auto-captures after drag;
   Print copies full screen; neither writes a screenshot file. Test cancellation.
8. Super+scroll navigates; notifications appear top centre and expire. No manual
   service start. Test file picker/screen sharing portal and polkit authentication.
9. Check audio and media keys, xwayland-satellite compatibility, workspace/focus
   transitions, single-window centering, and absence of keyring prompts.
10. Repeat logout/login once: one shell, swaybg, persistence owner, keyring daemon;
    two history watchers; no failed user units or services tied to an old socket.

Current session doctor reports HEALTHY, Finnish/dark mode active, audio/portals/
polkit/shell/history/wallpaper/persistence/notifications healthy. Original runtime
shell/history/notification units are recreated by tracked session-start each login;
they are intentionally session-local, not stale persistent startup dependencies.
Full post-change cold boot not performed by this agent. No logout or reboot forced.
