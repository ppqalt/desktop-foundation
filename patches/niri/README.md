# Overview-only Ctrl+wheel

`overview-ctrl-scroll.patch` adds an opt-in setting to Niri's native overview
wheel handler. It was prepared against upstream commit `8ed0da4`, the source
revision reported by Niri 26.04. The corrected zoom behavior requires the
corresponding rebuilt compositor package.

The patch adds this compositor configuration:

```kdl
overview {
    ctrl-scroll-workspaces true
}
```

With overview open, vertical wheel events with exactly Ctrl adjust the overview
zoom: up zooms in and down zooms out. The overview remains open. Zoom changes
are clamped between 15% and 75% and apply to all outputs. The setting retains
its original name for compatibility. Ctrl scrolling uses an independent wheel-detent accumulator without an additional cooldown, so partial high-resolution wheel events do not leak into workspace scrolling. Ordinary
wheel, horizontal wheel, touchpad gestures and other modifier combinations keep
their existing behavior. The existing exclusion for layer surfaces is preserved.

The patch also gives overlay-layer surfaces pointer priority over the hot corner. This lets a screenshot selector cover the corner without changing or reloading the user's hot-corner settings; closing or crashing the selector naturally removes the overlay. This behavior still needs physical-session acceptance.

Outside overview the new zoom branch is unreachable. There are no global bindings,
input grabs, replayed events, IPC listeners or background processes.

## Build and integration status

`scripts/build-niri-package` builds a pinned, separate `niri-foundation` pacman
package without installing it. It provides `niri=26.04` and conflicts with stock
`niri`, so an unattended repository upgrade cannot silently replace this package
with stock Niri. The build checks the patch checksum, builds the exact source
commit with the lockfile and runs Niri configuration tests. The package is retained
in a private per-build state directory. It does not automatically follow Niri
security updates: maintainers must review and rebuild the pinned revision.

The normal installer now builds and installs this package in every installation
profile, including Minimal and Everything (`install-all`). There is no second
patching command. `--stock-niri` saves a stock-compositor preference; on a managed
installation it restores the original signed package. `--no-packages` performs
source/configuration deployment only and leaves compositor packages alone.

Before replacement, `scripts/niri-package install` retains the exact installed
stock package and its verified distribution signature, checks the candidate's
package identity and validates the active configuration with the candidate.
If the exact signed recovery package cannot be obtained, installation stops.
The package and rollback journal live under
`$XDG_STATE_HOME/desktop-foundation/niri-package`, separately from deployment
ownership and original configuration backups. Do not remove that directory.
Package operations use sudo, or `--gui-auth` for a real polkit administrator
prompt. Pacman still asks before replacing the conflicting package.

The existing compositor keeps running. At a later fresh login, `session-start`
checks the executable of the actual Niri IPC peer, including an old executable
inode retained across package replacement. Only a matching patched compositor
activates the managed input capability and reloads its configuration. This avoids
sending an unsupported option to a still-running stock compositor. Explicit
`overview_ctrl_scroll = false` in user input settings remains authoritative.

`df-update` checks packages only after this component has been installed and
managed. An unchanged verified recipe/package needs neither rebuilding nor
administrator authentication. A changed pinned recipe rebuilds and verifies the
package before installation; authentication is required for package changes.
A foreign replacement or modified recovery artifact stops safely. The package's
`niri` provider/conflict prevents routine stock upgrades from silently removing
the patch. This is not automatic upstream security tracking: the pinned source
and any relevant CachyOS patches must be reviewed when updating the recipe.

Recovery commands, run as the desktop user from the owning checkout:

```sh
scripts/niri-package status
scripts/niri-package restore
```

Restore verifies the retained signature and hash, disables the managed capability,
regenerates existing theme configuration and reinstalls the exact original stock
package. It does not end the current graphical session; use a later normal login
to run stock Niri. User input settings and wallpaper/palette remain intact.
A failed replacement attempts recovery to the previous verified patched package,
or the original stock package on first installation. Interrupted transactions
retain their journal and can be retried. Package and configuration deployment
are separate transactions, not a single operating-system snapshot.

The default without a managed capability remains false for stock Niri. The
managed file is `desktop-foundation/niri-package-input.toml` under XDG config;
do not edit it manually. It supplies a default before profile/user overrides when
patched, and a compatibility ceiling when restoring stock. User preferences are
preserved rather than rewritten. `config input` reports the effective setting and
its source files.

## Deferred acceptance

Before activation, verify source compatibility and configuration reload,
up/down wheel detents, rapid repeated scrolling, zoom limits, behavior on
multiple monitors, and unchanged browser Ctrl+wheel outside overview. Touchpad
Ctrl scrolling is deliberately unchanged; the requested navigation is for mouse
wheel events. Ordinary touchpad overview gestures remain native.

## Source and license

The patch modifies [Niri's native input handler](https://github.com/niri-wm/niri/blob/8ed0da4/src/input/mod.rs)
and its configuration parser. Niri is licensed under GPL-3.0-or-later; upstream
copyright and license notices apply to the patched source.
