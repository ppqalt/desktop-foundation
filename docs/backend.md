# Rust command backend

`native/foundation` builds `desktop-foundationctl`. Run it through
`scripts/foundation`, which selects the checkout's release binary and supplies
its root. Build with `scripts/build-backend`; installation builds it before
deployment. Keep the executable and its callers from the same source revision.

## Commands

```text
clipboard [--state DIRECTORY] init|watch|store text|store image|store --mime MIME
clipboard [--state DIRECTORY] copy|copy-plain|pin|unpin|delete ID
clipboard [--state DIRECTORY] clear|clear-all
apps launch [terminal|browser|files|pdf|image|text] [--check]
app-roles roles|apply|check [--personal]
app-roles validate ROLE|desktop NAME|conflicts|restore
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
theme gtk-accent
preferences [--state DIRECTORY] set TYPE KEY VALUE|check|restore|gtk-refresh
wallpaper start
notifications start|check-owner
system packages [--config FILE]
shell call METHOD [ARGS...]
local-ai start|status|plan
build-info
doctor --json
monitor toggle|close|status|refresh
browser-theme status|apply COLOR|remove
```

Clipboard `store` reads bytes from stdin. `theme derive` reads material-palette
JSON from stdin. `theme gtk-accent` reads semantic palette JSON and returns the
nearest supported GNOME accent. State responses use JSON; `system packages` returns a display
string and shell calls return the invoked method's response. Failures report to
stderr and exit nonzero. Native command arguments are passed as arrays.

## Behavior

- **Clipboard:** SQLite MIME bundles, file lists, pinning, retention and atomic
  index/image projections. Native Wayland capture and replay preserve formats
  together; normal clearing retains pins. See [clipboard](clipboard.md).
- **Applications:** role selection follows the installation journal and validates
  desktop entries against their configured executable. `--check` returns the
  launch plan. Default assignment, validation, ownership and rollback also run
  in Rust. Per-key journals retain original defaults and recover interrupted
  writes without replacing unrelated associations. See [application roles](application-roles.md).
- **Volume:** 3% changes, capped at 100%, followed by sink readback. The shell shows
  the resulting percentage; Mako provides a fallback when shell IPC is unavailable.
  Audio changes are serialized independently of feedback. Repeated presses keep
  applying their steps while a single finite feedback owner catches up to the
  latest volume, so slow shell IPC does not hold the audio-change lock.
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
- **Preferences:** native setting reads/writes and the existing per-setting
  ownership journal. Pending writes preserve recovery across interruptions;
  restoration checks ownership before changing settings. GTK refresh selects
  the accent from the active palette and checks the installed enum.
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
- **Local AI:** starts the configured existing Ollama/Odysseus installation and
  opens its browser UI. See [local AI](local-ai.md).
- **Diagnostics:** embedded source/build identity and read-only JSON deployment
  metadata. See [diagnostics](diagnostics.md).

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
