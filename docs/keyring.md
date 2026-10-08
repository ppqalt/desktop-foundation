# Keyring

Desktop-foundation uses the packaged `oo7` Secret Service provider. Applications
reach it through `org.freedesktop.secrets`; the package supplies the D-Bus
activation alias and the hardened `oo7-daemon.service`. User deployment enables
that service for normal login and suppresses redundant GNOME Keyring autostart
entries. GNOME Keyring is not an installer dependency.

Greetd captures the login password with these optional PAM hooks, after the
existing system login stacks:

```pam
auth       optional     pam_oo7.so
session    optional     pam_oo7.so
```

`scripts/system-setup install` installs the greetd configuration and PAM template.
It does not restart greetd or the current desktop. Use a normal password login
for automatic unlocking. Fingerprint or passwordless login does not supply a
password to unlock an encrypted keyring.

The user deployment keeps a per-user activation descriptor pointing to the
package's `dbus-org.freedesktop.secrets.service` alias. This also replaces older
desktop-foundation activation files that selected GNOME Keyring. Missing GNOME
Keyring daemon service/socket masks are journaled for rollback; pre-existing
masks and an already enabled oo7 service remain owned by the user.

If oo7 PAM was installed manually after an earlier deployment,
`scripts/system-setup install` accepts the live file only when it exactly matches
the current repository template. It updates the recorded managed hash and keeps
the original rollback backup. Other edits still require review.

`scripts/system-setup restore` restores the original greetd/PAM files.
`scripts/uninstall` restores user configuration and removes only owned links.
Neither command deletes keyrings or changes package installations.

Package and PAM references: [Arch oo7 files](https://archlinux.org/packages/extra/x86_64/oo7/files/)
and [oo7 PAM module](https://docs.rs/oo7-pam/0.6.0/pam_oo7/).
