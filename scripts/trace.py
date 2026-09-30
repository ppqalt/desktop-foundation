#!/usr/bin/env python3
"""Finite owned-process exec tracing; leaves the resident shell untouched."""
import argparse
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=float, default=3)
    args = parser.parse_args()
    if not 0 < args.seconds <= 30:
        parser.error('seconds must be in (0,30]')
    if not shutil.which('strace'):
        print('Unavailable: install sudo pacman -Syu --needed strace')
        return 0
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory() as temporary:
        log = Path(temporary) / 'process.log'
        process = subprocess.Popen(['strace', '-f', '-e', 'trace=process', '-o', str(log), 'quickshell', '--path', str(root / 'shell')], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        children = []
        try:
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError('strace unavailable: check ptrace permissions')
                children = Path(f'/proc/{process.pid}/task/{process.pid}/children').read_text().split()
                if children:
                    break
                time.sleep(.01)
            if not children:
                raise RuntimeError('No traced child')
            time.sleep(args.seconds)
        finally:
            for child in children:
                try:
                    os.kill(int(child), signal.SIGTERM)
                except ProcessLookupError:
                    pass
            if not children and process.poll() is None:
                process.terminate()
            process.wait(timeout=5)
        execs = [line for line in log.read_text().splitlines() if 'execve(' in line or 'execveat(' in line]
        print(f'{args.seconds}s trace: {len(execs)} exec requests, including initial Quickshell launch; no conclusion about future wakeups.')
        if len(execs) > 1:
            print('\n'.join(execs))


if __name__ == '__main__':
    main()
