# Desktop Foundation

A Niri desktop for Arch and CachyOS with graphite menus, wallpaper-derived colors,
Rust backends and a shared terminal setup.

![Application launcher](docs/screenshots/launcher.png)

## Install

Use a normal login user with sudo, Git and Python. Keep the checkout in a
permanent location.

```sh
git clone --branch v0.20 https://github.com/ppqalt/desktop-foundation.git
cd desktop-foundation
./scripts/install-core --dry-run
./scripts/install-core
```

Core includes the desktop, terminal, fonts, audio, Bluetooth, application defaults
and login setup. For Spotify, Spicetify, Marketplace, ChatGPT and Steam, use
`./scripts/install-all`.

Log into Niri after installation. Use `--no-greeter` to keep an existing login
manager, or `--no-packages` when dependencies are already installed.
[Installation options](docs/installation.md) cover profiles, upgrades and boot setup.

## Desktop

- Application launcher and searchable text/image clipboard history.
- Bluetooth switch, paired-device connections and native Nothing/CMF controls.
- Session menu with suspend, logout, reboot and power off.
- Wallpaper-derived accents across the shell, terminal, notifications and apps.
- A toggleable corner clock and volume readout with instant bar updates.
- Scrolling columns, automatic single-column centering and manual centering.
- Kitty, Fish and Fastfetch with Google Sans fonts and cached package counts.
- Matrix login screen, AppArmor setup and boot configuration helpers.

Menus share the same card styling, keyboard navigation and animations. Applications
remain visible behind their dimmed, blurred backgrounds. Volume uses only a 100 ms
fade in/out; the bar and percentage update immediately.

| Shortcut | Action |
| --- | --- |
| Super+Space | Application launcher |
| Super+V | Clipboard history |
| Super+B | Bluetooth |
| Super+Shift+Q | Session menu |
| Super+Tab | Overview |
| Super+Shift+C | Corner clock |
| Super+T / Super+Enter | Terminal |
| Super+E / Super+W | Files / browser |
| Super+C | Center column |
| Super+R | Cycle column width |
| PageUp / PageDown | Volume +3% / −3% |
| End | Play/pause |
| Print / Super+Shift+S | Copy screen / selected region |

Menu shortcuts fire once per press. Arrow keys, Tab and the mouse wheel select;
Enter or a click activates. Escape closes. The default keyboard layout is Finnish;
edit `config/input.lua` to change it. [All bindings](docs/input-bindings.md).

## Wallpaper and colors

```sh
./scripts/wallpaper-set /path/to/image
./scripts/wallpaper-set /path/to/image --mode fit
./scripts/theme-rollback
```

Matugen supplies the accents while surfaces stay graphite. Theme changes are
cached and saved as complete runtime revisions. [Theme controls](theme/README.md)
include Brave and Spotify setup and reload commands.

## Tools

```sh
./scripts/doctor
./scripts/install-core --check
./scripts/shell-reload
./scripts/uninstall
```

See [recovery](docs/recovery.md), [application defaults](docs/application-roles.md),
[Bluetooth controls](docs/nothing-controls.md), [screenshots](docs/screenshots/README.md)
and [v0.20 release notes](docs/releases/0.20.md).

[Development](CONTRIBUTING.md) · [Architecture](docs/architecture.md) ·
[Rust commands](docs/backend.md) · [Fonts](fonts/README.md)
