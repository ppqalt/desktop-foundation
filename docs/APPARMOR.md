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

## Portable kernel derivation and approved boot cleanup

The AppArmor installer now reads each installed kernel's configuration (matching
headers, `/boot/config-<release>`, or the running `/proc/config.gz`). It preserves
its default LSM ordering, inserts AppArmor before BPF, and refuses missing support,
conflicting major LSMs or inconsistent installed-kernel defaults. Matching headers
may be required on a fresh installation. Activation arguments now apply to recovery
entries too. An explicit existing LSM list remains preserved by AppArmor-only setup.

For the approved complete target use `sudo scripts/boot-setup plan`, then
`sudo scripts/boot-setup install`, or fresh installation `scripts/install --clean-boot`.
This explicitly selects a three-second console GRUB menu, preserves OS discovery
and unrelated kernel arguments, removes installed Plymouth/theme packages, removes
its initramfs hook, enables fallback images and rebuilds/checks boot artifacts.
It preserves the base distribution's main-kernel preference; it never inserts a
Tops hostname, GPU, UUID or disk identifier into shared configuration. GRUB and
mkinitcpio are required; unsupported/custom configurations stop for review.

Original configs, generated GRUB, boot images and exact cached rollback packages
are saved root-only in `/var/lib/desktop-foundation/boot`. Exact cached package
versions and sufficient space are required BEFORE any live mutation. A second
transaction refuses to overwrite these originals. Rollback is
`sudo scripts/boot-setup restore`; it reinstalls saved packages and restores saved
configs/images. No automatic reboot or login-manager change. After any failure,
do not reboot until rollback or artifact repair succeeds. Retain the backup through
successful main/LTS boots. Full target reasoning: [boot proposal](boot-security-proposal.md).

AppArmor activation is not blanket application confinement. Distribution restrictive
profiles remain maintained by their package. Chromium/Electron unconfined userns
stubs remain compatibility policy, not a claim of browser isolation. No speculative
custom browser/terminal/compositor policy is installed by this command.

Interrupted installation: `sudo scripts/boot-setup status` collects a read-only
transaction/artifact report. `sudo scripts/boot-setup resume` continues only a
prepared/failed transaction with matching original/managed file hashes and intact
rollback artifacts. Original backups are never replaced. Build/package output is
streamed, so long fallback builds show progress. Do not interrupt/reboot during
rebuilds. Once complete, collect status again before rebooting.

The bootstrap also installs tracked `packages/firmware.txt` (`mkinitcpio-firmware`)
to supply firmware used by broader fallback images. CachyOS uses its repository
package; Arch/EndeavourOS without a repository candidate uses paru/AUR with the
existing PKGBUILD review flow. Builds remain non-root. Already installed packages
are skipped. This is a fresh-install dependency, not a resident service.
