#!/usr/bin/env python3
"""Finite /proc measurement; no resident polling component."""
import argparse
import json
import os
from pathlib import Path
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('pid', type=int)
parser.add_argument('--seconds', type=float, default=10)
args = parser.parse_args()
if args.seconds <= 0 or args.seconds > 60:
    parser.error('seconds must be in (0, 60]')
proc = Path('/proc') / str(args.pid)

def sample():
    fields = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
    ticks = int(fields[11]) + int(fields[12])
    switches = {}
    for line in (proc / 'status').read_text().splitlines():
        if 'ctxt_switches:' in line:
            key, value = line.split(':')
            switches[key] = int(value.strip())
    memory = {}
    for line in (proc / 'smaps_rollup').read_text().splitlines():
        if line.startswith(('Rss:', 'Pss:')):
            key, value, unit = line.split()
            memory[key.rstrip(':') + '_KiB'] = int(value)
    return ticks, switches, memory

start = time.monotonic()
ticks, switches, _ = sample()
time.sleep(args.seconds)
end_ticks, end_switches, memory = sample()
elapsed = time.monotonic() - start
print(json.dumps({'pid': args.pid, 'duration_s': elapsed,
    'cpu_percent_one_core': 100 * (end_ticks - ticks) / os.sysconf('SC_CLK_TCK') / elapsed,
    'main_thread_context_switches_per_s': {k: (end_switches[k] - v) / elapsed for k, v in switches.items()},
    **memory}, indent=2))
