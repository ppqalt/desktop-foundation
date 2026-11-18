# Development workflow

Run development commands from the checkout. Install dependencies and build the
release backends before deploying configuration:

```sh
scripts/bootstrap --dev
scripts/dev build
scripts/deploy --compositor niri --profile default
```

`bootstrap --greeter` adds login dependencies; the normal installer configures
login integration. Package installation requires sudo; AUR builds run as the
login user.

| Command | Purpose |
| --- | --- |
| `scripts/dev check` | Check Bash/QML formatting and lint, compositor profiles and Rust format/Clippy/tests |
| `scripts/dev format` | Format Bash, QML and Rust files |
| `scripts/dev test` | Native and offscreen QML regressions |
| `scripts/dev test-python` | Build the debug command backend and run Python regression tests |
| `scripts/doctor` | Inspect installation and active-session services |
| `scripts/reload` | Run checks, then reload the active compositor configuration |
| `scripts/shell-start` / `scripts/shell-stop` | Start or stop the supervised shell |
| `scripts/restore` | Restore deployment-owned configuration |
| `scripts/bench --runs 3 --seconds 5 --output work/bench.json` | Collect finite repeated shell measurements |

`scripts/dev --help` lists the workflows. Existing `scripts/test`, `scripts/lint`,
`scripts/format` and other aliases remain compatibility links to the shared
dispatcher; see the [command map](../scripts/README.md).
The native crates under
`native/foundation` and `native/nothing` have their own Cargo manifests and lockfiles.
Runtime commands use their built release executables. The shared backend links
against the system SQLite package.

`profiles/default` contains automatic output defaults. The
`profiles/dual-display-example` directory demonstrates hardware overrides. Shared
input and bindings belong in `config/` and compositor configuration rather than
hardware profiles.

## Interactive checks

`python3 tools/live/compositor.py` exercises the native compositor backend with temporary
Kitty windows and restores focus. `scripts/failure` deliberately crashes
the supervised shell once, checks recovery and runs isolated bad-configuration
checks. Run these explicitly in a desktop session when those behaviors need review.

Use `scripts/bench --trace` for finite process tracing. Inspect runtime state with
`quickshell ipc --path shell call foundation status`. See
[tooling](tooling.md), [development tools](../tools/README.md),
[deployment](deployment.md) and [performance](performance.md).
