# Measured boot optimization

This pass preserves the clean non-Plymouth console handoff, AppArmor, microcode,
normal/fallback images for both kernels, and every existing boot-menu entry.
No firmware setting, graphics flag, global audio latency or CPU policy is changed.

## Deploy and roll back

`scripts/install --clean-boot` includes `scripts/boot-optimize install` after
the clean-boot and greeter-console setup. On an already configured machine:

```sh
sudo scripts/boot-optimize install
sudo scripts/boot-optimize status
python3 scripts/boot-report.py --save after-1
```

The helper never reboots, restarts greetd or rebuilds an initramfs. Root files,
including `/etc/default/grub` and the generated menu, are backed up before any
write under `/var/lib/desktop-foundation/boot-optimization`. The candidate menu
must pass `grub-script-check` and retain all prior entry identities and boot
commands before it replaces the running configuration. A failed installation
restores its originals. Repeating an unchanged installation retains originals.

```sh
sudo scripts/boot-optimize restore
```

Restore this layer **before** greeter-console-setup, boot-setup or an older full
system rollback. Outside changes are rejected rather than overwritten. After a
kernel update/regenerated menu, review the saved/current menu instead of forcing
an old menu back. Firmware recommendations below have their own manual rollback.

## GRUB: one second, not a fractional timeout

Tops has GRUB `2:2.16-1.1`. Its installed and `/boot/grub` copies of
`x86_64-efi/normal.mod` match byte-for-byte. Disassembly confirms
`grub_menu_get_timeout()` calls `grub_strtoul(value, NULL, 0)` and stores an integer.
The exact upstream 2.16 parser compiled in a small standalone harness returns:

```
0.25 => 0 (error 0)
1 => 1 (error 0)
3 => 3 (error 0)
```

The menu loop decrements in 1000 ms steps. Zero bypasses drawing the menu. A
syntactically valid `set timeout=0.25` therefore does **not** provide a 250 ms
visible menu. The smallest supported positive timeout is **1 second**. Keyboard
input is polled repeatedly between ticks; a detected key cancels the countdown.
Firmware keyboard initialization still matters. No custom bootloader patch or
timer hack is introduced.

The exact base-file edit is its one `GRUB_TIMEOUT` assignment to `GRUB_TIMEOUT=1`
(Tops originally had `GRUB_TIMEOUT='5'`). All other base-file lines are preserved.
The later owned `96-desktop-foundation-timeout.cfg` sets timeout 1 and menu style
because the prior owned 90 drop-in overrode the base value with 3. New clean-boot
installations also use 1 in their 90 drop-in. Default kernel index, OS probing,
console routing and security arguments are not changed.

Sources: [GRUB 2.16 menu implementation](https://gitlab.freedesktop.org/gnu-grub/grub/-/blob/grub-2.16/grub-core/normal/menu.c),
[integer parser](https://gitlab.freedesktop.org/gnu-grub/grub/-/blob/grub-2.16/grub-core/kern/misc.c).

## Firmware findings and manual options

Tops identifies as ASUS PRIME X570-P, BIOS 5044. Firmware time over the four
recent boots is 13.900–13.998 seconds. CachyOS is already first/current in
BootOrder. The other entries include EndeavourOS, internal fallback `UEFI OS`,
another disk's `UEFI OS`, the DPDUO128GB external device, and generic optical,
removable and network entries. Similar labels do not establish redundant paths.
X-01 is attached; **no EFI entry is deleted or reordered**. The firmware boot
manager reports Timeout=1, which is not evidence of the ASUS POST delay setting.
An opaque `FastBootOption` EFI variable exists but contains a device/load path;
it does not establish whether the BIOS Fast Boot toggle is enabled. Linux cannot
attribute these 14 seconds to POST, memory training or peripheral scanning.

Manual recommendations from the board's X570 BIOS manual:

| Setting | Recommendation | Tradeoff / recovery |
| --- | --- | --- |
| Boot → Boot Configuration → Boot Logo Display | Auto | Avoid the separate minimum-one-second Post Report option shown with Disabled. No measured saving yet. |
| Same menu → Post Delay Time | 0 sec | Removes deliberate setup-entry waiting, if currently nonzero. Use Del/F2 early or OS firmware setup access. |
| Advanced → Network Stack Configuration → Network Stack | Disabled, if PXE is unused | Removes firmware network boot capability, not Linux networking or USB recovery. Current value is unknown. |
| Boot → Boot Configuration → Fast Boot | Trial Enabled | Confirm keyboard, F8 device menu and X-01 boot still work. Revert to Disabled if recovery/peripheral initialization is affected. |
| Same menu → Next Boot after AC Power Loss | Keep Normal Boot | Retains a full initialization/recovery opportunity after removing power. |
| Boot → CSM → Launch CSM | Consider Disabled only after confirming all required recovery media boot via UEFI | Removes legacy/BIOS boot compatibility. Not applied and not necessary for the software changes. |

Keep USB boot and Legacy USB Support enabled; do not choose an aggressive USB
initialization shortcut that hides X-01 or the keyboard. Keep Wait For F1 If Error
enabled. Do not change SATA mode, Secure Boot keys, memory training/overclocking
or storage controllers for this pass. The suggestions are Tops-specific; lucky38
must use its own board's manual and measurements.

Source: [ASUS X570 BIOS manual, Network Stack/USB and Boot chapters](https://dlcdnets.asus.com/pub/ASUS/mb/SocketAM4/PRIME_X570-P/E15829_PRIME_PRO_TUF_GAMING_X570_Series_BIOS_EM_WEB.pdf).

## Initramfs / graphics evidence

Four recent boots: median initrd 6.963 s, range 6.938–6.977 s. Current normal
images are approximately 49 MiB (main) / 48 MiB (LTS); fallback images 202 / 199
MiB. Firmware package 1.6.0-1 is installed and all four images were rebuilt after
its installation. The extra firmware has not produced a clear timing change in
the existing samples; these are not a controlled before/after firmware test.

Current boot evidence, keeping separate clock sources:

| Event / interval | Evidence |
| --- | --- |
| Initramfs unpack | Kernel-source 0.690–0.780 s; approximately 90 ms, overlaps other kernel work. |
| Root block device ready | Journal 1.765 s. |
| Root filesystem mounted / ready | Journal 2.637 s. Btrfs mount service takes about 223 ms. |
| Initrd default target reached | Journal 2.645 s. |
| Cleanup requested | Around journal 2.652 s. |
| AMDGPU enters logged driver initialization | Kernel-source 6.084 s. Earlier module loading is not instrumented by normal boot logs. |
| AMDGPU framebuffer ready | Kernel-source 8.233 s, initialization interval 2.149 s. |
| udev finally stops | Journal 7.924 s; reports 6.627 s aggregate CPU / 6.504 s wall time. |
| udev database cleanup | Journal 7.925–7.939 s, approximately 14 ms. |
| Switch-root request | Journal 7.954 s; root-ready-to-switch interval 5.317 s. |

Early kernel messages are buffered. `_SOURCE_MONOTONIC_TIMESTAMP` is used for
kernel intervals; journal receipt times cannot time kernel unpacking, and these
two timelines must not be subtracted from each other.

The critical initrd tail waits for udev/driver work before cleanup/switch-root.
AMDGPU finishes immediately before udev stops, but its **logged initialization
only accounts for ~2.15 seconds**, not the entire gap. A warm userspace zstd
decompression of the 5.6 MiB AMDGPU module took a median 36.5 ms over five runs;
that is not a cold-boot/kernel-module benchmark and does not explain the earlier
delay. Module verification, relocation, dependency loading and other concurrent
udev work need a boot trace to distinguish.

mkinitcpio 42.1 uses default zstd compression (`-T0`, default compression level).
Already compressed modules/firmware are placed in the early uncompressed CPIO,
avoiding double compression. No ultra/long compression overrides are present.
HOOKS are `base systemd autodetect microcode kms modconf block keyboard
sd-vconsole filesystems`. Autodetect already precedes KMS; base preserves the
rescue shell. No clearly redundant runtime hook is established.

**No initramfs change is applied.** Removing KMS risks moving the wait later and
reintroducing console resizing. Compression changes cannot credibly eliminate
a multi-second delay when unpacking itself is about 90 ms. Keep both kernels and
all four images. A deeper module-load trace is a future diagnostic, not a claimed
optimization.

## Mirror timer and Wine

The vendor mirror **timer** has both Wants/After=network-online.target. Its owned
full unit override removes only those two lines, preserving the rest of the
vendor unit. Empty dependency assignments in a drop-in do not remove these
edges; the initial deployed attempt was caught by live verification and repaired.
Future vendor schedule updates require reviewing/regenerating the owned copy.
The refresh **service** gets explicit
Wants/After=network-online.target (the vendor previously supplied After alone),
retaining DNS ordering. Calendar, random offset, persistence and refresh code
are unchanged. NetworkManager itself and other online consumers are preserved.
This targets scheduling/boot completion, not the already earlier greetd display.

Wine's only binfmt rule registers DOSWin/MZ with `/usr/bin/wine`. Four recent
boots spend 0.851–0.987 s in systemd-binfmt, median 0.951 s, before sysinit and
greetd. The program triggers the binfmt_misc automount. It also accesses/flushes
the registration filesystem even if the effective rule list is empty, so masking
Wine alone would not reliably avoid that work.

The per-file `/etc/binfmt.d/wine.conf -> /dev/null` mask disables only Wine's
kernel direct-execution handler. A lightweight ExecCondition uses native
`systemd-binfmt --tldr` to skip the service when no effective non-comment rules
remain. That native read-only mode returns before touching binfmt_misc. Other
formats, including future registrations, still enable the normal service; the
service/module is not globally masked. Install unregisters only the live DOSWin
handler; rollback re-registers Wine without flushing other live rules.

Lost: direct `./program.exe` execution through the kernel's MZ interpreter.
Preserved: `wine program.exe`, Winetricks, and the installed wine.desktop
`Exec=wine start /unix %f` file-manager action. Desktop MIME associations are not
changed. Expected saving is around 0.95 s **subject to reboot measurement**.

Sources: [systemd binfmt implementation](https://github.com/systemd/systemd/blob/main/src/binfmt/binfmt.c),
[binfmt.d override semantics](https://www.freedesktop.org/software/systemd/man/latest/binfmt.d.html).

## Quickshell profile

`python3 scripts/shell-profile.py --seconds 10 --popups` samples the live shell,
then uses an isolated copy for a QML debug trace and three open/close cycles of
launcher, clipboard, Bluetooth and power. It does not activate actions or change
settings. All test processes and temporary copies are cleaned up. Trace, logs
and reports stay in the user's cache, not Git.

Live idle sample: 0.00% one-core CPU at tick resolution; about 0.1 thread context
switches/s across ten seconds. Context switches are a scheduling proxy, not an
exact timer-wakeup count. Live initial RSS ~286 MiB / PSS ~226 MiB, with ~77 MiB
shared clean pages and ~166 MiB private dirty pages. Thus the RSS is partly
shared Qt/graphics libraries but not entirely shared memory.

Isolated debug shell: initial RSS 133 MiB / PSS 78 MiB. After all popup types
were exercised, the three closed-cycle RSS samples were 227, 230, 227 MiB;
PSS 152, 154, 151 MiB. Launcher/clipboard/Bluetooth alive flags were false and
power was closed after every cycle. Final idle CPU was again 0.00% at tick
resolution. No monotonic growth or retained popup UI is demonstrated.

The trace shows a ~62 ms inclusive root compilation span, ~20 ms clipboard
compilation, ~11 ms Bluetooth compilation, and ~8 ms unused-backend definition
compilation in this isolated run. These inclusive spans overlap and must not be
summed; debug instrumentation and a fresh temporary config affect startup.
Definitions are compiled even when instance loaders remain inactive. Root live
state is the compositor adapter plus volume surface; lazy providers belong to
their popup. Imports provide those types. Qt/LLVM/graphics and allocator caches
explain plausible retention, but exact heap attribution requires allocator-level
profiling. There is no measured justification for a rewrite, feature removal or
allocator policy change. The stale services README is corrected; runtime shell
code is deliberately unchanged.

## Measurement status

Before changes, four real boots give these medians:

| Stage | Median seconds |
| --- | ---: |
| Firmware | 13.958 |
| Loader | 3.737 |
| Kernel | 1.034 |
| Initrd | 6.963 |
| Userspace | 9.113 |
| Total | 34.809 |

Loader times range 1.694–4.392 s and include manual entry selection; do not treat
that range as a software improvement. Current session: session-open→Niri ready
699 ms, Niri-ready→shell launch 49 ms, shell-launch→QML loaded 382 ms,
Niri-launch→QML loaded 788 ms. The latest greetd service start is journal 11.090 s;
its first visible frame is not instrumented. User password-entry time is excluded
from software startup comparisons.

Deployment is verified on Tops (2026-10-02); new-boot measurements are pending.
No actual after improvement is claimed. Use `python3 scripts/boot-report.py --save after-1`
after a manual reboot, then after-2/after-3 if noisy. The collector captures all
requested stages and available session evidence, with missing visual timestamps
explicit rather than invented. Firmware changes should be tested separately from
the software changes to distinguish their effects.

Live deployment checks: timer Wants is empty and After has no network-online;
the service still Wants/After network-online. The native binfmt condition exits 1
for the empty effective rule list and DOSWin is absent. `wine --version` still
reports wine-11.18. No failed system units; only the original Quickshell process
remains. The installer reported successful candidate-menu validation, including
both kernels, four normal/fallback paths, Alpine, Arch, EFI entries including
Endeavour and external recovery, firmware setup and snapshots. The original
base file/menu backups remain in the optimization transaction. No reboot was
performed by the agent.

Validation: eight boot-policy/optimization tests (including failed-menu rollback),
four installer tests, seven session-startup tests, full `scripts/check` including
nine native Rust tests, candidate unit verification and the isolated QML trace.

## Repository files

- `scripts/boot-optimize`: root transaction, deployment, live checks and rollback.
- `scripts/boot-report.py`: read-only multi-boot/session measurements.
- `scripts/shell-profile.py`: opt-in idle/PSS and isolated QML profiling.
- `scripts/boot-setup`, `scripts/install`: one-second fresh-install policy and integration.
- `tests/test_boot_optimization.py`: preservation, time parsing and rollback coverage.
- `shell/services/README.md`: corrected description of current lazy services.
- `README.md`, `docs/boot-optimization.md`: installation, findings and recovery.
