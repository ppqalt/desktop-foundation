#!/usr/bin/env python3
"""Isolated fault checks plus one supervised shell crash; never exit compositor."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def command(*args, **kwargs):
    return subprocess.run(args, capture_output=True, text=True, timeout=10, **kwargs)


def main():
    niri = bool(os.environ.get('NIRI_SOCKET'))
    query = ('niri', 'msg', '-j', 'windows') if niri else ('hyprctl', '-j', 'clients')
    key = 'id' if niri else 'address'
    before = json.loads(command(*query).stdout)
    identities = {w[key] for w in before}
    pid = int(command('systemctl', '--user', 'show', '-p', 'MainPID', '--value', 'desktop-foundation-shell.service').stdout)
    os.kill(pid, signal.SIGKILL)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        new_pid = int(command('systemctl', '--user', 'show', '-p', 'MainPID', '--value', 'desktop-foundation-shell.service').stdout)
        if new_pid and new_pid != pid:
            result = command('quickshell', 'ipc', '--pid', str(new_pid), 'call', 'foundation', 'status')
            try:
                if json.loads(result.stdout)['ready']:
                    break
            except json.JSONDecodeError:
                pass
        time.sleep(.05)
    else:
        raise RuntimeError('Supervised shell did not recover')
    after = json.loads(command(*query).stdout)
    assert identities <= {w[key] for w in after}, 'Existing clients must survive shell failure'
    for override in [{'DF_COMPOSITOR': 'unsupported'}, ({'DF_COMPOSITOR': 'niri', 'NIRI_SOCKET': '/tmp/nonexistent-niri-socket'} if niri else {'DF_COMPOSITOR': 'hyprland', 'HYPRLAND_INSTANCE_SIGNATURE': 'nonexistent-test-instance'})]:
        result = command(str(ROOT / 'scripts/run-shell'), env={**os.environ, **override})
        assert result.returncode and ('unavailable' in result.stderr or 'Unsupported' in result.stderr)
    with tempfile.TemporaryDirectory() as temporary:
        path = Path(temporary) / 'shell.qml'
        path.write_text('import Quickshell\nShellRoot { invalid syntax !!! }')
        result = command('quickshell', '--path', temporary, '--no-color')
        assert result.returncode and 'ERROR' in result.stderr + result.stdout
    assert command(*(('niri', 'msg', '-j', 'outputs') if niri else ('hyprctl', '-j', 'monitors'))).returncode == 0
    print('PASS: supervised crash/restart, clients survive, unavailable IPC, unsupported compositor, isolated syntax failure')


if __name__ == '__main__':
    main()
