# EAC boot and independent recovery

The EAC profile prepares this route:

- Daily: UEFI → CachyOS-signed systemd-boot → `linux-cachyos-eac`.
- Recovery: independent Limine firmware entry → `linux-lts` or Btrfs snapshots.

Fresh `scripts/install`, `install-core` and `install-all` installations select
this profile. Existing DF installations, including older saved selections without
`boot_profile`, preserve their existing boot configuration. Select explicitly:

```sh
./scripts/install --boot-profile eac
./scripts/install --boot-profile existing
```

The boot preflight precedes package/compiler bootstrap. Desktop deployment proceeds before boot preparation. Safe firmware checkpoints
leave desktop installation complete, with boot integration pending and its
recovery journal retained; package/signature/ownership defects still fail. Rerun the same command after the requested manual
firmware action/reboot. Administrator authentication uses sudo, never a bypass.
`--clean-boot` is the legacy GRUB path and conflicts with the EAC profile.

## Inspect and resume

```sh
./scripts/boot-integrate preflight --summary
./scripts/boot-integrate preflight --admin --summary
./scripts/boot-integrate status --admin
./scripts/boot-integrate resume
```

Unprivileged inspection cannot establish ownership of protected signing keys.
It must report that limitation, not recommend clearing firmware keys from
incomplete evidence. Discovery covers firmware/ESP/root identity, public
certificate fingerprints, TPM/IOMMU, encryption measurements, GPUs, kernels,
DKMS, repository availability, Snapper, boot entries and update hooks. The JSON
report is local; nothing is uploaded.

Packages come from configured repositories in one `pacman -Syu --needed`
transaction. NVIDIA driver planning covers every retained installed kernel;
conflicting DKMS removals must be accepted in the same solver transaction as
all precompiled replacements. Unknown/mixed GPU generations or unavailable
replacements stop preparation. EAC headers are never installed. Extra DKMS-only
modules stop the default plan. An explicit `--allow-recovery-only-dkms` choice
retains them for conventional kernels without claiming they work under EAC;
this choice is saved with the installation selection. Matching headers are
planned for those conventional kernel consumers, including LTS; never for EAC.

The protected standalone helper lives at
`/usr/local/libexec/desktop-foundation-boot`. Its root journal is
`/var/lib/desktop-foundation/boot/journal.json`; original files, original ESP
kernel/initramfs assets and enrolled public variable backups are retained there.
Private signing keys are never copied. Interrupted phases retain errors and
original backups. A changed root/ESP identity stops resumption.

## Enrollment and signatures

Foundation checks the installed `eac-setup` enrollment contract, then delegates
to it interactively. It does not independently append certificates, clear keys,
use unattended enrollment, or substitute its own signing implementation.
Outside Setup Mode, PK/KEK fingerprint changes stop verification. TPM-bound LUKS
or detected BitLocker requires explicit possession of recovery credentials
(`--recovery-keys-confirmed`, also accepted by `scripts/install`) before enrollment. Other OS disks that are locked,
disconnected or not recognizable by lsblk cannot be ruled out automatically.

The upstream EAC CA must sign the EAC kernel and the dedicated loader exactly
once. Both are excluded from the local sbctl signing database. Conventional LTS
recovery images use the owner's sbctl key. Limine configuration enrollment and
snapshot synchronization use the upstream tools. Snapshot kernel references are
checked for a trusted EAC or owner signature; unsigned historical snapshot images
stop verification rather than being silently discarded or declared bootable.

EAC options are:

```text
lockdown=integrity lsm=landlock,lockdown,yama,integrity,bpf iommu.passthrough=0 intel_iommu=on module.sig_enforce=1 sysctl.kernel.kexec_load_disabled=1
```

Root/encryption options are preserved. AppArmor is intentionally excluded from
this EAC LSM policy; the installer and doctor distinguish it from a conventional
AppArmor kernel. Legacy GRUB cleanup helpers refuse EAC activation while their
existing restore paths remain available.

## Boot selection and recovery acceptance

Preparation never reboots. It reads BootNext back and records a boot-ID-bound
attempt before requesting a manual reboot. If firmware ignores it, there is no
automatic retry loop:

```sh
./scripts/boot-integrate select-order  # explicit compatibility test
./scripts/boot-integrate restore-order
./scripts/boot-integrate verify
./scripts/boot-integrate finalize
```

The compatibility path puts EAC and Limine first while preserving other entries.
Restoration matches partition/path identities, handles recreated IDs and retains
new unrelated entries. Successful diagnostic BootOrder tests restore the saved
order; `finalize` selects EAC daily only after actual EAC verification. Native
installation calls finalize only after resume verifies the running EAC boot.

Verification requires the dedicated BootCurrent path, actual systemd-boot/current
entry, running installed EAC image identity, valid signatures/modules and a fully
passing authoritative `eac-status`. Its "files ready" result on another kernel
is not success. Warnings remain failures of full acceptance.

After physically booting independent Limine/LTS and a snapshot, record their
observed boot identities using `accept-lts` and `accept-snapshot`. These commands
check kernel/root/firmware identity; they cannot prove a usable graphical login.
Acceptance is also recorded on the ESP so an overlay-backed snapshot can carry
its explicit boot proof back to the normal root.
`installation_complete` requires verified EAC daily selection. Optional physical
LTS/snapshot boot proofs independently set `recovery_verified`; without them the
phase is `installed-recovery-unverified`. They do not block desktop installation.
File readiness, verified EAC boot and recovery acceptance remain distinct.
A Btrfs snapshot alone never backs up the ESP. Upstream
`limine-snapper-sync` controls separate ESP snapshot-copy retention using
`LIMIT_USAGE_PERCENT` and `MAX_SNAPSHOT_ENTRIES`; existing settings are preserved.

## Updates and limits

Managed installation adds a pacman hook and a finite
`desktop-foundation-boot-refresh.service`. After relevant package updates it
rechecks upstream signatures, signs conventional LTS assets with sbctl, updates
the dedicated entry, syncs snapshots and verifies recovery references. It never
enrolls keys, creates firmware entries, changes boot order or reboots. A failed
refresh is recorded in the root journal/systemd logs; inspect and run `repair`.
Automatic package recovery does not roll back a running kernel or replay EFI keys.

This source integration needs physical firmware/boot acceptance before being
called production-validated. Fresh-install/reboot, ignored BootNext, encrypted
Windows/LUKS recovery, graphical LTS login and snapshot restoration are separate
acceptance tasks. No game compatibility is promised by a passing EAC status.
The implementation deliberately stops for unsupported storage/initramfs/driver
layouts and unsigned old snapshot assets instead of converting or deleting them.

Upstream references:
[CachyOS EAC](https://wiki.cachyos.org/configuration/easy_anti_cheat/) and
[Secure Boot](https://wiki.cachyos.org/configuration/secure_boot_setup/).
