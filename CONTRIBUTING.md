# Contributing

Install development tools with `scripts/bootstrap --dev`.

```sh
scripts/build-backend
scripts/build-nothing
scripts/check
scripts/test
```

Niri handles layout, input and window effects. Quickshell provides the QML
surfaces; Rust provides system and state operations. Reuse the shared theme and
components when adding a surface. Keep hardware settings in profiles and retain
the deployment journals when changing managed configuration.

Tests use temporary state and command fixtures. Display-dependent tests are
opt-in with `DF_TEST_NIRI_IPC=1`. Exercise power routing with `--check`.

See [workflow](docs/workflow.md), [tooling](docs/tooling.md) and
[backend commands](docs/backend.md). Preserve upstream licenses and attribution.
