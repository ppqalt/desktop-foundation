# Greeter console routing

The console helper separates a tty1 greetd login screen from kernel and systemd
boot output on tty8. It requires greetd configured on tty1 and GRUB with drop-in
support. Existing `console=` arguments, including serial-console settings, must
be reviewed before using this profile.

```sh
sudo scripts/greeter-console-setup install
```

Installation adds two owned files:

- `/etc/default/grub.d/95-desktop-foundation-console.cfg` adds `console=tty8` to
  common kernel arguments, including recovery entries.
- `/etc/systemd/system/greetd.service.d/20-desktop-foundation-console.conf` gives
  greetd tty1 input and terminal reset, hangup and disallocation settings.

The helper validates the service and regenerated GRUB menu. Changes take effect
on the next boot. Use Ctrl+Alt+F8 for the boot console and the journal for service
output.

```sh
cat /proc/consoles
systemctl cat greetd.service
journalctl -b -u greetd.service
```

The clean-boot installer runs this helper after greetd/PAM setup when greeter
installation is selected.

## Restore

Root-owned backups are under `/var/lib/desktop-foundation/greeter-console`.

```sh
sudo scripts/greeter-console-setup restore
```

Restore recovers the saved drop-ins and generated GRUB menu. When reverting the
combined boot setup, restore `boot-optimize` first, this console layer second and
`boot-setup` last. After completing restoration, regenerate GRUB before rebooting
if kernels changed since the backup:

```sh
sudo grub-mkconfig -o /boot/grub/grub.cfg
```

See [recovery](recovery.md).
