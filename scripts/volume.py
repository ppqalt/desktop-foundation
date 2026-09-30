#!/usr/bin/env python3
"""Three-percent volume steps with a replacing, transient native readout."""
import argparse
import fcntl
import os
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('direction', choices=['up', 'down'])
    args = parser.parse_args()
    runtime = Path(os.environ['XDG_RUNTIME_DIR'])
    with (runtime / 'desktop-foundation-volume.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        subprocess.run(['wpctl', 'set-volume', '-l', '1', '@DEFAULT_AUDIO_SINK@',
                        '3%+' if args.direction == 'up' else '3%-'], check=True)
        status = subprocess.check_output(['wpctl', 'get-volume', '@DEFAULT_AUDIO_SINK@'], text=True)
        match = re.search(r'Volume:\s*([\d.]+)', status)
        if not match:
            raise RuntimeError('Unable to read output volume')
        percent = round(float(match.group(1)) * 100)
        muted = '[MUTED]' in status
        summary = f'Muted · {percent}%' if muted else f'Volume {percent}%'
        filled = round((0 if muted else min(percent, 100)) * 12 / 100)
        bar = '━' * filled + '─' * (12 - filled)
        subprocess.run(['notify-send', '--app-name', 'desktop-foundation-volume',
                        '--hint', 'string:x-canonical-private-synchronous:desktop-foundation-volume',
                        '--expire-time', '1500', summary + '  ' + bar], check=True)


if __name__ == '__main__':
    main()
