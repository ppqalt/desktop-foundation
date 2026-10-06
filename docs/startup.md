# Session startup

Deployment generates foundation systemd user units under
`$XDG_STATE_HOME/desktop-foundation/session-units` and installs journaled links in
the user configuration directory. Login uses those definitions.

Niri imports its native environment before running `scripts/session-start`. That
script imports compositor and foundation variables into D-Bus/systemd and starts
`desktop-foundation-session.target` asynchronously.

```text
foundation session target
├─ Quickshell
├─ Mako on Niri
├─ polkit
└─ clipboard initialization → text and image watchers

Niri session
├─ wallpaper
└─ clipboard persistence
```

The shell starts independently of auxiliary services. Clipboard watchers wait for
database initialization. The clipboard popup retries a missing index for up to
three seconds. Launcher, clipboard, Bluetooth and power surfaces are created on
demand; Nothing controls and audio setup also run on demand.

Niri owns its graphical session and Xwayland satellite. The foundation target and
services follow graphical-session shutdown. Hyprland uses the same foundation
session units, with UWSM finalization when launched through UWSM.

Deployment applies dark preferences and installs user autostart overrides. The
Cachy-Update tray entry is excluded in Niri, which has no StatusNotifierWatcher.
Its application and timer remain available. An existing notification provider is
checked before the foundation Mako service claims its D-Bus name.

## Diagnostics

```sh
scripts/doctor
scripts/doctor --startup
scripts/doctor --startup --save before
scripts/doctor --startup --save after
scripts/doctor --startup --compare
```

Startup reporting reads systemd and journal timestamps. Saved samples go to
`$XDG_CACHE_HOME/desktop-foundation/startup/samples`; comparison counts each Niri
process launch once. A retained user manager can have earlier `default.target`
and PSD events, which the report separates from the current session.

The report measures session-open, Niri-ready, shell-launch and QML-loaded intervals.
`Configuration Loaded` records QML initialization. For popup creation and focus
roundtrips through resident-shell IPC, close the popups and run:

```sh
scripts/doctor --startup --benchmark
```

This command opens and closes the launcher, clipboard, Bluetooth and power
surfaces. See [performance](performance.md) for process measurements and QML
profiling, and [recovery](recovery.md) for service recovery.
