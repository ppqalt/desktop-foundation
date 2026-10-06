# Performance measurement

Record software versions, profile, output refresh rate and scale, hardware,
workload and power state with each comparison. Keep fresh shell processes and
processes that have already opened a surface in separate groups. Compare repeated
samples under the same conditions.

## Shell processes

```sh
python3 scripts/baseline.py PID --seconds 10
python3 scripts/startup.py
scripts/bench --runs 3 --seconds 5 --output work/bench.json
scripts/bench --runs 3 --seconds 5 --trace
```

`baseline.py` reports RSS, PSS, CPU time as a percentage of one core and main-thread
context switches. `startup.py` measures launch through backend readiness, including
readiness-probe overhead, and cleans up its process.

`bench` starts owned shell processes and records fresh and warmed samples, memory,
CPU ticks, surviving-thread context-switch deltas, process-family endpoints,
backend readiness and probe creation/destruction IPC roundtrips. `--trace` adds a
finite process-execution trace. Process endpoints miss short-lived subprocesses;
use the trace when measuring process creation.

CPU ticks have kernel sampling resolution. Context switches measure scheduling
activity; wakeup sources require a scheduling trace. PSS accounts for shared pages.
Popup IPC timings include the harness and end at reported creation/focus readiness.
First-frame presentation requires separate compositor or graphics instrumentation.

## Sessions and boots

```sh
scripts/doctor --startup --save before
scripts/doctor --startup --save after
scripts/doctor --startup --compare
scripts/doctor --startup --benchmark
python3 scripts/boot-report.py --boots 4 --save before
```

Startup samples are saved under
`$XDG_CACHE_HOME/desktop-foundation/startup/samples`. Boot reports are saved under
`~/.cache/desktop-foundation/boot-audit/optimization`.

The startup report uses service and journal timestamps; retained user-manager
events are separated from the current session. Comparison deduplicates captures
of the same Niri process launch. Keep cold-boot logins and same-boot relogins in
separate groups. Close foundation popups before the explicit popup benchmark.

Boot reports separate firmware, loader, kernel, initrd and userspace stages.
Menu interaction affects loader time. Keep kernel source and journal receipt
timestamps separate when calculating intervals.

## Profiling

```sh
python3 scripts/shell-profile.py --seconds 15
python3 scripts/shell-profile.py --seconds 15 --popups
```

The profiler samples the resident shell and captures a QML trace from an isolated
shell copy. `--popups` adds open/close cycles. Logs, JSON and `shell-qml.qtd` are saved
under `~/.cache/desktop-foundation/boot-audit/optimization`. The debug trace adds
overhead, so compare ordinary measurements separately.

For targeted native profiling, use a finite capture:

```sh
perf stat -p PID -e task-clock,context-switches,cpu-migrations -- sleep 30
timeout 30s strace -f -c -p PID
heaptrack quickshell --path shell
```

Check available perf events and kernel permissions before capturing. Use QML
profiling alongside heaptrack to inspect binding evaluation, allocation and
animation activity. See [tooling](tooling.md) for dependencies and
[startup](startup.md) for the session service layout.
