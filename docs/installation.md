# Installation

The primary target is a clean Arch/CachyOS installation, not a migration of an old desktop.
Use a normal Arch/CachyOS login user with sudo and a working graphical stack.
A TTY is sufficient for preparing the configuration; sign out and choose Niri to
start the complete setup. Do not run the user installer with sudo.

1. Obtain a Git clone in a permanent location, for example `~/Projects/desktop-foundation`.
2. Run `./scripts/install --plan` to see the selected steps without mutations.
3. Run `./scripts/install`; missing runtime packages are installed with a full
   Pacman upgrade, avoiding partial upgrades. Existing packages are retained.
4. The default installer configures greetd + tuigreet, Matrix animation and GNOME
   Keyring PAM hooks. This step requires sudo. Use `--no-greeter` only when you
   deliberately supply your own login setup.
5. Reboot, then log into the **Niri** session and run `./scripts/session-acceptance` before repairs.

`--profile default` is implicit. Input is Finnish by default; change `config/input.lua`
for another keyboard before installation. Create `profiles/NAME/niri.kdl` for local
output/input-device overrides and use `--profile NAME`. Fonts, software names,
wallpaper, window theme and terminal output otherwise discover the actual machine.
No host name, GPU model or display resolution is required by the portable config.

## Supported build and limitations

This repo currently uses native Niri `blur`, `background-effect`, popup effects and
native Wayland screenshot interfaces available in the tested CachyOS build
(26.04-1.1). Runtime preflight validates the rendered KDL on the installed binary.
If validation fails, deployment stops before replacing user configs. Obtain a
compatible compositor through your chosen Arch/CachyOS packaging; the installer
does not add external repositories or silently weaken the approved visual theme.
Quickshell 0.3.1, Kitty 0.49.1, Fish 4.9.3 and Fastfetch 2.69.0 were tested.
Fastfetch package formatting uses a small explicit JSON config, not removed CLI
module options. Tuigreet must support `--background matrix`; system setup validates
this option before changing authentication files.

Firefox and Nautilus are default applications for a new host, not copies of Tops'
Brave account or former file manager. Existing valid defaults are kept. The supplied
wallpaper is deployed from `wallpapers/`. Optional Spotify is documented separately.
No application credentials or home-directory app data are included.

## Native dark dialogs

`adw-gtk-theme` is installed by bootstrap. Tracked GTK 3 settings select
`adw-gtk3-dark` and the shared Google Sans font; GTK 4 gets native dark preference
and font settings, alongside the portal's `prefer-dark` value. This fixes GTK 3
dialogs that remained light on the tested build despite reporting `Adwaita-dark`.
Configuration files and global preferences are backed up and restored by the
normal installer. Existing applications may need reopening to adopt the settings.
No app CSS overrides are installed. Notification toasts continue using the tracked
Niri graphite/top-center/two-second Mako configuration.

## Bluetooth

Runtime bootstrap installs BlueZ, Blueman, PipeWire audio/codec support, WirePlumber
and `libpulse` (`pactl`), and enables/starts `bluetooth.service`. Pair once through
Blueman Manager, then use **Super+B** to reconnect. Radio rfkill/power state remains
under the user's control; turn Bluetooth on in Blueman if disabled.
The tracked WirePlumber 0.5 fragment selects quality policy, permits temporary
microphone-driven headset switching, and avoids remembering headset mode across
connections. Deployment backs up/restores that fragment. Existing sessions need
WirePlumber restarted or the corresponding runtime settings applied; new logins
load the fragment normally.
The tracked XDG autostart override suppresses Blueman's permanent applet only in
Niri. Blueman Manager can activate its applet for pairing/passkeys and stops it on
exit when it started it. Blueman itself and its D-Bus activation are retained.
Connected Nothing/CMF models have native device controls. The installer also
provisions Rust 1.98.1 and builds the locked RFCOMM helper; no resident service or
upstream clone is needed. See [native controls](nothing-controls.md) and
[Bluetooth implementation and validation](bluetooth.md).

## Backup and recovery

User deployment journals original config paths under
`$XDG_STATE_HOME/desktop-foundation` (normally `~/.local/state/desktop-foundation`).
Global dark preferences and MIME defaults have their own backups in that directory.
System login/PAM backups live under `/var/lib/desktop-foundation/system`.
`./scripts/uninstall` restores user files/preferences; `--greeter` also restores
system files. System restore does not switch the active display manager service.

If login fails, use Ctrl+Alt+F3, authenticate and run the restore commands from the
checkout. Reboot normally after fixing greeter config; avoid restarting the display
manager from an active session. Keyring auto-unlock requires a password login and
matching login/keyring passwords. Saved credentials are never copied or deleted.

## Repeat installs and updates

Pull changes in the same checkout and run `./scripts/install --no-packages` after
reading their notes. Use the full installer when dependency requirements change.
No background updater is installed. Deployment refuses externally replaced config
paths/preferences rather than discarding local edits. Preserve customizations in
tracked configuration or a host profile; keep backups until satisfied.

The current desktop was validated on Tops; a pristine lucky38 install has not yet
been exercised. The portable suite and parser checks reduce that gap but do not
substitute for a fresh-login acceptance pass on its actual GPU/output setup.
