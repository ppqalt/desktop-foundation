# lucky38 development baseline — 2026-10-03

Continue in `~/Projects/desktop-foundation` on `lucky38/converge-v0.12`.
The inspected starting HEAD is `bbddde0a214671be912a35c94a3465abfb01189a`
(`v0.12-8-gbbddde0`); the working tree was clean. Origin is
`https://github.com/ppqalt/desktop-foundation.git`. No fetch, branch switch,
reset, installation, deployment or session reload was performed.

## Changes after v0.12

- `3ac2692`: lucky38 output profile preserving the established monitor layout.
- `8cabbb2`: packaged Mako activation through the managed notification unit.
- `cd92489`, `ca5b7af`, `c8c492f`: Brave Origin Nightly core browser policy and
  tests that check core/personal separation independently of browser choice.
- `0bc8753`: bounded clipboard projection and no export on ordinary copy.
- `4111b94`: missing-PAM diagnostics and isolated host-policy fixtures.
- `bbddde0`: browser-role documentation aligned with branch policy.

## Observed desktop baseline

Niri runs with `--session`; the active config resolves to
`~/.local/state/desktop-foundation/niri.kdl` and validates successfully.
One Quickshell process runs with `--no-duplicate --path` pointing to this
checkout's `shell` directory. The foundation session target, shell, notifications,
wallpaper, clipboard persistence and text/image watchers are active; clipboard
initialization completed. DP-1 is 2560×1440 at 155 Hz, scale 1, position 0,0.
These are process/service/config observations, not visual or interaction acceptance.

The working desktop is the feature-development baseline. Do not reinstall it,
run `install-all`, repeat cleanup/migration, or reset to v0.12. This checkout is
used by the resident shell: runtime-source edits may affect the active desktop.
Prepare features in an isolated checkout when runtime changes are involved, and
review deployment/reload separately.

## Architecture and feature entry points

- `compositor/niri`, `scripts/render_niri.py`, `config`, `profiles`: native layout,
  input, effects and rendered hardware configuration.
- `shell/shell.qml`: composition root, adapter selection, IPC and lazy surfaces.
- `shell/adapters/niri`: event-driven JSON IPC state and bounded action requests;
  shared surfaces use the normalized compositor interface.
- `shell/surfaces`, `shell/components`, `shell/services`: UI and shared behavior;
  no permanent bar or presentation polling worker.
- `session`, `scripts/session_units.py`: systemd-owned graphical lifecycle and
  supporting services. Basic compositor bindings work independently of the shell.
- `theme`: immutable runtime revisions and transactional publication.
- `native/nothing`: on-demand Rust device backend, not a resident Rust shell daemon.
- `tests`: isolated standard-library Python tests; native Rust tests are separate.

See [architecture](architecture.md), [compositor interface](compositor-interface.md),
[startup](startup.md), and [workflow](workflow.md).

## Validation at the starting HEAD

- `scripts/test`: 107 tests, successful, one opt-in live Qt adapter test skipped.
- `scripts/check`: blocked immediately by missing ShellCheck. `shfmt` is also
  absent. Bootstrap was inspected but not run because it installs packages and
  enables services.
- Available stages run separately: Python AST, Lua syntax, Niri validation for
  default/Tops/lucky38/missing-profile, QML formatting, and Rust formatting pass.
- `qmllint` exits 0 with unresolved-type/property/signal warnings; this is not
  warning-free validation.
- Native crate offline locked Clippy with warnings denied passes; all nine Rust
  tests pass. No dependency downloads were needed.

Use `scripts/test` for portable regression checks and `scripts/check` for the
full gate once ShellCheck/shfmt are available. Keep live adapter, crash, popup,
deployment, reload and reboot tests outside routine baseline inspection.
