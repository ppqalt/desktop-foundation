# Development workflow

Run from this checkout:

- `scripts/bootstrap`: install prerequisites (pacman needs physical sudo).
- `scripts/deploy --profile Tops`: validate and journal selected compositor/shared shell links.
- `scripts/restore`: recover originals, including interrupted deployment.
- `scripts/check` / `scripts/lint`: ShellCheck/shfmt, qmllint/qmlformat, Lua syntax,
  Python AST, installed Rust toolchain and all shipped/fallback Lua and Niri profiles.
- `scripts/format`: format Bash and QML. Python uses standard-library syntax checks;
  Lua uses luac. Optional Python/Lua formatters are not silently installed.
- `scripts/test`: isolated deployment recovery tests.
- `scripts/doctor`: active session, service, environment, binaries and fi checks.
- `scripts/bench --runs 3 --seconds 5 --output work/bench.json`: finite repeated measurements.
- `scripts/reload`: check then reload compositor.
- `scripts/shell-start` / `scripts/shell-stop`: independent bounded shell supervision.
- `python3 scripts/smoke.py`: secondary Hyprland live test with owned temporary Kitty windows; restores focus.

The `scripts/dev ACTION` dispatcher provides the same commands. If a Cargo workspace
is added later, check also runs fmt/clippy/test. No speculative crate exists today.
Perf is optional and absent on Tops: install with
`sudo pacman -Syu --needed perf`. Optional formatters: `sudo pacman -Syu --needed ruff stylua`.
Missing sudo credentials block package changes only. Existing Qt profiler, heaptrack,
strace and hyperfine remain available. No actual runtime Timer or periodic Process
loop is present; finite benchmark readiness checks are intentionally separate.

Profiles/default contains portable automatic monitor and input defaults. Tops owns
only host overrides. An absent lucky38 profile or missing category falls back to
default; create profiles/lucky38 when real hardware is known. Shared input/animation/
bindings never encode output names, resolution, refresh rate or GPU paths.

`python3 scripts/failure.py` performs one real supervised shell crash and isolated
bad-config/backend checks; keep it separate from routine pure unit tests.
`bench --trace` appends finite process tracing. Probe lifecycle counters and debug
raw-event counters are exposed through `quickshell ipc --path shell call foundation status`.
