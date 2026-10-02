# Session startup and measurement

Deployment generates the foundation shell, notification, clipboard-init and watcher
units under the user's state directory, then installs journal-owned symlinks and
reloads systemd. Reinstall replaces changed generated definitions atomically; it
preserves original backups. Hyprland deployment also gets the shared session units.

Niri imports its native environment before signalling readiness. Its startup command
runs `session-start`, which imports foundation-specific variables and starts
`desktop-foundation-session.target` without waiting for the target's jobs.

```
Niri ready → environment import → foundation session target
                                 ├─ Quickshell
                                 ├─ Mako (Niri only; retain another provider)
                                 ├─ polkit
                                 └─ clipboard initialization → text/image watchers
Niri ready → wallpaper
           → clipboard persistence
```

The shell does not wait for notifications, wallpaper, clipboard, audio, PSD, portals
or GVFS. Clipboard watchers require initialization; the clipboard popup retries a
missing index for at most three seconds. Launcher/clipboard/Bluetooth/power remain
lazy. Native Nothing controls and audio setup remain on-demand. Ordinary login
never generates unit files or reloads the user manager. Dark preferences are applied
reversibly during deployment using `preferences.py install --theme-only`, rather
than an unreachable command after `exec`.

Cachy-Update's tray autostart is excluded only in Niri: this desktop has no
StatusNotifierWatcher. The Cachy-Update application and update timer are retained;
package files are untouched. GeoClue remains: its agent can authorize location
requests. PSD, portals, GVFS, audio, keyring and lazy satellite are unchanged.

An early shortcut probes resident-shell IPC for bounded readiness, then invokes
its requested toggle exactly once. It never starts another shell and does not move
the pointer. A failed early IPC request is not interpreted as a working keyboard.

## Diagnostics

```
scripts/doctor --startup
scripts/doctor --startup --save before
scripts/doctor --startup --save after
scripts/doctor --startup --compare
scripts/doctor --startup --benchmark
python3 tests/live_session_surface.py
```

Normal startup reporting reads systemd/journal evidence only. `--save` writes
sanitized timings to the user's cache, not Git. Comparison deduplicates repeated
captures of the same Niri process launch. A retained user manager can leave
`default.target` and PSD timestamps from an earlier login; these are separated from
the current-session timeline. Never mistake that target's original 1.173-second
startup statistic for a new login duration.

`--benchmark` explicitly opens/closes resident popups and measures IPC roundtrip
until creation/focus is reported. It refuses an already-open popup. No radio,
connection, earbud, volume or power action is invoked. This is not a login benchmark,
physical key latency, nor compositor presentation timing. The opt-in live test uses
a finite virtual keyboard, checks the four actual bindings, typing/navigation and
Escape cleanup, without pointer motion or destructive actions.

Precise first-frame presentation, separate Qt engine/import/root creation stages,
and authentication acceptance are not instrumented. `Configuration Loaded` remains
an initialization marker. Future presentation telemetry should use a supported
window/compositor presentation signal, not an inferred timestamp or private QML API.

Quickshell already writes `.qmlc` files to its Qt QML cache. No speculative cache
or precompilation change was made. Qt documents its cache behavior at
https://doc.qt.io/qt-6/qmldiskcache.html and profiling support at
https://doc.qt.io/qt-6/qtquick-profiling.html.

## Rollback and remaining validation

User configuration, autostart overrides and unit symlinks are journal-owned with
original backups. `scripts/uninstall` restores the managed desktop configuration
and recorded preferences; it stops the foundation services, so use a recovery TTY
or prepare to log out. No package-owned autostart file or system login/boot file was
changed in this optimization. To return to the old foundation version after restore,
select the pre-refactor Git revision and deploy it normally before the next Niri login.
Do not simply overwrite managed symlinks or delete the deployment manifest.

Real logout/login samples are required before reporting a startup improvement.
A cold user-manager login and a same-boot relogin are different populations; keep
them identifiable and do not claim three cold boots from three Niri sessions.
Recheck all keybindings with the pointer untouched after login. Also validate
clipboard text/images/persistence, notifications, OSD, screenshot selector, terminal,
browser, Spotify, file picker/screen sharing, keyring and polkit on the target host.

The optional TTY experiment is a separate phase after startup regression validation.
Current `tuigreet` is already terminal-based, rather than a graphical compositor.
A smaller `agreety` frontend can keep manual username/password authentication and
the existing greetd PAM/keyring/session launch, without unsafe Fish auto-start hooks.
A true getty/login path changes PAM integration and needs a separately journaled
system installer and a confirmed recovery TTY. Neither path has been switched yet.
