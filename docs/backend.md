# Rust command backend

`native/foundation` builds `desktop-foundationctl`. Run it through
`scripts/foundation`, which selects the checkout's release binary and supplies
its root. Build with `scripts/build-backend`; installation builds it before
deployment. Keep the executable and its callers from the same source revision.

## Commands

```text
clipboard [--state DIRECTORY] init|store text|store image|copy ID|delete ID|clear
apps launch [terminal|browser|files|pdf|image|text] [--check]
volume up|down
bluetooth power on|off
bluetooth codecs DEVICE_PATH
bluetooth connect|disconnect|reconnect DEVICE_PATH
bluetooth codec DEVICE_PATH --codec sbc|sbc_xq
screenshot [--backend niri|hyprland] region|window|output
power suspend|logout|reboot|poweroff [--check]
actions list|plan ID|invoke ID
cache plan|prune
theme generate IMAGE
theme derive --source SOURCE --hash HASH
wallpaper start
notifications start|check-owner
system packages [--config FILE]
shell call METHOD [ARGS...]
```

Clipboard `store` reads bytes from stdin. `theme derive` reads material-palette
JSON from stdin. State responses use JSON; `system packages` returns a display
string and shell calls return the invoked method's response. Failures report to
stderr and exit nonzero. Native command arguments are passed as arrays.

## Behavior

- **Clipboard:** SQLite storage, deduplication, retention and atomic index/image
  projections. Copy hands exact stored bytes to wl-copy. See [clipboard](clipboard.md).
- **Applications:** role selection follows the installation journal and validates
  desktop entries against their configured executable. `--check` returns the
  launch plan. [Application roles](application-roles.md) also covers MIME defaults.
- **Volume:** 3% changes, capped at 100%, followed by sink readback. The shell shows
  the resulting percentage; Mako provides a fallback when shell IPC is unavailable.
- **Bluetooth:** BlueZ radio and connection operations with state confirmation,
  plus PipeWire codec discovery and playback routing. See [Bluetooth](bluetooth.md).
- **Screenshots:** backend-specific capture and clipboard handoff. Selection waits
  for the user; capture and clipboard requests have deadlines. See [screenshots](screenshots.md).
- **Power:** native systemctl/logind operations and compositor-aware logout.
  `--check` returns routing without executing it. The menu executes a selected
  action immediately; logind/polkit controls authorization.
- **Actions:** stable IDs, labels, icons, categories, keywords, availability and
  invocation policy for application, volume and power actions. `list` and `plan`
  return metadata; `invoke` executes the action.
- **Theme:** wallpaper hashing, Matugen extraction, semantic color mapping,
  contrast checks and palette caching. `generate` returns `{palette, cached}`.
  The theme transaction stages and publishes the active revision separately.
  See [theme controls](../theme/README.md).
- **Wallpaper:** reads the active wallpaper configuration and replaces itself
  with swaybg. Legacy startup prepares the overview image before that handoff.
- **Notifications:** starts the configured session unit. `check-owner` succeeds
  only when the session bus reports that the notification name is available.
- **Packages:** combines Fastfetch's native total with pacman's foreign count.
  Cache validity follows package-database signatures and repository changes;
  package transactions prevent cache reuse or publication.
- **Shell IPC:** waits up to three seconds for readiness, then sends the requested
  method once with a three-second deadline. Use `scripts/shell-reload` after
  replacing QML source. See [surface behavior](surface-qol.md).

## Cache maintenance

`cache plan` lists retention candidates; `cache prune` removes eligible owned
files under the theme transaction lock. Publication requests maintenance after
releasing that lock.

Retention keeps six theme revisions, including current and previous, 32 semantic
palettes and 16 overview PNGs. Active palette/backdrop references are protected.
Only recognized owned files are eligible; foreign revisions, backups, journals
and symlinks are retained. Invalid pointers or ownership metadata stop removal.
A maintenance error leaves the published theme available for use.

Finite native subprocesses have output bounds and deadlines. Failure cleans up
the command's process group; successful clipboard owners survive the helper's
handoff. Application and native session startup paths use process replacement.
SQLite is dynamically linked from the system. Nothing/CMF protocol commands use
[their separate backend](nothing-controls.md).
