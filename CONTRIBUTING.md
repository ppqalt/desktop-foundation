# Contributing

Use an Arch/CachyOS development machine and run `scripts/bootstrap --dev`.
Run `scripts/check` for shell/QML/native config validation and `scripts/test` for
portable tests. Live adapter coverage is opt-in: `DF_TEST_NIRI_IPC=1 scripts/test`
requires Quickshell and a display/test environment. Use a temporary profile to test
hardware choices; keep machine names and absolute user paths out of reusable code.

Niri is the reference compositor. Reuse shell/theme tokens and event-driven adapters;
avoid polling or new persistent workers for presentation. Config replacements must
remain reversible and startup work must survive logout/reboot. User-session and
system authentication changes belong in separate installers.

Do not commit private screenshots, clipboard histories, credentials or generated
font caches. Controlled live fixtures go under ignored `work/`. Assets retain their
original licenses/provenance. Test destructive menu routing with `--check`, not
by rebooting a contributor's active session.
