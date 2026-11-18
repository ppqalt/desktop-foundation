# Contributing

Install development tools with `scripts/bootstrap --dev`.

```sh
scripts/dev build
scripts/dev check
scripts/dev test
scripts/dev test-python
```

Niri handles layout, input and window effects. Quickshell provides the QML
surfaces; Rust provides system and state operations. Reuse the shared theme and
components when adding a surface. Keep hardware settings in profiles and retain
the deployment journals when changing managed configuration.

Tests use temporary state and command fixtures. Display-dependent tests are
opt-in with `DF_TEST_NIRI_IPC=1`. Exercise power routing with `--check`.

See [workflow](docs/workflow.md), [tooling](docs/tooling.md) and
[backend commands](docs/backend.md). The [command map](scripts/README.md) explains
shared aliases and the [tools map](tools/README.md) separates development-only
utilities. Preserve upstream licenses and attribution.
