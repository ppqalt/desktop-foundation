#!/usr/bin/env python3
"""Finite repeated measurements; process-cold does not mean cold page cache."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def ipc(pid, method):
    result = subprocess.run(['quickshell', 'ipc', '--pid', str(pid), 'call', 'foundation', method], capture_output=True, text=True, timeout=3)
    if result.returncode:
        return None
    try:
        return json.loads(result.stdout) if method == 'status' else result.stdout
    except json.JSONDecodeError:
        return None


def snapshot(pid):
    proc = Path('/proc') / str(pid)
    stat = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
    threads = {}
    for task in (proc / 'task').iterdir():
        try:
            threads[task.name] = sum(int(line.split(':')[1]) for line in (task / 'status').read_text().splitlines() if 'ctxt_switches:' in line)
        except FileNotFoundError:
            pass
    memory = {line.split()[0].rstrip(':') + '_KiB': int(line.split()[1]) for line in (proc / 'smaps_rollup').read_text().splitlines() if line.startswith(('Rss:', 'Pss:'))}
    return int(stat[11]) + int(stat[12]), threads, memory


def descendants(pid):
    parents = {}
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            parents[int(proc.name)] = int((proc / 'stat').read_text().rsplit(')', 1)[1].split()[1])
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    found = {pid}
    while True:
        added = {child for child, parent in parents.items() if parent in found} - found
        if not added:
            return sorted(found)
        found |= added


def wait_status(pid, predicate):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        state = ipc(pid, 'status')
        if state and predicate(state):
            return state
        time.sleep(.01)  # Finite benchmark harness only, never resident shell.
    raise RuntimeError('Timed out waiting for shell state')


def measure(pid, seconds, phase):
    ticks, threads, _ = snapshot(pid)
    before = time.monotonic()
    family_before = descendants(pid)
    time.sleep(seconds)
    end_ticks, end_threads, memory = snapshot(pid)
    elapsed = time.monotonic() - before
    common = threads.keys() & end_threads.keys()
    return {'phase': phase, 'duration_s': elapsed, **memory,
            'cpu_percent_one_core': 100 * (end_ticks - ticks) / os.sysconf('SC_CLK_TCK') / elapsed,
            'all_surviving_threads_context_switches_per_s': sum(end_threads[t] - threads[t] for t in common) / elapsed,
            'thread_set_changed': threads.keys() != end_threads.keys(),
            'processes_before': family_before, 'processes_after': descendants(pid),
            'transient_subprocess_observation': 'not captured by endpoint snapshots; use strace -f -e trace=process for exact finite exec tracing'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--seconds', type=float, default=5)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--trace', action='store_true', help='append finite owned-process exec tracing')
    args = parser.parse_args()
    if not 1 <= args.runs <= 10 or not 0 < args.seconds <= 30:
        parser.error('runs must be 1..10 and seconds (0,30]')
    records = []
    for iteration in range(args.runs):
        with tempfile.TemporaryFile(mode='w+') as log:
            started = time.monotonic()
            process = subprocess.Popen(['quickshell', '--path', str(ROOT / 'shell'), '--no-color'], stdout=log, stderr=log)
            try:
                wait_status(process.pid, lambda s: s['ready'])
                ready_ms = (time.monotonic() - started) * 1000
                cold = measure(process.pid, args.seconds, 'fresh-process, warm filesystem cache')
                started = time.monotonic()
                ipc(process.pid, 'showProbe')
                wait_status(process.pid, lambda s: s['probeAlive'])
                create_ms = (time.monotonic() - started) * 1000
                started = time.monotonic()
                ipc(process.pid, 'hideProbe')
                wait_status(process.pid, lambda s: not s['probeAlive'])
                destroy_ms = (time.monotonic() - started) * 1000
                warm = measure(process.pid, args.seconds, 'warm-process after surface use')
                records.append({'run': iteration + 1, 'ready_ms': ready_ms,
                                'probe_request_roundtrip_create_ms': create_ms,
                                'probe_request_roundtrip_destroy_ms': destroy_ms,
                                'idle': [cold, warm]})
            finally:
                process.terminate()
                process.wait(timeout=5)
    report = {'runs': records, 'ready_median_ms': statistics.median(r['ready_ms'] for r in records),
              'limitations': ['Probe timings include IPC harness overhead; not first-frame latency', 'No dropped caches; no zero-wakeup claim', 'PSS includes sharing with the resident shell']}
    output = json.dumps(report, indent=2) + '\n'
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output)
    print(output)
    if args.trace:
        subprocess.run(['python3', str(ROOT / 'scripts/trace.py')], check=True)


if __name__ == '__main__':
    main()
