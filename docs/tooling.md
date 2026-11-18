# Development tooling

Install the development package set with:

```sh
scripts/bootstrap --dev
```

The package manifests under `packages/` define the dependencies. Bootstrap installs
the core packages and pinned Rust toolchain; `--dev` adds build,
lint and profiling tools plus Rust formatting, Clippy and rust-analyzer components.
Additional package groups are selected with `--greeter`, `--personal`,
`--standard`, `--recording` and `--firmware`.

| Area | Tools |
| --- | --- |
| Bash | ShellCheck, shfmt |
| QML | qmllint, qmlformat, qmlls, qmlprofiler, qsb |
| Shared configuration | Rust TOML parser and Niri validation |
| Rust | Cargo, rustfmt, Clippy, rust-analyzer |
| Native build | GCC, Clang/LLVM, CMake, Ninja, pkgconf |
| Profiling | hyperfine, heaptrack, gdb, strace, perf |

Qt command-line tools are called through `/usr/lib/qt6/bin`. The Quickshell package
supplies the QML module metadata used by linting. `scripts/check` and
`scripts/format` use these paths directly.

```sh
scripts/dev check
scripts/dev test
scripts/dev test-python
scripts/dev format
```

`scripts/dev build` builds both release backends; use `build foundation` or
`build nothing` for one crate. The original build scripts remain for installer
bootstrap. For
finite profiling commands and report locations, see [performance](performance.md).
For the command workflow, see [development workflow](workflow.md).
