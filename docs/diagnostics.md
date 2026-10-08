# Backend diagnostics

`scripts/foundation build-info` prints the backend's package version, Git revision,
build-time native source working-tree state and native source digest as JSON.

`scripts/doctor --json` prints those fields alongside the current source match,
compositor, deployment owner, application profile and theme revision. It reads
bounded desktop-foundation metadata without starting services or changing state.
The normal `scripts/doctor` command retains the installation and session checks.

`sources.matchesBinary` compares the current Rust modules, native Cargo files and
build script with the binary's inputs. A false value means rebuild the backend;
changes to wallpaper or desktop configuration do not affect that comparison.
An unavailable source tree is reported separately from a mismatch. The build's
`dirty` field describes the native package when compiled and is not a runtime check.

Both JSON formats have `formatVersion: 1`. Source digests use SHA-256 with an
ordered, length-prefixed file list; the digest includes paths relative to the
native package so moving the checkout does not change its identity. Builds from
source archives retain the source digest and report an unknown Git revision.
