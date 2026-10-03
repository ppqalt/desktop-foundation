> Historical acceptance instructions for `fresh-install-test-1`. For v0.12 use
> [the current physical checklist](fresh-install-v012.md) and
> [release validation](releases/0.12-validation.md).

# First fresh Tops test

Do not wipe until the off-machine backup is verified and the candidate checklist
passes. The local tag is `fresh-install-test-1`; record `git rev-parse` below.

## Before reinstall

- Back up personal files/password stores and anything else you need separately.
- Have base CachyOS installation media and network access.
- Use a CachyOS Niri build with blur and a Matrix-capable tuigreet; stock Arch
  versions may fail capability checks. Use GRUB for the automated AppArmor path.
- Record `git rev-parse fresh-install-test-1`.
- Choose an actual external destination yourself, then run:

```bash
cd /path/to/desktop-foundation
git bundle create /YOUR/EXTERNAL/DESTINATION/desktop-foundation.bundle --all
git bundle verify /YOUR/EXTERNAL/DESTINATION/desktop-foundation.bundle
```

Verify the file is accessible from another machine/media before wiping. A bundle
on the disk being wiped is not a backup. No off-machine backup has been made by
this preparation pass. Nothing has been pushed to GitHub.

## After base CachyOS installation

Log in as a normal user with sudo access; mount boot and install Git if missing.
Copy/mount the bundle, then:

```bash
git clone /YOUR/EXTERNAL/DESTINATION/desktop-foundation.bundle desktop-foundation
cd desktop-foundation
git checkout fresh-install-test-1
./scripts/install --hardware-profile Tops
```

Default is personal. Portable core: `./scripts/install --core`.
Inspect first with `./scripts/install --dry-run --hardware-profile Tops`.
The Paru bootstrap prints the recipe and asks you to review it before execution.
Reboot yourself after successful installation; log into Niri through Matrix.

## First login

Run `./scripts/doctor --personal` and physically check:

- Matrix login, Niri wallpaper, Finnish punctuation (`/` is Shift+7).
- One tiled window centered, two normal scrolling columns; floating/fullscreen.
- Super+Space launcher, Super+V clipboard (text/image, copy closes, persistence).
- Super+B Bluetooth, keyboard/wheel navigation, pair earbuds then Controls.
- PageUp/PageDown 3% volume and thin readout; End play/pause.
- Super+Shift+Q power menu (avoid activating shutdown during other checks).
- Super+Shift+S fresh empty selection, release captures; Print whole screen;
  both clipboard-only.
- Dark popups, top-center notifications disappear around two seconds.
- Kitty crisp text and native 90% background, Fish `c`/`fast`, Fastfetch.
- Brave default, Spotify launches/playback/media; real wallpaper/blur.
- AppArmor enabled/enforcing and no failed services; inspect denials if needed.

## Personal finalization

Open Spotify, sign in, leave it running about a minute, then close it and run
`./scripts/post-install spotify`. Restart Spotify and confirm Marketplace.
Brave preferences are seeded by the personal installer. If it reports an open
browser, close Brave and run `./scripts/post-install brave`. Install the five
extensions using the official links it prints; no extension data is restored.

Pair Bluetooth devices via the administrative fallback. No browser login,
Spotify credentials, Bluetooth pairings, clipboard/Fish history or arbitrary
personal files are restored by this repository.
