# AppArmor

Core installation installs AppArmor and enables `apparmor.service`. Kernel
activation and profile loading are separate requirements. Check both after reboot:

```sh
cat /sys/module/apparmor/parameters/enabled
systemctl is-active apparmor.service
sudo aa-status
scripts/doctor
```

The kernel activation file must report `Y`.

## GRUB setup

When kernel activation is needed, the installer calls:

```sh
sudo scripts/apparmor-setup install
```

The helper requires kernel AppArmor support, GRUB with
`/etc/default/grub.d/*.cfg` support and the installed boot filesystem mounted at
its normal location. It writes the owned
`/etc/default/grub.d/60-desktop-foundation.cfg` drop-in and regenerates
`/boot/grub/grub.cfg`.

The LSM list is derived from installed kernel configurations. Matching headers,
`/boot/config-<release>` or the running kernel's `/proc/config.gz` provide those
configurations. Install matching headers if the required configuration is missing.
The helper checks kernel support and compatible LSM ordering before writing. Its
activation arguments apply to normal and recovery entries.

Backups and ownership metadata are stored root-only under
`/var/lib/desktop-foundation/apparmor`. Restore with:

```sh
sudo scripts/apparmor-setup restore
```

Restore recovers the original drop-in and regenerates GRUB. Reboot to apply the
restored kernel arguments. Service enablement has a separate lifecycle; use
`sudo systemctl disable apparmor.service` when removing AppArmor entirely.

## Other bootloaders

Use the base installation's kernel-command-line or UKI tools to set `apparmor=1`
and include `apparmor` in `lsm=`, preserving the other required LSMs. Regenerate
its boot artifacts and enable `apparmor.service`. Use those native tools for
systemd-boot and dracut configurations.

The optional [clean-boot setup](boot-optimization.md) has a separate journal and
also provisions AppArmor. Distribution profiles remain managed by their packages.
