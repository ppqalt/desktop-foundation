#!/usr/bin/env python3
"""Opt-in isolated QML trace and current-shell idle/memory measurements."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent


def memory(pid):
    values = {}
    for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines():
        fields = line.split()
        if len(fields) == 3 and fields[2] == 'kB':
            values[fields[0].rstrip(':')] = int(fields[1])
    return values


def counters(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    switches = 0
    for status in Path(f'/proc/{pid}/task').glob('*/status'):
        try:
            switches += sum(int(line.split()[1]) for line in status.read_text().splitlines()
                            if line.startswith(('voluntary_ctxt_switches:', 'nonvoluntary_ctxt_switches:')))
        except FileNotFoundError:
            pass
    return {'ticks': int(fields[11]) + int(fields[12]), 'thread_context_switches': switches}


def idle(pid, seconds):
    before = counters(pid)
    start = time.monotonic()
    time.sleep(seconds)
    elapsed = time.monotonic() - start
    after = counters(pid)
    return {'seconds': elapsed,
            'cpu_percent_of_one_core': (after['ticks'] - before['ticks']) / os.sysconf('SC_CLK_TCK') / elapsed * 100,
            'thread_context_switches_per_second': (after['thread_context_switches'] - before['thread_context_switches']) / elapsed}


def ipc(path, method):
    return json.loads(subprocess.check_output(['quickshell', 'ipc', '--path', str(path), 'call', 'foundation', method],
                                             text=True, stderr=subprocess.DEVNULL, timeout=5) or 'null')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=int, default=15)
    parser.add_argument('--popups', action='store_true', help='Briefly open/close isolated popups; never activate actions')
    args = parser.parse_args()
    if not 5 <= args.seconds <= 45:
        parser.error('Sample duration must be 5–45 seconds')
    folder = Path.home() / '.cache/desktop-foundation/boot-audit/optimization'
    folder.mkdir(parents=True, exist_ok=True)
    pid = int(subprocess.check_output(['systemctl', '--user', 'show', 'desktop-foundation-shell.service', '-p', 'MainPID', '--value'], text=True))
    report = {'live_before_kib': memory(pid), 'live_idle': idle(pid, args.seconds)}
    profiler = None
    shell = None
    trace = folder / 'shell-qml.qtd'
    trace.unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix='foundation-qml-profile-') as temporary:
        path = Path(temporary) / 'shell'
        shutil.copytree(ROOT / 'shell', path)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        try:
            with (folder / 'qml-profile.log').open('w') as log:
                shell = subprocess.Popen(['quickshell', '--path', str(path), '--debug', str(port), '--waitfordebug'],
                                         stdout=log, stderr=subprocess.STDOUT)
                profiler = subprocess.Popen(['/usr/lib/qt6/bin/qmlprofiler', '--attach', '127.0.0.1', '--port', str(port),
                                             '--interactive', '--output', str(folder / 'shell-qml.qtd')],
                                            stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True)
                deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    if shell.poll() is not None or profiler.poll() is not None:
                        raise RuntimeError('Profiler/shell exited; inspect qml-profile.log')
                    try:
                        state = ipc(path, 'status')
                        if state['ready']:
                            break
                    except (subprocess.SubprocessError, ValueError):
                        pass
                    time.sleep(.1)
                else:
                    raise RuntimeError('Isolated shell readiness timeout')
                # Keep reports free of window titles and clipboard contents.
                report['isolated_root'] = {key: state[key] for key in ('launcherAlive', 'clipboardAlive', 'probeAlive', 'ready')}
                report['isolated_initial_kib'] = memory(shell.pid)
                if args.popups:
                    samples = []
                    for _ in range(3):
                        for show, close in [('showLauncher', 'hideLauncher'), ('showClipboard', 'hideClipboard'),
                                            ('toggleBluetooth', 'toggleBluetooth'), ('togglePower', 'togglePower')]:
                            ipc(path, show)
                            time.sleep(.5)
                            ipc(path, close)
                            time.sleep(.5)
                        state = ipc(path, 'status')
                        samples.append({'kib': memory(shell.pid),
                                        'launcher_alive': state['launcherAlive'], 'clipboard_alive': state['clipboardAlive'],
                                        'bluetooth_alive': ipc(path, 'bluetoothStatus').get('alive', False),
                                        'power_visible': ipc(path, 'powerStatus')['visible']})
                    report['isolated_closed_popup_cycles'] = samples
                report['isolated_idle'] = idle(shell.pid, args.seconds)
                report['isolated_final_kib'] = memory(shell.pid)
                profiler.stdin.write('flush\n')
                profiler.stdin.flush()
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    try:
                        ET.parse(trace)
                        break
                    except (OSError, ET.ParseError):
                        time.sleep(.1)
                else:
                    raise RuntimeError('Profiler did not finish writing a complete trace')
                trace.chmod(0o600)
                # Qt's interactive reader can leave a second piped command
                # buffered. Export one command, then terminate the collector.
                profiler.terminate()
                profiler.wait(timeout=5)
        finally:
            for process in (profiler, shell):
                if process is not None and process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
    report['live_after_kib'] = memory(pid)
    report['limitations'] = ['Debug tracing adds overhead; isolated startup is not a real login benchmark.',
                             'RSS includes shared pages; use PSS for proportional attribution.',
                             'Thread context switches are a scheduling proxy, not an exact timer-wakeup count.',
                             'Allocator/graphics caches may retain freed pages; a higher RSS alone is not a leak.']
    target = folder / 'shell-profile.json'
    target.write_text(json.dumps(report, indent=2) + '\n')
    target.chmod(0o600)
    print(json.dumps(report, indent=2))
    print('Trace/log/report saved in ' + str(folder) + '; isolated process cleaned up.')


if __name__ == '__main__':
    main()
