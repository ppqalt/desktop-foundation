# Boot and AppArmor proposal — inspected 2026-10-02

Proposal only. No boot configuration, policy, service state, packages or login
configuration changed. The TTY experiment was cancelled; greetd/tuigreet and its
working PAM/keyring integration remain. Private raw audit evidence stays outside Git.

## AppArmor: actual state and target

Running CachyOS 7.2.8-2 and installed CachyOS LTS 6.18.52-1 both compile
`CONFIG_SECURITY_APPARMOR=y`, but their `CONFIG_LSM` is
`landlock,lockdown,yama,integrity,bpf`. Runtime securityfs reports
`capability,landlock,lockdown,yama,bpf`; AppArmor's enabled parameter is `N`.
`apparmor.service` is enabled but inactive: ConditionSecurity=apparmor failed.
There are no loaded enforcing or complain profiles because the AppArmor security
filesystem is absent. Package policy files on disk are not active policy.
`apparmor` 4.1.7 is installed; local/ contains only README, disable/ is empty.

Recommended kernel arguments, derived from both installed kernel configurations:

```
apparmor=1 lsm=landlock,lockdown,yama,integrity,apparmor,bpf
```

Preserve the compiled ordering and insert AppArmor before BPF. Capability is
implicitly registered, not an extra command-line item. Integrity's absence from
runtime names is not a reason to delete the kernel's configured component. Do not
claim that choosing this list alone enables IMA measurements or BPF policies.
AppArmor must load actual restrictive profiles to provide confinement.
[Kernel AppArmor documentation](https://docs.kernel.org/admin-guide/LSM/apparmor.html).

Put these arguments in `GRUB_CMDLINE_LINUX`, so recovery entries receive them too.
Keep apparmor.service enabled. After reboot require enabled=Y, AppArmor in the
active LSM list, a successful service start and `sudo aa-status` listing real
profiles. Check `/proc/<pid>/attr/current` on each intended confined executable;
service success does not prove that path attachments match.

### Recommended policy scope

| Category | Programs | Recommendation |
|---|---|---|
| Worth enforcing | Avahi; ping; unix_chkpwd | Use installed restrictive package profiles. Avahi is a running network-facing daemon with a bounded task. Check attachment and logs after activation. |
| Upstream/package profile only | Other system daemons with maintained profiles; Chromium/Electron user-namespace compatibility profiles | Retain distribution maintenance. Do not manufacture profiles for absent daemons or treat unconfined compatibility stubs as isolation. |
| Development/complain first | Brave Origin Nightly, then Spotify if meaningful policy is available | High-risk network/content workloads justify investigation, but downloads, file pickers, portals, GPU and browser sandbox need a tested policy. No home-wide write grants as a shortcut. |
| Not worth custom profiling in this pass | Kitty/Fish, Niri, Quickshell, PipeWire/WirePlumber, Mako, swaybg, Xwayland-satellite, clipboard and Nothing/Bluetooth helpers | Shell/terminal intentionally execute arbitrary user commands; compositor/audio/shell components need broad session access. Tiny helpers are on-demand and custom policies add maintenance with uncertain isolation value. Keep their existing privilege boundaries. |

The installed `/etc/apparmor.d/brave` explicitly uses `flags=(unconfined)` and
matches `/opt/brave.com/brave{,-beta,-nightly}/brave`. Actual Origin Nightly launches
`/opt/brave.com/brave-origin-nightly/brave-origin`: it does not match that rule.
Likewise the ChatGPT package profile is an unconfined userns compatibility stub.
Neither is a restrictive browser policy. Local includes cannot turn an existing
unconfined profile into a meaningful allowlist or change its attachment header.
A future restrictive Origin policy needs a separately named, precisely attached
profile, developed in complain mode and promoted only after real workflows pass.
Leave Chromium's own sandbox enabled. Do not set a global unprivileged-userns ban
or AppArmor userns-restriction sysctl in this pass: missing attachments can interfere
with browser sandbox operation. Upstream describes the purpose of these stubs in
[its Chromium userns change](https://gitlab.com/apparmor/apparmor/-/merge_requests/1238).

Proposed repository layout for policy additions: `security/apparmor/profiles/`
for owned profiles, `security/apparmor/local/` for package-supported local includes.
Deploy to `/etc/apparmor.d/desktop-foundation.<name>` and corresponding local/
paths with original backups, checksum ownership and explicit restore. Do not edit
package profiles, install a bulk third-party policy collection, or run aa-logprof
and automatically accept every requested access. Complain is only for the specific
new profile, not all existing policy. No custom policy is proposed as ready to enforce.

### Fresh installations and rollback

Arch/CachyOS/EndeavourOS: install `apparmor`, enable its service, inspect the chosen
kernel's supported/default LSMs, preserve them and add AppArmor. No additional
package is required on this machine. Existing `scripts/apparmor-setup` is GRUB-only
and has ownership/backups; its fixed fallback LSM list should become derived from
the target kernel before claiming broad portability. Move activation args to the
common kernel-argument setting when integrating the target below; avoid duplicate
boot argument owners. Non-GRUB installs need their own kernel/UKI configuration and
rebuild path, not a forced GRUB install. Doctor should verify profile loading and
attachment as well as enabled=Y. These are proposed installer improvements, not
implemented by this audit.

For boot recovery, edit the GRUB entry temporarily to remove the AppArmor LSM
selection or add `apparmor=0`; then restore the saved configuration and regenerate
GRUB. For one problematic profile, disable/unload that owned profile deliberately,
restore its backup and restart the affected application; do not unload all policy.
The existing helper's restore retains service enablement by design.

## GRUB: observed configuration

Five-second visible menu, default index 0, CachyOS graphical theme, submenus enabled,
recovery entries disabled, os-prober enabled, CachyOS main kernel explicitly preferred.
The normal kernel suffix is `nowatchdog nvme_load=YES splash loglevel=3`.
No current resume, AMD-specific, mitigation-disabling, scheduler or rd.* parameters.
Root uses a UUID and Btrfs `rootflags=subvol=@`; preserve both.

Generated entries include current CachyOS and LTS, Alpine, Arch on another disk,
UEFI firmware settings, EFI BootNext entries (including EndeavourOS), custom X-01
chainloading, and conditional Btrfs snapshots. EndeavourOS has a firmware entry
rather than a direct os-prober Linux entry here. A label alone cannot prove an
installation/firmware entry is stale; do not delete any in this pass. The generated
CachyOS-self BootNext entry looks redundant; leave it until its firmware path is
verified. Keep the X-01 UUID and chainloader untouched.

### Recommended explicit values

```sh
GRUB_DEFAULT=0
GRUB_SAVEDEFAULT=false
GRUB_TIMEOUT=3
GRUB_TIMEOUT_STYLE=menu
GRUB_DISABLE_OS_PROBER=false
GRUB_DISABLE_SUBMENU=false
GRUB_DISABLE_RECOVERY=false
GRUB_TOP_LEVEL=/boot/vmlinuz-linux-cachyos
GRUB_CMDLINE_LINUX="apparmor=1 lsm=landlock,lockdown,yama,integrity,apparmor,bpf"
GRUB_CMDLINE_LINUX_DEFAULT="loglevel=4"
GRUB_TERMINAL_INPUT=console
GRUB_TERMINAL_OUTPUT=console
GRUB_THEME=""
GRUB_BACKGROUND=""
```

Three seconds saves two seconds while keeping an immediately visible multi-boot
menu. Index zero remains predictable with the existing explicit main-kernel priority;
verify the generated first entry before reboot. No remembered accidental LTS/other
OS selection. Console GRUB removes the CachyOS theme; blank theme/background
prevents carrying old graphics. `loglevel=4` retains warnings/errors without quiet
or splash; systemd may still show useful progress. This is not a performance claim.
[GRUB configuration semantics](https://www.gnu.org/software/grub/manual/grub/html_node/Simple-configuration.html).

Effective normal CachyOS kernel line (UUID intentionally not copied into Git):

```
root=UUID=<existing-root-UUID> rw rootflags=subvol=@ apparmor=1 lsm=landlock,lockdown,yama,integrity,apparmor,bpf loglevel=4
```

Remove `nvme_load=YES`: no consumer was found in current initcpio/config and NVMe
modules already exist in both images. Do not assert that it is a documented kernel
optimization; unknown undotted parameters can become init environment variables.
Remove `nowatchdog`: no measured need for disabling lockup detection here. Retain
default CPU mitigations, AMD GPU/CPU driver behavior, scheduler and power settings.
No hibernation/resume args with the current zram-only swap configuration. Keep KMS
and AMD microcode hook: Plymouth removal does not justify removing either.
[Kernel argument handling](https://docs.kernel.org/admin-guide/kernel-parameters.html).

### Kernel and recovery strategy

Retain both installed kernels and headers. Both presets currently generate only
default images; the fallback definitions are commented out. Enable
`PRESETS=('default' 'fallback')`, each existing fallback_image path, and
`fallback_options="-S autodetect"` in both presets. Rebuild all presets and check
free /boot space first. A fallback image includes broader drivers; LTS is a separate
kernel choice. Neither substitutes for the other. Preserve snapshots and a recovery
USB. Generated recovery entries use `single` and require functioning rescue/PAM
credentials; verify that rather than assuming a root shell will be accessible.
No GRUB reinstall, partition changes, EFI entry deletion or firmware changes.

## Exact source and proposed full splash removal

Confirmed: `/etc/plymouth/plymouthd.conf` selects `cachyos`, provided by
`cachyos-plymouth-theme`. Its two-step plugin/theme/assets and Plymouth binaries,
configuration and unit links are embedded in BOTH default initramfs images.
The `plymouth` mkinitcpio hook adds those pieces; `splash` requests the display.
Boot journal confirms plymouth-start and plymouth-quit ran. Installed
`cachyos-plymouth-bootanimation` provides a different theme and is not the configured
one. Both theme packages depend on plymouth; no other installed reverse dependency
was listed. GRUB's `cachyos-grub-theme` is separate from the kernel splash.

Proposed ordered implementation, for a later authorized pass:

1. Save original GRUB defaults/drop-ins, generated grub.cfg, mkinitcpio config,
   both presets, theme config and package list in a root-only rollback directory.
   Save known-good boot images to separate storage; ensure room for fallback images.
2. Set the GRUB values above, preserving generated root/subvolume and other OS
   entries. Reconcile the AppArmor helper drop-in so it does not duplicate/override
   these settings. Remove only `plymouth` from HOOKS, leaving:
   `base systemd autodetect microcode kms modconf block keyboard sd-vconsole filesystems`.
   Enable the fallback preset definitions described above.
3. Remove exactly these splash packages together, without recursive dependency
   cleanup: `pacman -R cachyos-plymouth-bootanimation cachyos-plymouth-theme plymouth`.
   Review the transaction first and stop if new dependents appear. Removing the
   packages removes their units/hooks/plugins; do not add permanent masks for units
   that no longer exist. Optionally remove `cachyos-grub-theme` after blanking its
   reference; retain it if another installation/config references it.
4. Run `mkinitcpio -P`, require successful builds for both kernels and fallback
   images, then `grub-mkconfig -o /boot/grub/grub.cfg` and
   `grub-script-check /boot/grub/grub.cfg`. Package hooks may also rebuild; explicitly
   check final artifacts. Do not reboot on an error. Run `systemctl daemon-reload`
   to refresh removed system units; leave the current graphical session running.
5. Inspect every new image with lsinitcpio: no Plymouth binaries/theme/unit links.
   Inspect generated entries for both kernels/fallbacks/recovery, AppArmor args,
   correct root/subvolume and retained Alpine/Arch/EndeavourOS/X-01 access.
6. Reboot once, confirm actual AppArmor loading/attachments, useful output, no
   Plymouth startup, and normal greetd → Niri operation. Keep backups until both
   main and LTS paths have booted successfully.

Rollback restores configs/presets, reinstalls the exact three splash packages and
restores the theme selection, rebuilds initramfs and GRUB, and checks both before
reboot. If unable to boot, use the saved images or recovery USB/chroot with root and
ESP mounted correctly. Restore known-good generated grub.cfg when generation fails.
Disabling splash alone is not the proposed removal, and firmware vendor graphics
may still appear before GRUB; Plymouth removal does not alter firmware visuals.

No timing improvement is claimed for these unimplemented changes. Keep PSD,
portals, keyring, GVFS, other installations and login configuration unchanged.
