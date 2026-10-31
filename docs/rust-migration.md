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
Other privileged setup, Spotify client provisioning, boot-report diagnostics,
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

The installer still delegates to existing privileged boot/policy helpers and
Spicetify client provisioning. Login setup and doctor now use Rust. The obsolete
`install_engine.py` and its old import tests have been removed; native installer
fixtures cover their active contracts. No new Python implementation or tests were added.

`cargo test --locked --manifest-path native/foundation/Cargo.toml --test install`
uses private paths and an inert effect adapter to test layer separation, terminal
command routing, dry-run/check safety, overrides, ownership, failures and reviewed
AUR fallback. Real installation or live desktop acceptance is outside that test.

## Native login setup

`scripts/system-setup install|restore` now dispatches to Rust. Its existing
root-owned greetd/PAM/readiness-helper journal remains compatible, including
managed/pending/original digests and original backup paths. Reads reject symlinks,
unsafe parent ownership/modes, oversized or non-regular files and escaped journal
paths. Backup and target checks finish before any login target is replaced.

The native setup validates its PAM readiness order and greetd TOML, stages the
ELF helper before PAM, and restores PAM before removing the helper. Original
bytes/modes survive reruns and interrupted publication/restoration. The existing
flock is shared with older setup implementations; no production test-root or
non-root override is exposed. Only `systemctl enable greetd.service` is issued;
the running login manager is never started/restarted by this command.

Tests use a private simulated filesystem and inert capability/service operations.
They cover old journal recovery, legacy exact-template adoption, backup/target
conflicts, unsafe paths/modes, source validation and enable failure. No live PAM,
greetd, keyring or root-owned installation files were changed for verification.
Other privileged helpers and provisioning remain migration targets.

## Native diagnostics and Spotify check

`scripts/doctor` now dispatches installation/session checks, startup timeline,
sample save/compare and the explicit popup benchmark to Rust. `--json` keeps its
existing build/deployment metadata contract. `scripts/session-acceptance` also
uses Rust. Operational diagnostics run as the normal desktop user, use bounded
subprocesses and report unreadable inputs rather than stopping with a traceback.

`scripts/spotify-setup check` uses the native read-only checker. It validates
client source stamps, Spicetify version/paths, Marketplace assets and the selected
scheme. Spotify preferences are checked only for existence; their contents are
not read. Client installation/restore still use the existing provisioning helper.

Native fixtures cover missing PAM/portal services, malformed shell responses,
corrupt role journals, theme hash mismatches, current-login startup timing,
missing timing evidence, sample deduplication, unsafe sample paths and private
Spotify checks without Python in PATH. No live popup benchmark was performed.
The legacy `startup_report.py` remains for `boot-report.py` and older import tests;
it is no longer reached by doctor.
