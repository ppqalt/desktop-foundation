#!/usr/bin/env python3
"""Measure launch to backend-ready; test-only readiness probes, own PID cleanup."""
import json
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parent.parent
started = time.monotonic()
process = subprocess.Popen(['quickshell', '--path', str(root / 'shell'), '--no-color'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    while time.monotonic() - started < 10:
        if process.poll() is not None:
            raise RuntimeError(f'Quickshell exited: {process.returncode}')
        result = subprocess.run(['quickshell', 'ipc', '--pid', str(process.pid), 'call', 'foundation', 'status'], capture_output=True, text=True)
        try:
            status = json.loads(result.stdout)
        except json.JSONDecodeError:
            status = {}
        if status.get('ready'):
            print(json.dumps({'backend_ready_ms': (time.monotonic() - started) * 1000}))
            break
        time.sleep(0.01)
    else:
        raise RuntimeError('Timed out waiting for backend readiness')
finally:
    if process.poll() is None:
        process.terminate()
    process.wait(timeout=5)
