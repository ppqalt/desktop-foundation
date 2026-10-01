# Desktop foundation

A Niri-first Arch/CachyOS desktop with a restrained graphite/glass theme. Includes
an application launcher, searchable text/image clipboard with persistence, session
power menu, paired-device Bluetooth popup with native Nothing/CMF controls, compact volume feedback, clipboard-only screenshots, and a portable
Kitty/Fish/Fastfetch environment. No permanent bar.

## Install

Clone this repository into a permanent directory, then run as your normal user:

```sh
cd desktop-foundation
./scripts/install --plan   # optional: preview the steps
./scripts/install
```

The installer installs the runtime packages through Pacman (sudo), validates
your Niri build, backs up replaced configuration, deploys bundled fonts and wallpaper,
and sets dark mode. It preserves an existing browser/file-manager default; fresh
hosts receive Firefox and Nautilus. Choose **Niri** at your next login.

The standard install includes greetd/tuigreet with Matrix animation and keyring
login unlocking. That system step needs sudo, backs up its files, and does not
restart the active display manager. Reboot when installation finishes and select
**Niri**. This is intended for a fresh Arch/CachyOS system with working graphics,
a normal user account, networking and sudo already configured.

Use `./scripts/install --no-greeter` only when deliberately supplying another login
manager (or starting `niri-session` from a TTY). See
[installation and compatibility](docs/installation.md). For host-specific monitor
configuration use `--profile NAME`; the default has no output/GPU assumptions.

**Compatibility:** the exact appearance requires a Niri build supporting the
configured native blur/background effects. Tested with CachyOS Niri 26.04 and
Quickshell 0.3.1. Installation checks the actual config parser before replacing
files; a version number alone is not sufficient. Plain Arch builds may require a
compatible Niri package. This is an Arch-native setup, not a cross-distro installer.

## What you get

- Finnish keyboard (`fi`); input stays separate from hardware profiles.
- Matching active/inactive 90% content opacity, rounded windows and soft depth.
  Kitty uses native 90% background alpha with opaque text.
- Super+Space launcher, Super+V clipboard, Super+Shift+Q session actions.
  Scroll moves menu selection; clicks/Enter act immediately.
- Super+B reconnects paired Bluetooth devices; wheel/arrows select, Enter/click act.
  Dynamic LDAC → AAC → available A2DP preference, battery reporting and playback
  routing; [Blueman handles pairing and administration](docs/bluetooth.md).
- PageUp/PageDown ±3% volume; End play/pause. Small themed feedback fades away.
- Print copies the current screen; Super+Shift+S captures a dragged area to clipboard.
- Google Sans, Google Sans Code and Nerd Font Mono bundled with upstream licenses.
- Small native Fish prompt; `c` clears and `fast` clears then runs Fastfetch.
  Pacman/AUR counts are discovered dynamically and foreign counts cached until changes.
- Wallpaper and clipboard ownership start with the Niri session, one process each.

Overview, window actions, scrolling columns and strict single-window centering are
covered in [the binding guide](docs/input-bindings.md). Settings and cheatsheet are
still placeholders. Hyprland code remains as an optional secondary backend;
`./scripts/bootstrap --hyprland` installs its packages, then deploy explicitly with
`./scripts/deploy --compositor hyprland`.

## Validate and undo

After a fresh login, run `./scripts/session-acceptance`. It inspects without repairing
missing services. Follow [the cold-login checklist](docs/reliability.md) for actual
keyboard, clipboard, portal, audio and screenshot checks.

```sh
./scripts/uninstall             # restore user configurations/preferences
./scripts/uninstall --greeter   # also restore the optional system files
```

Packages remain installed. Configuration changed outside deployment is protected
from being overwritten. Keep the checkout and backups until rollback is complete;
symlinks point into this checkout. Existing terminals are not forcibly closed.

## Optional extras and development

Spotify/Spicetify/Marketplace is a separate opt-in download:
[setup instructions](apps/spotify/README.md). Accounts, saved passwords, application
data, Bluetooth pairings, network connections and machine hardware settings are
not copied from Tops. Optional apps require their own setup.

Development tools are opt-in: `./scripts/bootstrap --dev`, then `./scripts/check`
and `./scripts/test`. Portable checks run in GitHub Actions; live compositor testing
remains a separate acceptance pass. See [contributing](CONTRIBUTING.md),
[deployment](docs/deployment.md), [terminal](terminal/README.md) and
[font sources](fonts/README.md). Asset provenance and publication decisions are in
[publication notes](docs/publication.md).

Native earbud controls: [implementation, hardware validation and limits](docs/nothing-controls.md).
The optional device-control helper contains AGPL-derived code; its full
[license](native/nothing/LICENSE) and [source attribution](native/nothing/NOTICE.md)
are included. No browser or web server is involved.
