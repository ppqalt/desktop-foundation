# Measure before optimizing

Always record software versions, profile, monitor refresh/scale, hardware, workload, power state and whether the probe/surface has ever opened. Distinguish an invisible fresh shell from a hidden shell after GPU resources were allocated. Do not interpret an empty foundation as the budget for finished visuals.

Run `scripts/baseline.py PID --seconds 10` for RSS/PSS, CPU time as percent of one core, and main-thread voluntary/involuntary context switches. Context switches are NOT hardware wakeups, and zero CPU ticks is limited by kernel tick resolution. Capture longer controlled samples manually for very low idle activity. `/proc/PID/task/*/status` or a task-aware profiler is needed for all-thread context switches.

Run `scripts/startup.py` for elapsed launch-to-backend-ready including the cost/resolution of test IPC readiness checks. `hyperfine --warmup 1 --runs 5 'scripts/startup.py'` measures the whole harness including process teardown and Python/IPC overhead; do not call that pure startup latency. Cold and warm startup should be measured separately. Readiness here does not assert the first visible frame.

For wakeups/scheduling, optional `perf stat -p PID -e task-clock,context-switches,cpu-migrations -- sleep 30` provides scheduling counters. Exact wakeup sources require tracepoints (e.g. sched:sched_wakeup) and appropriate kernel permissions. Inspect `perf list` and permissions first; no permanent privilege relaxation. `strace -f -c -p PID` and a bounded `strace -f -e trace=process -p PID` identify syscalls/unnecessary spawning. Profilers perturb the workload; compare uninstrumented runs. Prefer native event integrations to polling.

QML tooling lives in /usr/lib/qt6/bin: qmllint, qmlformat, qmlls, qmlprofiler, qsb. Use `quickshell --debug 3768 --waitfordebug --path shell` in a separate development instance and attach qmlprofiler (`qmlprofiler --attach localhost --port 3768 --output work/profile.qtd`). See tool help for capture controls. Use Qt Creator if a GUI profiler is wanted later. Profile binding evaluations, allocations, animations and scene-graph frames during open/close transitions, then assess p50/p95/p99 frame times against the output's frame budget. Check supported Qt scene-graph logging for the installed release. Keep profiling/debug endpoints development-only.

Heaptrack can launch `heaptrack quickshell --path shell` for native allocations; it cannot by itself explain GPU memory or every QML object. Use Qt QML profiling alongside it. If a Rust backend is added, repeat CPU/PSS/allocations/startup measurements independently and track IPC latency and process count. Keep no invisible menu object trees alive without justification. Animations and blur remain desired; optimize concrete hot paths rather than disabling the visual design.

## Repeatable hardened harness

`scripts/bench` collects multiple fresh-process and warmed-process runs: RSS/PSS,
CPU ticks, all surviving threads' context-switch deltas, changed thread sets,
process-family endpoint counts, startup to backend-ready and actual probe object
creation/destruction request roundtrips. Timing includes IPC overhead and is not
first-frame presentation latency. Fresh process does not mean cold kernel caches.
PSS sharing and user workload affect results. Endpoint snapshots cannot rule out
short-lived subprocesses; attach `strace -f -e trace=process -p PID` for a finite
window. Continuous runtime polling remains prohibited. Zero CPU ticks at the
kernel's sampling resolution does not imply zero wakeups.

Use `scripts/bench --trace` to additionally launch an owned shell under finite
strace process tracing; the resident instance is untouched. Latest finite trace
observed exactly one exec request (initial launch) and no subprocess execs.

## Paired backend comparison

Build `scripts/build-backend`, then run:

```sh
python3 scripts/quality_bench.py --baseline-ref bbddde0 --samples 24 --image-samples 10 --output work/quality-comparison.json
```

This finite test reads the old clipboard worker through Git, uses synthetic text
and signature-only image payloads, and alternates old/new order after two warmups.
Each sample creates a private fresh database on the checkout's filesystem. Index
projections must match apart from timestamps/state paths. It never reads or changes
the real clipboard. The optional `bench` Cargo feature builds a small Rust wait4
driver so the Python driver's pre-exec memory does not dominate child peak RSS.
Regular runtime builds do not include that measurement executable.

Record wall time, child user/system CPU time and peak child RSS separately from
resident PSS. Btrfs/fsync and background activity affect timings. Live idle samples
with unchanged desktop code cannot establish an optimization gain. See the current
code-quality review for results, methodology and remaining activation checks.
