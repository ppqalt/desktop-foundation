# Desktop Foundation

A barless CachyOS and Niri desktop with Material-inspired widgets, wallpaper colors and Rust backends.

## Install

Fresh installs prepare the EAC kernel with independent Limine/LTS snapshot recovery.
Existing installations preserve their boot setup. Firmware steps may require a
manual reboot and rerunning the same command. Desktop deployment can finish
with boot verification pending; see [Secure Boot and recovery](docs/secure-boot.md).

```sh
git clone https://github.com/ppqalt/desktop-foundation.git
cd desktop-foundation
./scripts/install-all
```

This installs the full desktop and application setup, including recording/replay
tools and patched Niri. Use `./scripts/install-core` for Minimal, or
`./scripts/install --standard` for Standard (paru, stable Brave Origin from pacman, GNOME utilities, recording and firmware). Existing login managers are preserved
automatically. Every profile builds and installs patched Niri; your running
session stays open and the patch activates at the next login. Stock recovery is
retained. See [installation](docs/installation.md) for requirements and options.

## Shortcuts

| Keys | Action |
| --- | --- |
| Super+Space | Applications |
| Super+V | Clipboard |
| Super+B | Bluetooth |
| Super+Z | Control Center |
| Super+X | System monitor |
| Super+Shift+Q | Session |
| Super+Tab | Overview |
| Super+Shift+C | Clock |
| Super+T | Terminal |
| Super+E / Super+W | Files / browser |
| Insert / Delete | Next / previous track |
| Pause/Break | Transparency Mode |
| Shift+Pause/Break | Microphone monitoring |

```sh
df-update
./scripts/wallpaper-set /path/to/image
```

[All shortcuts](docs/input-bindings.md) · [Theme](theme/README.md) · [Updates](docs/updating.md) · [Recovery](docs/recovery.md)

### Automatic Lucky38 performance policy

General clipboard/cache/readiness improvements ship with normal DF updates.
The normal installer automatically selects reversible binfmt startup policy
only for the matching Lucky38 motherboard/CPU/GPU combination. No extra command
is required. `--performance-profile off` disables it persistently; updates preserve
and reconcile the choice. Other hardware stays unchanged. See
[performance policy, trade-offs and rollback](docs/lucky38-performance.md).
