# Installation

Desktop Foundation supports CachyOS with Niri. Start with a normal
login user, sudo, network access, working graphics, Git and Python. Keep the
checkout in a permanent directory and run the installer as that user. Installer
orchestration is native Rust; Python remains necessary for the not-yet-migrated
privileged helpers.

Malformed arguments and `--help` are handled before bootstrapping or building.
Do not run the installer with sudo: a mutable root invocation is rejected before
bootstrap/build, and the normal-user workflow requests authentication for its
privileged steps. The installer runs its selected doctor checks before reporting
success; no separate validation command is needed to finish installation.

A normal installation bootstraps missing build dependencies and builds the release
backend automatically. A normal rerun rebuilds the selected checkout before
starting its installer, so an older executable cannot omit newly added components.
Inspection reuses the existing executable without building. It verifies any existing deployment owner before running
package setup. Inspection (`--dry-run`, `--plan`, `--check`) never installs packages
or builds the backend; run `scripts/bootstrap` and `scripts/build-backend` first if
you want to use those inspection options on a completely fresh checkout.

```sh
scripts/install-core --dry-run
scripts/install-core
```

The default installation provides the Niri desktop, Quickshell and terminal
configuration. It builds both Rust backends and the pinned patched Niri package, retains a signed
stock-compositor recovery package, deploys configuration and enables
Bluetooth and AppArmor. Existing application associations are left alone. When no display-manager service is configured, the installer provides greetd
with tuigreet and a plain background. An existing configured manager (including
SDDM, GDM, LightDM or greetd) is preserved automatically. An explicit
`--greeter` cannot replace another manager and is rejected before native package
or deployment actions. `--no-greeter` always preserves the login setup. The keyring uses oo7 with PAM password unlocking through
greetd. Choose Niri at login. `scripts/doctor` remains available for later
troubleshooting. See [keyring setup](keyring.md).

Use Standard to add Brave Origin Nightly, paru, ChatGPT and Vesktop without
changing application defaults:

```sh
scripts/install --standard --dry-run
scripts/install --standard
```

Add supported extras explicitly, for example:

```sh
scripts/install --standard --extra steam --extra spicetify
scripts/install --minimal --extra geode
```

Geode also selects Steam and may remain pending until Geometry Dash has been
installed and launched once. The currently selectable extras are `steam`,
`spicetify`, `geode` and `recording`; Odysseus continues to use its existing manual setup.
The minimal and standard paths do not automatically install the optional
firmware bundle. `--clean-boot` and the legacy personal installer retain it.

Successful installations save their selected layer, extras, hardware profile
and greeter choice in `$XDG_STATE_HOME/desktop-foundation/installation.toml`.
Running `scripts/install` again reuses them; an explicit layer replaces the
saved layer and extras. `--no-packages` and `--dry-run` are one-time options.

The legacy personal installer remains available:

```sh
scripts/install-all --dry-run
scripts/install-all
```

`scripts/install` defaults to Minimal on a new installation and reuses the last
saved selection afterward; `scripts/install-all` selects the legacy personal
profile and includes GPU Screen Recorder and FFmpeg. The recorder, `ffmpeg` and
`ffprobe` are checked when the feature is selected, including check/deploy-only
modes. A missing tool stops the install before configuration deployment, rather
than surfacing after a replay has been captured. See [personal applications](personal-apps.md) for package sources and setup.

| Option | Purpose |
| --- | --- |
| `--dry-run` or `--plan` | Show packages, application roles, deployment paths and ownership conflicts |
| `--check` | Inspect the selected installation layer |
| `--no-packages` | Build and deploy using installed dependencies; leave compositor packages alone |
| `--stock-niri` | Save a stock-compositor preference; restore a managed patched installation to its signed original |
| `--patched-niri` | Build/install patched Niri (default for every profile) |
| `--no-greeter` | Always preserve the login setup; automatic detection already preserves configured managers |
| `--hardware-profile NAME` or `--profile NAME` | Select a directory under `profiles/`; default is `default` |
| `--greeter` | Explicitly configure greetd; another configured manager is never replaced |
| `--clean-boot` | Apply the GRUB/mkinitcpio cleanup and boot optimization helpers |

The installed Niri must accept the generated KDL, including its blur and
background effects. The installer checks that configuration before deployment.
Greeter setup checks tuigreet's None background before writing login files. Existing display-manager configuration is detected through systemd's
`display-manager.service` alias. Broken aliases and custom unit files are preserved
rather than treated as permission to replace login. Existing PAM settings remain
untouched when its manager is preserved; keyring auto-unlock then depends on that
manager's existing PAM integration.

The default profile uses automatic outputs and Finnish input. Shared input lives
in `config/input.toml`; Niri output overrides belong in `profiles/NAME/niri.kdl`.
The `dual-display-example` profile shows an explicit output layout.

The running graphical session is never restarted to activate a package. The next
normal login enables Ctrl+wheel overview zoom only after verifying the patched
Niri IPC peer. See [Niri package integration and recovery](../patches/niri/README.md).

## Reinstall and update

Use `df-update` for Desktop Foundation updates. Managed patched Niri is checked
and rebuilt when its pinned recipe changes, with authentication for package changes;
unmanaged compositors and other system packages are left alone. It reuses the compositor session and hardware profile and preserves the current
wallpaper, palette and user preferences. `df-update --dry-run` checks the local
prerequisites without fetching or deploying. See [updating](updating.md).

Keep the checkout and `$XDG_STATE_HOME/desktop-foundation` state directory. Inspect
the chosen revision and run the same installer's dry-run before reinstalling.
Missing packages are installed with a full pacman upgrade. AUR recipes are built
as the login user and presented for review.

Kitty, Fish and Fastfetch configuration directories are managed as complete units.
Their originals and other replaced paths are saved in the deployment journal.
MIME defaults have a separate journal for the designated application-role keys.
Rerunning preserves the original backups. An externally replaced managed path or
owned MIME key must be resolved before installation can continue.

After a reported error, inspect the named path or helper journal, resolve the
cause and rerun. Keep the journals and backups available for recovery. Package
installation and the later configuration steps have separate recovery paths.

## Boot and login integration

AppArmor requires kernel activation as well as profile loading. If activation is
needed, the installer uses the supported GRUB helper. Other bootloaders require
their native kernel-command-line tools; see [AppArmor](APPARMOR.md).

`--clean-boot` requires an existing GRUB setup with drop-in support, mkinitcpio
presets and the CachyOS main-kernel menu policy required by `boot-optimize`.
Mount the installed boot filesystem first. A dracut installation needs its own
native boot configuration. See [boot setup and optimization](boot-optimization.md)
for changes, prerequisites and restore order.

Greetd and PAM configuration are backed up under
`/var/lib/desktop-foundation/system`. Login changes take effect at the next boot.
The clean-boot path also separates boot output on tty8 from the greeter on tty1;
see [console routing](greeter-console.md).

## Restore

```sh
scripts/uninstall
scripts/uninstall --greeter
```

The first command restores managed user configuration and preferences. The second
also restores greetd/PAM files. Packages remain installed. Spotify's optional
links, configuration and tool backup use `scripts/spotify-setup restore`.
Privileged boot helpers have separate restore commands. Follow
[recovery](recovery.md) when restoring an interrupted deployment or boot setup.
