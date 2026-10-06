# Boot setup and optimization

The optional clean-boot path combines three helpers: `boot-setup`,
`greeter-console-setup` when greetd is selected, and `boot-optimize`.

```sh
scripts/install-core --clean-boot
```

These helpers use the existing GRUB installation and separate root-owned journals.
Mount the installed boot filesystem before running them.

## Clean boot setup

```sh
sudo scripts/boot-setup plan
sudo scripts/boot-setup install
sudo scripts/boot-setup status
```

`boot-setup` requires GRUB drop-in support, mkinitcpio configuration and kernel
presets. It checks installed-kernel LSM support, removes Plymouth hooks and
installed Plymouth/theme packages, enables fallback images, rebuilds initramfs
images and regenerates GRUB. The generated profile selects a one-second console
menu, OS discovery, normal and recovery entries, and AppArmor activation.

An overriding mkinitcpio `HOOKS` drop-in must be reviewed first. A dracut setup
requires its native configuration tools. Before mutation, the helper requires at
least 1 GiB free in both `/boot` and `/var/lib`, plus exact cached package archives
for any installed Plymouth packages it will remove.

Original configuration, the generated menu, boot images and rollback package
archives are saved under `/var/lib/desktop-foundation/boot`. A saved transaction
is inspected with `status`, continued with `resume` or reversed with `restore`.
Complete recovery before rebooting after a failed operation.

See [AppArmor](APPARMOR.md) for kernel activation and
[console routing](greeter-console.md) for tty1/tty8 setup.

## Optimization helper

```sh
sudo scripts/boot-optimize plan
sudo scripts/boot-optimize install
sudo scripts/boot-optimize status
```

This helper requires the clean-boot drop-in and the CachyOS menu policy with
`GRUB_DEFAULT=0`, a visible menu and
`GRUB_TOP_LEVEL=/boot/vmlinuz-linux-cachyos`. It applies these changes:

- Set the base GRUB timeout and the owned timeout drop-in to one second.
- When the CachyOS mirror timer is present, preserve its schedule while moving
  network-online dependencies to the mirror service.
- When the expected Wine binfmt rule is present, mask direct execution of Windows
  binaries and skip systemd-binfmt when the effective rule list is empty. Launch
  Windows programs through `wine`.

A candidate GRUB menu must pass syntax validation and retain existing entry
identities and boot commands before publication. The helper checks expected vendor
files and reports conflicting administrator overrides. Backups are stored under
`/var/lib/desktop-foundation/boot-optimization`.

## Restore and measure

Restore only the layers installed, in reverse order:

```sh
sudo scripts/boot-optimize restore
sudo scripts/greeter-console-setup restore
sudo scripts/boot-setup restore
```

Owned files are checked before restoration. Review a menu regenerated after a
kernel update before restoring an older transaction. After completing all layers,
regenerate GRUB if installed kernels changed since the saved menu.

Collect boot stages and session timestamps with:

```sh
python3 scripts/boot-report.py --boots 4
python3 scripts/boot-report.py --boots 4 --save before
```

Saved reports are placed under `~/.cache/desktop-foundation/boot-audit/optimization`.
Kernel source timestamps and journal receipt timestamps are separate timelines.
GRUB menu interaction affects loader duration; greetd's service-start timestamp
measures service launch. See [performance](performance.md) for measurement tools.
