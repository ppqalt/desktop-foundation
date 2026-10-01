# Fresh installation audit

Audited 2026-10-01 against Tops. This is preparation, not proof of a fresh install.

| Component | Classification and installer path |
|---|---|
| Niri, input, keys, 90% opacity, blur, rounding, centering, animations | Reproduced automatically by render/deploy; blur-enabled Niri required |
| Output settings | Intentionally machine-specific `--hardware-profile Tops`; default portable |
| Launcher, clipboard, power, Bluetooth, volume UI | Reproduced automatically: tracked Quickshell; no redesign |
| Native Nothing/CMF helper | Reproduced automatically: locked Rust 1.98.1 build, AGPL source/NOTICE |
| Kitty/Fish/Fastfetch, functions, package count cache | Reproduced automatically by reversible deployment |
| Wallpaper | Reproduced automatically at XDG data/wallpapers; public redistribution permission unresolved |
| Google Sans/Code/Nerd glyphs | Reproduced automatically: vendored checksummed OFL fonts with notices |
| Clipboard watchers/persistence | Reproduced automatically: session startup, Niri-owned user units |
| Notifications, screenshots, media controls | Reproduced automatically; explicit packages and tracked scripts; live swaybg/mako/wl-clip-persist binaries are unowned by Pacman, corrected through declared repository packages on fresh install |
| Dark mode | Improved: tracked preferences and automatic session helper |
| BlueZ | Reproduced automatically: explicit packages and enabled service; no Blueman tray |
| PipeWire/WirePlumber, portals, polkit, XWayland | Reproduced automatically: explicit packages; hyprpolkitagent is the chosen standalone agent, not a Hyprland session dependency |
| Matrix greetd/PAM | Improved: tracked files, backups including absent originals; capability probe; CachyOS Matrix-enabled tuigreet required |
| AppArmor | Missing before this pass; now package/service/GRUB drop-in/doctor; reboot enforcement pending; non-GRUB automation unsupported |
| Paru | Missing before this pass; personal nonroot PKGBUILD bootstrap with review |
| Brave Origin Nightly | Verified live `brave-origin-nightly-bin`; personal manifest, package-owned `brave-origin-nightly` and `brave-origin-nightly.desktop`, explicit default association |
| Spotify/Spicetify/Marketplace | Existing pinned user-local installers now called by personal installer; Spotify Debian source, not a pacman package; first-login apply remains |
| Browser/Spotify credentials, cookies/history/sessions | Intentionally personal state; never restored |
| Bluetooth pairings, clipboard/Fish history, Documents/Downloads | Intentionally personal state; never restored |
| Old COSMIC/Hyprland session state | Not required; Hyprland support stays optional; old greeter disable is compatibility cleanup only |

`packages/*.txt` owns repository package requirements. Personal AUR additions are
separate from user-local downloads in `apps/spotify/sources.json`. Spotify sources
are pinned and hash-verified, and Marketplace uses upstream's recommended manual
release extraction rather than executing a mutable remote installer.

The default install is personal; `--core` omits Paru, Brave and Spotify tools.
`--profile` remains a backwards-compatible hardware option; prefer the explicit
`--hardware-profile` spelling. No hostname branching was introduced.

Known limitations: installation depends on current repositories, pinned download
availability and x86_64 Spotify artifacts; an actual blank system, GPU session and
Bluetooth must still be tested. AppArmor enforcement may reveal application
profile denials after boot. No destructive reinstall or publication occurred.

## Non-destructive validation results

- All repository manifest names resolved using the configured Pacman databases;
  AUR RPC resolved Paru and Brave Origin Nightly. Brave's executable/desktop entry
  were checked against its installed package file list.
- Core and personal dry runs completed; isolated deploy dry run created no files.
- Clean temporary configuration deployment twice and rollback passed, including
  restoration of an existing Kitty directory and persistent Niri unit links.
- Paru's missing-helper path passed with instrumented commands: Pacman installed
  build tooling, clone/review preceded nonroot makepkg. This did not compile Paru
  on a blank OS; that remains part of the genuine fresh installation.
- GRUB drop-in repeated sourcing preserves its LSM list without duplicate flags.
  No live boot configuration was edited or regenerated during these tests.
- Locked Nothing release build succeeded from an empty separate Cargo target.
- 32 Python tests passed; nine Rust tests passed. Repository static checks passed,
  including shell formatting/lint, QML, generated Niri/optional Hyprland configs,
  Cargo formatting/clippy and Rust tests. Existing QML metadata warnings remain.
- Pinned Spotify/Spicetify/Marketplace URLs returned HTTP 200. All eight cached
  client/tool/runtime artifacts matched their recorded SHA-256 hashes. No live
  Spotify login or client resources were changed during the installer audit.
- Font files all matched their recorded hashes. Privacy/history review is recorded
  separately in PUBLICATION_REVIEW.md.

Live installation doctor correctly flags unowned swaybg/mako/wl-clip-persist
package gaps, and warns about the disabled AppArmor kernel. The installer fixes
the package graph and boot configuration for the future installation; this pass
intentionally leaves Tops' current system untouched. It does not claim a genuine
fresh install or new AppArmor enforcement has been validated. Bare-metal login,
GPU visuals, audio/Bluetooth and first-run Spotify finalization remain checklist
items. No temporary UI windows were created during this pass.
