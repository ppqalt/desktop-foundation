# Development tools

These tools are separate from installed runtime commands. Normal workflows are
in `./scripts/dev --help`; automated regression tests are in `tests/`.

| Directory | Purpose |
| --- | --- |
| `benchmarks/` | Finite synthetic comparisons against frozen Git baselines |
| `live/` | Explicit interactive acceptance checks that touch the active session |
| `fixtures/` | Inert data adapters used by the component gallery and tests |

Benchmark tools accept `--help`; their `ROOT` is the checkout containing this
directory. Historical paths inside `git show REV:PATH` refer to the frozen
revision and intentionally retain their old names. They do not require those
files in the current checkout. Reports remain under ignored `work/` or the
explicit output path.

`python3 tools/live/compositor.py` creates temporary windows, changes focus and
restores it. Run it only when that live interaction is explicitly wanted; it is
not part of routine checks. The removed `clipboard-smoke.py` and
`launcher-smoke.py` depended directly on obsolete Hyprland IPC/uinput routes;
current clipboard/launcher regressions use the isolated tests in `tests/`.

`fixtures/gallery-backend` provides only sample wallpaper responses. It never
changes real wallpaper, audio, recording or power state. Gallery and motion
tests share this same fixture.
