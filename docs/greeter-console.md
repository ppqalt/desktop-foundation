# Non-Plymouth console handoff

Confirmed on Tops after the approved boot cleanup: greetd owns tty1; stock unit
conflicts with getty@tty1 (autovt is an alias), which is inactive. greetd starts at
11.43 s after kernel launch, NetworkManager wait-online finishes at 17.48 s.
There is no explicit console= setting; `/proc/consoles` names tty0, which follows
the foreground VT. Thus late PID1 boot status reaches the same VT as tuigreet.
Plymouth removal exposed this existing output route. loglevel=4 controls kernel
verbosity, not PID1's [ OK ] messages. Delaying greetd for network-online would
add approximately six seconds; Type=idle is bounded and does not establish
exclusive console ownership. Neither is used as a substitute for routing.

Framebuffer is 1920x1080 and tty1 is 240 columns by 67 rows, consistent with an
8x16 console font. No separate sizing fault established; the photograph alone
cannot prove a geometry problem. Recheck centering after removing concurrent writes.

`scripts/greeter-console-setup install` adds two owned drop-ins:

- `console=tty8` on common GRUB kernel arguments routes kernel and PID1 console
  output away from tty1, including recovery entries. Useful output remains accessible
  through Ctrl+Alt+F8 and the journal; no quiet/show_status suppression. Other OS
  kernel arguments are not changed. Explicit serial or custom console settings stop
  installation for review rather than being discarded.
- greetd.service gets StandardInput=tty, TTYPath=/dev/tty1, TTYReset=yes,
  TTYVHangup=yes and TTYVTDisallocate=yes. Terminal setup/reset and clearing happen
  at the ownership boundary; normal session PAM, VT number and application remain.
  Daemon output remains in the journal. No framebuffer resolution is hardcoded.

No active greetd restart. Backups of these paths and generated GRUB are root-only
in /var/lib/desktop-foundation/greeter-console. Restore:
`sudo scripts/greeter-console-setup restore`, then reboot. If reverting all boot
changes, restore this layer BEFORE `scripts/boot-setup restore`, so the older
transaction can verify its GRUB checksum. Restore returns its saved generated
GRUB; if kernels changed since the backup, regenerate GRUB before reboot.

Fresh `scripts/install --clean-boot` integrates this after greeter configuration.
The current profile uses tty1 on both Tops and lucky38. A custom greeter VT or
existing serial console needs a matching reviewed profile; do not promise universal
support by forcing the machine into this default.

Reboot validation pending: leave login screen visible beyond wait-online completion,
confirm centering and no overwrite, then log in. Confirm /proc/consoles is tty8,
getty@tty1 stays inactive, greetd unit settings applied, kernel arguments retain
AppArmor and normal session/keyring startup. Kernel/PID1 output on tty8 should
remain available. Do not infer visual correctness from unit validation alone.

## Confirmed reboot validation — 2026-10-02

User reports clean login UI after reboot. Runtime /proc/consoles names tty8;
greetd remains tty1 with reset/hangup/disallocate settings, getty@tty1 inactive.
NetworkManager wait-online still completes roughly six seconds after greetd starts,
so success does not come from delaying login or silencing boot output. No separate
geometry change was needed; the observed offset was resolved with console separation.
AppArmor remains enabled and its service active; user PAM unlocks the keyring.
No failed system units. Plymouth references in existing unit dependencies are
not-found/inactive, not running splash services.

One post-fix boot: firmware 13.939 s, loader 3.084 s, kernel 1.048 s,
initrd 6.956 s, userspace 9.114 s, total 34.142 s. Before cleanup total was
39.225 s. Graphical target reached 3.064 s into userspace; user default.target
1.266 s (includes PSD). This is an observed pair, not a multi-boot median or
proof that every component sped up. Private timing samples remain outside Git.
