# Desktop Foundation

A barless CachyOS and Niri desktop with Material-inspired widgets, wallpaper colors and Rust backends.

## Install

```sh
git clone https://github.com/ppqalt/desktop-foundation.git
cd desktop-foundation
./scripts/install-core
```

Use `./scripts/install-all` for the full application setup. Both commands build
and install the patched Niri compositor automatically; your running session stays
open and the patch activates at the next login. Stock recovery is retained. See [installation](docs/installation.md) for requirements and options.

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
| Insert | Transparency Mode |

```sh
df-update
./scripts/wallpaper-set /path/to/image
```

[All shortcuts](docs/input-bindings.md) · [Theme](theme/README.md) · [Updates](docs/updating.md) · [Recovery](docs/recovery.md)
