# Authentication and locking

Foundation uses the existing text-based systemd/cryptsetup prompt for LUKS2 and
an independent, on-demand Quickshell session-lock configuration for the desktop.
There is no graphical boot splash and no new permanent authentication daemon.

## Encrypted boot

A package-enabled installation with DF-owned greetd records the installing account
in root-owned `/etc/desktop-foundation/authentication.toml`:

```toml
encrypted_login = true
user = "your-account"
```

The root-owned `foundation-auth` helper runs once before greetd, generates its
configuration under `/run`, and permits a single initial Niri session only when:

- The actual mounted root is a direct LUKS2 mapping. Multi-device Btrfs and
  unsupported layered storage retain normal login.
- The named account is a regular interactive account with an enabled password.
  greetd still applies its PAM account/session policy.
- LUKS metadata has no hardware/automatic tokens; crypttab has no root keyfile,
  automatic root keyfiles are absent, and the command line has no keyfile/token
  unlock options.
- Root crypttab explicitly disables the cryptsetup password cache, and the
  installer has successfully rebuilt the external boot images with that policy.

The installer preserves encryption identifiers, keyslots and unrelated crypttab
entries. It adds `password-cache=no,x-initrd.attach` to the root entry; a legacy
`crypttab.initramfs`, when present, also has caching disabled. No passphrase is
read, compared with the account password, passed to PAM, or written by Foundation.
The LUKS passphrase can differ from the Linux account password.

This is an explicit authorization policy: anyone able to unlock that root disk
may enter the configured account. It does not identify which LUKS keyslot owner
entered a password. Shared/multi-user systems should set `encrypted_login=false`
and use conventional login. Policy edits are preserved during subsequent installs.

A root-only marker makes this an initial boot session, not recurring autologin:
logout and greetd restarts return to the normal greeter. Unencrypted, unsupported
or uncertain boots also retain the normal greeter. Existing SDDM/other managers
and `--no-greeter` installations receive locker integration only; DF does not
replace their login policy. Firmware and bootloader selection are unchanged.
Vendor-signed EFI executables are not rewritten. Custom generated-UKI workflows
require explicit signing review and retain normal login until supported.

No password exists for automatically unlocking the application secret store on
this path. A non-secret one-shot PAM marker skips oo7's password-readiness wait;
application keyrings stay protected and may ask for an account/keyring password
when first used. Ordinary password login keeps the existing oo7 PAM handoff.

## Desktop lock

Super+L calls `scripts/lock-screen lock`; Super+Right still focuses right.
The user service runs only while locked. `shell/lock.qml` runs independently of
the ordinary shell, disables file reloads, and uses `WlSessionLock` rather than
an overlay. Niri's `secure` acknowledgement is required before service readiness.
Every output receives an opaque lock surface, including newly connected outputs.
Only successful PAM authentication for the current process account can unlock.
PAM uses `/etc/pam.d/desktop-foundation-lock` and the distribution's `system-auth`
policy. Passwords stay masked, are cleared after submission/cancellation/result,
and are never sent through command lines, IPC, environment variables or logs.

The shared `LockContent.qml` is also used in the gallery; its sample action cannot
unlock a real session. Semantic colors, Google Sans Flex, shared input/button
components and reduced-motion timing are reused. Only content fades after success;
the covering lock surface remains opaque until the protocol unlock.

Crashes or failed startup switch to stock swaylock using a real ready-fd, not a
process-start assumption. Niri keeps an abandoned lock covered while replacement
starts. If recovery itself fails, the compositor remains locked; switching VT and
logging in conventionally is the recovery path. Never terminate Niri to recover
unsaved applications. Ordinary shell reloads do not own the locker.

## Idle and suspend

User overrides belong in
`~/.config/desktop-foundation/session-security.toml`:

```toml
idle_lock_seconds = 0 # disabled; otherwise 30..86400
```

Restart `desktop-foundation-lock-events.service` after changing this setting.
A single event-driven swayidle instance receives logind lock requests and optional
idle notifications; it does not poll or render a background lock UI.

The system's sleep target requires a bounded, one-shot lock check. It finds actual
local managed Niri sessions, including greetd sessions initially classified as tty,
and starts a freshly confirmed locker before sleep proceeds. Failure denies the
sleep transaction. This also covers suspend requested outside DF's power menu.
The compositor lock remains held through suspend/resume and output reconfiguration.
The check runs only for sleep, with no additional boot service or idle daemon.
Forced firmware/root-level sleep and compositor/driver failures are outside this
user-session guarantee.

## Installation, recovery and validation

`authentication-setup install` installs reviewed native helper/PAM/unit assets,
preserves originals and hashes in
`/var/lib/desktop-foundation/authentication/manifest.json`, then rebuilds changed
encrypted boot policy through the existing no-Plymouth image workflow. It never
restarts the active display manager. `--no-packages` and dry-run do not provision
these privileged assets. User lock/event units use the normal deployment journal.

`sudo ./scripts/authentication-setup restore` restores owned privileged files,
checks retained backups and refuses unrelated modifications. It preserves explicit
administrator edits to authentication.toml. Restore authentication before reversing
Plymouth cleanup. Rebuild failures retain a marker that forces conventional login;
rerun the same installation command to finish the rebuild.

Portable tests cover root-device/policy validation, crypttab preservation, installer
routing and masked keyboard interaction. Explicit nested tests use private headless
Weston/Niri, synthetic PAM allow/deny policies and stock swaylock to verify protocol
acquisition, denied/successful authentication and replacement after a killed locker.
They do not use a real account password or the host desktop. GitHub's dedicated
Arch job requires these tools instead of silently skipping those lock tests.
Physical encrypted boot, real-password unlock, multiple physical displays and real
suspend/resume remain hardware acceptance checks; fixture results do not prove them.

References: [Quickshell session lock](https://quickshell.org/docs/v0.3.2/types/Quickshell.Wayland/WlSessionLock/),
[PAM interface](https://quickshell.org/docs/v0.3.2/types/Quickshell.Services.Pam/PamContext/),
[Niri recovery](https://niri-wm.github.io/niri/FAQ.html#how-do-i-recover-from-a-dead-screen-locker-from-a-red-screen),
[greetd initial sessions](https://man.archlinux.org/man/greetd.5.en),
[crypttab caching](https://www.freedesktop.org/software/systemd/man/latest/crypttab.html).
