# AppArmor installation

The core manifest explicitly installs AppArmor. `scripts/install` enables its
service and calls `sudo scripts/apparmor-setup install`. This provisions the
tracked GRUB drop-in with a write-ahead backup and regenerates GRUB's existing
`/boot/grub/grub.cfg`. It never reinstalls GRUB or edits disk partitions.
Mount the installed boot filesystem before installation.

Only GRUB versions sourcing `/etc/default/grub.d/*.cfg` are automated. Other
bootloaders stop with an actionable error before boot files are changed. For
systemd-boot/UKIs, configure `apparmor=1` and include `apparmor` in the `lsm=` list
using the base installation's kernel command-line/UKI tooling, preserving other
LSMs; regenerate its boot artifacts, then enable `apparmor.service`. Do not run
the GRUB helper against a systemd-boot installation. Automatic support remains a
fresh-install blocker for that bootloader.

After reboot, `scripts/doctor` requires `/sys/module/apparmor/parameters/enabled`
to report `Y`. Package installation and an enabled service alone are insufficient.
The pre-login installation check warns about pending boot enablement.

Rollback: `sudo scripts/apparmor-setup restore` restores/removes only its owned
drop-in and regenerates GRUB; reboot is needed. Changed files are protected from
overwrite. Service enablement is retained intentionally; disable it explicitly
if removing AppArmor entirely. No local AppArmor overrides existed on Tops at
this audit, so none are fabricated or disabled. Distribution profiles stay owned
by their package. New overrides must be tracked and reviewed here before use.

Current Tops was audited with AppArmor disabled in the running kernel. This pass
does not apply the boot helper to Tops; that is a separate later task.

Kernel activation and profile loading are separate requirements; see the
[upstream kernel documentation](https://docs.kernel.org/admin-guide/LSM/apparmor.html).
