# Keyring

Desktop-foundation uses the packaged `oo7` Secret Service provider. Applications
reach it through `org.freedesktop.secrets`; the package supplies the D-Bus
activation alias and the hardened `oo7-daemon.service`. User deployment enables
that service for normal login and suppresses redundant GNOME Keyring autostart
entries. GNOME Keyring is not an installer dependency.

Greetd captures the login password during authentication. After `system-local-login`
creates the user manager and runtime directory, a bounded readiness hook starts
or reuses the packaged oo7 service and waits for its private PAM socket. Native
`pam_oo7.so` then supplies the login password:

```pam
auth       optional     pam_oo7.so
session    include      system-local-login
session    [success=ok ignore=ignore default=1] pam_exec.so quiet type=open_session /usr/local/libexec/desktop-foundation-oo7-ready
session    optional     pam_oo7.so
```

`scripts/build-backend` builds the readiness executable. `scripts/system-setup install`
copies it to a root-owned, non-setuid path before installing the PAM template.
The login hook never executes a user-writable checkout binary. It drops to the
authenticated user's credentials before contacting their user manager, clears
the display manager environment, and has a ten-second total deadline. It checks
socket ownership and mode without connecting to the password socket or handling
the password.

The native oo7 session line must immediately follow the readiness line. A readiness
failure skips that one handoff, leaving the keyring locked for manual unlocking
while preserving the normal login result. The close-session path follows whichever
modules opened successfully. Doctor checks this ordering and control policy.

`scripts/system-setup install` installs the greetd configuration and PAM template.
It does not restart greetd or the current desktop. Use a normal password login
for automatic unlocking. Fingerprint or passwordless login does not supply a
password to unlock an encrypted keyring.

oo7 0.6's native PAM handoff makes a single socket connection attempt. Enabling
its `Type=simple` service alone can report started before that socket exists.
The readiness hook closes that startup gap. A second 0.6 issue removes the socket
after a completed connection; if an active daemon still has no listener after
the bounded wait, the helper permits one restart through the same hardened unit.
That recovery retains encrypted files but relocks collections in other active
sessions. Healthy service/socket state is reused.

Automatic unlocking still requires the collection password to match the login
password. The helper does not change collection passwords or make them empty.
The native PAM module retains password capture and delivery.

The user deployment keeps a per-user activation descriptor pointing to the
package's `dbus-org.freedesktop.secrets.service` alias. This also replaces older
desktop-foundation activation files that selected GNOME Keyring. Missing GNOME
Keyring daemon service/socket masks are journaled for rollback; pre-existing
masks and an already enabled oo7 service remain owned by the user.

If oo7 PAM was installed manually after an earlier deployment,
`scripts/system-setup install` accepts the live file only when it exactly matches
the current or immediately previous oo7 template. It updates the recorded managed
hash and keeps
the original rollback backup. Other edits still require review.

`scripts/system-setup restore` restores the original greetd/PAM files.
`scripts/uninstall` restores user configuration and removes only owned links.
Neither command deletes keyrings or changes package installations.

Package and PAM references: [Arch oo7 files](https://archlinux.org/packages/extra/x86_64/oo7/files/)
and [oo7 Arch/greetd handoff issue](https://github.com/linux-credentials/oo7/issues/516).
The upstream [socket cleanup correction](https://github.com/linux-credentials/oo7/commit/2eac1187db54ae9c37d19a6f50a5f59adeac3a53)
documents the 0.6 warm-service issue.
