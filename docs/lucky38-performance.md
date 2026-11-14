# Lucky38 performance profile

Performance policy is opt-in and independent of Minimal/Standard/Personal selection.
Only Lucky38's verified TUF B450-PRO GAMING / 5700X3D / RTX 3060 hardware is covered.
Tops, firmware changes and NVIDIA module ordering are outside this pass.

Install in one normal DF transaction:

```sh
./scripts/install --standard --profile lucky38 --performance-profile lucky38
```

Existing installation:

```sh
./scripts/performance-profile plan
./scripts/performance-profile enable lucky38
# Optional graphical administrator authentication:
./scripts/performance-profile enable lucky38 --gui-auth
./scripts/performance-profile status
./scripts/performance-profile verify --gui-auth # authoritative eac-status, read-only
./scripts/performance-profile disable
```

The explicit selection lives in the deployment owner's state as
`performance-profile`. Installation and `df-update` deploy paths reconcile that
selection automatically. An unchanged effective policy requires no new privilege.
A changed/missing privileged helper or policy needs authenticated repair; updates
never open a password prompt, overwrite foreign files, or silently claim repair.
They report pending administrator work in the updater output and retain the
selection/journal. `status` returns exit code 3 for a selected but unreconciled
policy. Policy upgrades replace only unchanged DF-owned files, preserving the
original-default rollback across interruptions. Re-run
`enable lucky38` to finish it. Profile failures do not undo a successful desktop
update or claim the performance policy itself was installed.

The standalone root-owned helper is installed at
`/usr/local/libexec/desktop-foundation-performance`; its journal is under
`/var/lib/desktop-foundation/performance-lucky38`. It records hardware/root
identity and file hashes, rejects administrator changes and changed Wine vendor
contracts, and supports restoration of interrupted operations. The initial file
set must be absent, so rollback removes only the profile's own two files. It
never backs up or overwrites preexisting administrator binfmt rules. Run disable
before uninstalling DF if the system-level optimization is no longer wanted.

## What changes

- Mask only Wine's automatic kernel `MZ` registration by its documented per-file
  `/etc/binfmt.d/wine.conf -> /dev/null` override. Explicit `wine program.exe`,
  desktop Wine launchers and Proton retain their normal executable paths. Direct
  shell `./program.exe` requires explicit Wine after the next boot. Wine itself
  is not required or installed by this profile; the opt-out also covers a later
  Wine installation. Current live
  handlers are deliberately left alone; there is no binfmt flush or service restart.
- A scoped `systemd-binfmt.service` ExecCondition skips initialization only when
  the effective binfmt configuration has no rules. New non-Wine handlers continue
  to run. Query errors fall back to normal initialization. This is extracted from
  legacy GRUB tooling; neither the profile nor restoration touches boot entries,
  initramfs, signatures, kernels, TPM/IOMMU, GPU options or recovery images.
- General DF reliability/performance fixes apply through ordinary source updates
  on all hardware: clipboard initialization reuses an unchanged validated private
  projection generation; Control Center coalesces cache writers over bounded
  250-ms windows, while confirmed UI state updates immediately. These do not need
  a privileged hardware profile or a permanent optimization daemon.

Clipboard generation reuse checks the database and projection identities,
ctime/mtime/nanoseconds, ownership, private permissions and schema-versioned
metadata. It still opens/migrates/validates SQLite schema under the existing
history lock. Missing/corrupt caches, database/image/index changes, WAL mode and
changed referenced-file metadata use the normal full validation/export path.
Referenced paths are statted without reading their contents; large reference sets
fall back to full validation rather than unbounded metadata caching. Existing
stored bundles remain authoritative. An absent optional cache never prevents use;
rollback to an older DF simply ignores its metadata. No clipboard payloads are
written to diagnostics. Projection semantics changes must bump the generation
cache version.

Cache coalescing persists the latest usable audio/network observations, not desired
settings. A shell exit during the coalescing window may lose the latest historical
observation on disk; live services remain authoritative after restart. No audio
routing, volume commit or network action is delayed by cache persistence.

## Readiness and comparisons

`startup-report` now accepts SDDM and greetd session-open evidence, Niri's notify
and socket milestones, QML Configuration Loaded, and mapped Qt frame submission.
Shared `FrameReadiness` observes the real `Window.frameSwapped` signal in existing
clock, popup, volume and Control Center windows. It adds no hidden window and no
polling. A hidden/unopened surface has no first-frame marker. Frame submission is
**not compositor presentation feedback**, animation completion or proof that the
entire desktop is usable. Cold-login markers require the next login; a source
reload is not a cold-login comparison. Initial-configuration frame metrics and
the latest reload's configuration-to-frame interval are reported separately;
a reload cannot fill missing cold-login evidence.

`boot-report` records NVIDIA module announcement, DRM/framebuffer and NVMe discovery
using kernel source timestamps, separately from journal receipt times. Current
binfmt unit start/exit/invocation metadata is retained alongside journal intervals
so clock/invocation mismatches remain inspectable. Neither NVIDIA interval ordering
nor a loader timestamp by itself proves a causal bottleneck.

Pin source revision, binary SHA-256/build-info/source digest and boot ID before
comparing. Keep private clipboard fixture contents outside reports and delete
fixtures after measurement. Compare release binaries, unchanged warmed fixtures
and the same event sequence. A next-boot time reduction is unverified until an
actual comparable boot; the old measured ~0.98-second binfmt stage is an opportunity,
not an achieved saving.

## NVIDIA memory clearing: decision and trade-offs

`/etc/modprobe.d/nvidia.conf` shadows CachyOS's same-named vendor file. Effective
`InitializeSystemMemoryAllocations=1` is NVIDIA's clearing-enabled default, even
though CachyOS's shadowed file suggests 0.

| Policy | Benefit | Cost |
|---|---|---|
| Keep 1 (this pass) | Clears driver/GPU system-memory allocations to prevent exposing potentially sensitive previous contents; preserves current behavior | Memory clearing can consume CPU/memory bandwidth; no Lucky38 penalty has been quantified |
| Set 0 | May reduce allocation work on a trusted single-user setup | Removes that clearing protection; gain is unmeasured and workload/driver dependent |

This is system-memory allocation clearing, distinct from VRAM preservation for
suspend/recording. EAC boot signatures do not make memory-disclosure trade-offs
irrelevant, and no claim is made that this option alone passes or fails EAC.
DF does not rename the local file, copy the vendor option, disable clearing or
rebuild initramfs automatically. Future investigation can compare it only after
an explicit policy decision; no broad vendor-file merge should erase DRM,
VRAM-preservation or nouveau settings.

References: [NVIDIA's option definition](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/main/kernel-open/nvidia/nv-reg.h),
[CachyOS's actual setting and warning](https://github.com/CachyOS/CachyOS-Settings/blob/master/usr/lib/modprobe.d/nvidia.conf),
[kmod same-name precedence](https://man.archlinux.org/man/modprobe.d.5.en),
[binfmt.d per-file masks](https://www.freedesktop.org/software/systemd/man/latest/binfmt.d.html),
[Qt frameSwapped semantics](https://doc.qt.io/qt-6/qquickwindow.html#frameSwapped),
[CachyOS EAC requirements](https://wiki.cachyos.org/configuration/easy_anti_cheat/).
