# Native theme pipeline

Wallpaper/theme commands now run inside `desktop-foundationctl`:

- `scripts/wallpaper-set IMAGE [--mode fill|fit|center|tile]`
- `scripts/theme-generate [IMAGE]`
- `scripts/theme-apply [IMAGE]`, `scripts/theme-reset`
- `scripts/theme-rollback`, `scripts/theme-promote`

The same backend owns image decoding, backdrop blur/cache, Niri/terminal/desktop
renderers, immutable revisions, stable Brave theme files, reloads and rollback.
Matugen remains the packaged one-shot palette generator. GTK, managed Brave policy
and Spicetify use their existing native refresh mechanisms. No extra worker or
polling daemon is introduced.

`foundation theme-runtime ensure PROFILE` refreshes an install's outputs without
restarting the session or changing its wallpaper/palette. The existing deployment
entry point delegates to it. `scripts/preflight` also validates configuration in
Rust; the installer and updater use that entry point.

Image decoding uses the pinned Rust `image` crate for PNG, JPEG, WebP, GIF, BMP and
TIFF. Native Gaussian blur retains the 24 px radius and 1920 px downsampling for
fill/fit. Its cache has a distinct version, since pixels need not match Pillow's
blur implementation exactly. Original wallpaper content is copied intact.

Revisions retain the existing `current`/`previous`, metadata and ownership layout.
All rendering and validation finishes before publication. Reload failure restores
the old pointer and stable Brave manifest, then attempts to reload the old theme.
The immutable revision path changes; Brave's installed theme directory does not.

`foundation spotify-theme install|refresh|restore` replaces the color adapter.
It preserves unrelated INI lines/schemes, refuses externally changed owned keys,
and accepts the existing theme-binding journal. Spotify provisioning delegates
to this adapter; installing/upgrading the clients is a separate remaining task.

## Verification

`cargo test --locked --manifest-path native/foundation/Cargo.toml --test theme_runtime`
uses isolated files, a fake Matugen and inert reload callbacks. It covers atomic
publication, failure recovery, foreign ownership, format decoding, output roles,
cache hits and generation failure. A private PATH containing no Python proves
native deployment preparation and preflight do not require it.

## Remaining Python

This is a runtime migration, not a claim that the repository is Python-free.
Legacy doctor/system setup, Spotify client provisioning, diagnostics,
older tests and their import-compatible helper modules remain. Those legacy
modules are retained until their callers/tests migrate; normal theme commands,
new deployment theme preparation, Spotify color binding and the wallpaper-startup
recovery path now use Rust. No new Python implementation or tests were added.

## Native deployment and supported platform

`deploy`, `uninstall` and preference setup now use the Rust ownership journal and
native session-file generation. Existing `manifest.json` backups are retained,
including their original inode fingerprints and recovery phases. Updates skip
global preference writes. Fish's writable runtime state remains outside the
checkout. Pre-existing oo7 enablement and GNOME Keyring masks are not adopted.

`scripts/preferences install --theme-only` and `scripts/preferences restore`
use the native preference/MIME coordination lock. `scripts/check` and
`scripts/test` use native validation, Rust tests and the offscreen QML checks.
Older Python test/helper modules remain available until their remaining callers
are migrated; normal deployment no longer runs them.

The supported platform is CachyOS with Niri. Bootstrap and deployment reject
other platforms/compositors; the shell, session startup, launch routing and
updater select only Niri. Existing archived compatibility sources are not an
alternate supported installation path. Restoration still understands older
journals so removing support does not strand their backups.

## Native installer orchestration

`scripts/install`, `install-core` and `install-all` now dispatch through native
`install.rs`. Layer/profile parsing, read-only planning, ownership inspection,
package/adoption decisions, application metadata validation, build/deploy order
and error propagation no longer execute Python. Native application-role and
app-mod configuration readers are reused.

Inspection does not create installation state, authenticate sudo or build a
missing backend. Normal installation is serialized by `install.lock`; the native
deployment and preference journals retain their own locks and recovery behavior.
A missing personal application in `--no-packages` mode stops before configuration
deployment. No package, service or root-helper actions are run by migration tests.

The installer still delegates to existing privileged boot/login/policy helpers,
Spicetify provisioning and doctor; these remaining Python consumers have not
been relabeled as native. `install_engine.py` remains for old imports/tests until
those callers migrate. No new Python implementation or tests were added.

`cargo test --locked --manifest-path native/foundation/Cargo.toml --test install`
uses private paths and an inert effect adapter to test layer separation, terminal
command routing, dry-run/check safety, overrides, ownership, failures and reviewed
AUR fallback. Real installation or live desktop acceptance is outside that test.
