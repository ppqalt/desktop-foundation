#!/usr/bin/env python3
"""Bounded Bluetooth radio action; no resident process or privileged service."""
import json
import subprocess
import sys
import time


def run(*args):
    return subprocess.run(args, capture_output=True, text=True, timeout=2, check=True).stdout


def power(enabled):
    paths = [line.split()[0] for line in run('busctl', '--system', '--list', 'tree', 'org.bluez').splitlines()
             if line.split() and line.split()[0].startswith('/org/bluez/hci') and '/dev_' not in line]
    paths = [p for p in paths if p.rsplit('/', 1)[-1].removeprefix('hci').isdigit()]
    if not paths:
        raise RuntimeError('No Bluetooth adapter is available.')
    if enabled:
        run('rfkill', 'unblock', 'bluetooth')
    deadline = time.monotonic() + 5
    pending = set(paths)
    while pending and time.monotonic() < deadline:
        for path in tuple(pending):
            try:
                run('busctl', '--system', 'set-property', 'org.bluez', path,
                    'org.bluez.Adapter1', 'Powered', 'b', 'true' if enabled else 'false')
                state = json.loads(run('busctl', '--system', '--json=short', 'get-property',
                                       'org.bluez', path, 'org.bluez.Adapter1', 'Powered'))['data']
                if state == enabled:
                    pending.remove(path)
            except subprocess.CalledProcessError:
                # BlueZ can reject power-on briefly after an rfkill unblock.
                pass
        if pending:
            time.sleep(0.15)
    if pending:
        raise RuntimeError('Bluetooth could not be turned ' + ('on. Check the hardware radio switch or airplane mode.' if enabled else 'off. Try again.'))
    return {'success': True, 'powered': enabled}


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2 or sys.argv[1] not in ('on', 'off'):
            raise RuntimeError('Expected on or off.')
        result = power(sys.argv[1] == 'on')
    except (RuntimeError, subprocess.SubprocessError, ValueError, KeyError) as exc:
        result = {'success': False, 'error': str(exc) if isinstance(exc, RuntimeError) else 'Bluetooth radio request failed. Check adapter availability and permissions.'}
    print(json.dumps(result))
    sys.exit(0 if result['success'] else 1)
