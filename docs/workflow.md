# Development workflow

Run from this checkout:

- `scripts/bootstrap`: install prerequisites (pacman needs physical sudo).
- `scripts/deploy --profile Tops`: validate and journal selected compositor/shared shell links.
- `scripts/restore`: recover originals, including interrupted deployment.
- `scripts/check` / `scripts/lint`: ShellCheck/shfmt, qmllint/qmlformat, Lua syntax,
  Python AST, installed Rust toolchain and all shipped/fallback Lua and Niri profiles.
- `scripts/format`: format Bash and QML. Python uses standard-library syntax checks;
  Lua uses luac. Optional Python/Lua formatters are not silently installed.
- `scripts/test`: builds the debug Rust command backend, then runs isolated regression tests.
- `scripts/doctor`: active session, service, environment, binaries and fi checks.
- `scripts/bench --runs 3 --seconds 5 --output work/bench.json`: finite repeated measurements.
- `scripts/reload`: check then reload compositor.
- `scripts/shell-start` / `scripts/shell-stop`: independent bounded shell supervision.
- `python3 scripts/smoke.py`: native-backend live test with owned temporary Kitty windows; restores focus.

The `scripts/dev ACTION` dispatcher provides the same commands. The native Nothing crate is checked with
Cargo fmt/clippy/test by scripts/check. Build it with scripts/build-nothing; its
source, lockfile and license live under native/nothing.
The shared `native/foundation` crate has the same format, Clippy and test gates.
Build its release executable with `scripts/build-backend`. Direct deployment
requires both this and `scripts/build-nothing` to have completed, before any
links or state are changed. The core installer performs these builds itself.
Runtime commands use the release executable without compiling code on demand. SQLite is linked from
the system package; no second SQLite implementation is bundled.
Perf is optional and absent on Tops: install with
`sudo pacman -Syu --needed perf`. Optional formatters: `sudo pacman -Syu --needed ruff stylua`.
Missing sudo credentials block package changes only. Existing Qt profiler, heaptrack,
strace and hyperfine remain available. See the code-quality review for the complete
inventory of runtime deadlines, bounded startup retries and event subscriptions.

Profiles/default contains portable automatic monitor and input defaults. Tops and
lucky38 own only host overrides. The lucky38 profile preserves the established
AOC monitor / LG TV layout. A missing profile or category falls back to
default. Shared input/animation/
bindings never encode output names, resolution, refresh rate or GPU paths.

For continued work on lucky38, see [the development baseline](development-baseline-lucky38.md).

`python3 scripts/failure.py` performs one real supervised shell crash and isolated
bad-config/backend checks; keep it separate from routine pure unit tests.
`bench --trace` appends finite process tracing. Probe lifecycle counters and debug
raw-event counters are exposed through `quickshell ipc --path shell call foundation status`.
