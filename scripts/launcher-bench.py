#!/usr/bin/env python3
"""Finite resident-shell launcher measurements; restore hidden state on exit."""
import importlib.util
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('bench', ROOT / 'scripts/bench.py')
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)
pid = int(subprocess.check_output(['systemctl', '--user', 'show', '-p', 'MainPID', '--value', 'desktop-foundation-shell.service'], text=True))
try:
    bench.ipc(pid, 'hideLauncher')
    bench.wait_status(pid, lambda s: not s['launcherAlive'])
    closed = bench.measure(pid, 5, 'launcher hidden, warmed resident')
    started = time.monotonic()
    bench.ipc(pid, 'showLauncher')
    bench.wait_status(pid, lambda s: s['launcherAlive'])
    opened_ms = (time.monotonic() - started) * 1000
    time.sleep(.5)
    opened = bench.measure(pid, 5, 'launcher open, focused text caret')
    started = time.monotonic()
    bench.ipc(pid, 'hideLauncher')
    bench.wait_status(pid, lambda s: not s['launcherAlive'])
    closed_ms = (time.monotonic() - started) * 1000
    after = bench.measure(pid, 5, 'launcher destroyed, warmed resident')
    print(json.dumps({'samples': [closed, opened, after], 'creation_roundtrip_ms': opened_ms, 'exit_and_destruction_roundtrip_ms': closed_ms, 'limitations': ['Single warmed resident process; retained Qt/icon caches are included', 'Finite IPC roundtrips are not first-frame presentation latency', 'Open text cursor intentionally wakes; no zero-wakeup claim']}, indent=2))
finally:
    bench.ipc(pid, 'hideLauncher')
