# Installation

Desktop Foundation targets Arch-family systems with pacman. Start with a normal
login user, sudo, network access, working graphics, Git and Python. Keep the
checkout in a permanent directory and run the installer as that user.

```sh
scripts/install-core --dry-run
scripts/install-core
```

Core installs the Niri desktop, Quickshell, terminal configuration and default
applications. It builds both Rust backends, deploys configuration, applies app
roles and enables Bluetooth and AppArmor. The default greeter is greetd with a
Matrix-capable tuigreet. Choose Niri at login, then run `scripts/doctor`.

For Spotify, ChatGPT and Steam, use the full installer:

```sh
scripts/install-all --dry-run
scripts/install-all
```

`scripts/install` is an alias for the full installation. `install --core` selects
core. See [personal applications](personal-apps.md) for package sources and setup.

| Option | Purpose |
| --- | --- |
| `--dry-run` or `--plan` | Show packages, application roles, deployment paths and ownership conflicts |
| `--check` | Inspect the selected installation layer |
| `--no-packages` | Build and deploy using installed dependencies |
| `--no-greeter` | Use an existing display manager or TTY login |
| `--hardware-profile NAME` or `--profile NAME` | Select a directory under `profiles/`; default is `default` |
| `--clean-boot` | Apply the GRUB/mkinitcpio cleanup and boot optimization helpers |

The installed Niri must accept the generated KDL, including its blur and
background effects. The installer checks that configuration before deployment.
Greeter setup checks tuigreet's Matrix support before writing login files. A
system with another display manager should use `--no-greeter`.

The default profile uses automatic outputs and Finnish input. Shared input lives
in `config/input.lua`; Niri output overrides belong in `profiles/NAME/niri.kdl`.
The `dual-display-example` profile shows an explicit output layout.

## Reinstall and update

Keep the checkout and `$XDG_STATE_HOME/desktop-foundation` state directory. Inspect
the chosen revision and run the same installer's dry-run before applying it.
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
