"""Read-only current-session timeline; explicit IPC benchmark is opt-in."""
import argparse
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
UNITS = {
    'niri': 'niri.service', 'shell': 'desktop-foundation-shell.service',
    'notifications': 'desktop-foundation-notifications.service',
    'clipboard_init': 'desktop-foundation-clipboard-init.service',
    'clipboard_text': 'desktop-foundation-clipboard@text.service',
    'clipboard_image': 'desktop-foundation-clipboard@image.service',
    'clipboard_persist': 'desktop-foundation-clipboard-persist.service',
    'wallpaper': 'desktop-foundation-wallpaper.service', 'polkit': 'hyprpolkitagent.service',
    'portal': 'xdg-desktop-portal.service', 'psd': 'psd.service', 'default': 'default.target',
}


def run(*args):
    result = subprocess.run(args, text=True, capture_output=True, timeout=20)
    return result.stdout if result.returncode == 0 else ''


def decode_message(value):
    if isinstance(value, list):
        value = bytes(value).decode(errors='replace')
    return re.sub(r'\x1b\[[0-9;]*m', '', value)


def journal(user=True):
    args = ['journalctl', *(['--user'] if user else []), '-b', '-o', 'json', '--no-pager']
    if not user:
        args.extend(['-u', 'greetd.service'])
    for line in run(*args).splitlines():
        try:
            row = json.loads(line)
            yield int(row['__MONOTONIC_TIMESTAMP']) / 1e6, row.get('_SYSTEMD_USER_UNIT', ''), decode_message(row.get('MESSAGE', ''))
        except (ValueError, KeyError, TypeError):
            continue


def delta(events, start, end):
    return round((events[end] - events[start]) * 1000, 2) if start in events and end in events else None


def collect():
    events = {}
    units = {}
    for key, unit in UNITS.items():
        raw = run('systemctl', '--user', 'show', unit, '-p', 'ExecMainStartTimestampMonotonic',
                  '-p', 'ActiveEnterTimestampMonotonic', '-p', 'Type', '-p', 'LoadState')
        props = dict(line.split('=', 1) for line in raw.splitlines() if '=' in line)
        units[key] = props
        for field, label in [('ExecMainStartTimestampMonotonic', 'launch'), ('ActiveEnterTimestampMonotonic', 'active')]:
            if int(props.get(field, '0')):
                events[key + '_' + label] = int(props[field]) / 1e6
    niri = events.get('niri_launch', 0)
    shell = events.get('shell_launch', float('inf'))
    reload_requests = []
    reload_durations = []
    for stamp, unit, message in journal():
        if stamp < niri:
            continue
        if unit == 'niri.service' and 'listening on Wayland socket:' in message:
            events.setdefault('niri_wayland_socket', stamp)
        if unit == 'desktop-foundation-shell.service' and stamp >= shell and 'Configuration Loaded' in message:
            events.setdefault('shell_loaded', stamp)
        if stamp <= shell and 'Reload requested from client' in message:
            reload_requests.append({'boot_seconds': stamp, 'foundation': 'session\\x2dstart' in message or '(unit niri.service)' in message})
        if stamp <= shell and 'Reloading finished in' in message:
            match = re.search(r'finished in (\d+) ms', message)
            if match:
                reload_durations.append(int(match[1]))
    for stamp, _, message in journal(False):
        if stamp <= niri and 'session opened for user ' + os.environ.get('USER', '') + '(' in message:
            events['session_open'] = stamp
    metrics = {
        'niri_ready_to_shell_launch_ms': delta(events, 'niri_active', 'shell_launch'),
        'shell_launch_to_loaded_ms': delta(events, 'shell_launch', 'shell_loaded'),
        'niri_launch_to_shell_loaded_ms': delta(events, 'niri_launch', 'shell_loaded'),
        'session_open_to_niri_ready_ms': delta(events, 'session_open', 'niri_active'),
        'foundation_reload_requests_before_shell': sum(r['foundation'] for r in reload_requests),
        'all_reload_duration_before_shell_ms': sum(reload_durations),
    }
    origin = events.get('session_open', niri)
    prior_events = {k: v for k, v in events.items() if v < origin}
    return {'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
            'niri_launch_boot_seconds': niri, 'events_boot_seconds': events,
            'timeline_ms': {k: round((v - origin) * 1000, 2) for k, v in events.items() if v >= origin},
            'previous_user_manager_events_boot_seconds': prior_events,
            'origin': 'session_open' if 'session_open' in events else 'niri_launch',
            'metrics': metrics, 'default_target_report': run('systemd-analyze', '--user', 'time').strip(),
            'limitations': ['Type=simple/exec active means launched, not visually ready.',
                            'Wallpaper first presentation and precise first launcher frame are not instrumented.',
                            'Configuration Loaded is QML initialization, not proof of input or presentation.',
                            'No fabricated values when journal evidence is unavailable.',
                            'Default target/PSD may belong to a retained user manager from an earlier login.']}


def benchmark():
    def ipc(method):
        return run('quickshell', 'ipc', '--path', str(ROOT / 'shell'), 'call', 'foundation', method)
    for name in ['launcherStatus', 'clipboardStatus', 'bluetoothStatus', 'powerStatus']:
        value = ipc(name)
        if not value:
            raise RuntimeError('Resident shell unavailable; benchmark never starts another shell.')
        if json.loads(value).get('visible'):
            raise RuntimeError('Close open foundation popups before benchmarking.')
    results = {}
    actions = [('launcher', 'showLauncher', 'hideLauncher'), ('clipboard', 'showClipboard', 'hideClipboard'),
               ('bluetooth', 'toggleBluetooth', 'toggleBluetooth'), ('power', 'togglePower', 'togglePower')]
    try:
        for name, show, hide in actions:
            started = time.monotonic()
            ipc(show)
            deadline = started + 5
            while time.monotonic() < deadline:
                state = json.loads(ipc(name + 'Status') or '{}')
                if state.get('visible') and (name not in ('launcher', 'clipboard') or state.get('inputFocused')):
                    results[name] = {'creation_focus_ipc_roundtrip_ms': round((time.monotonic() - started) * 1000, 2)}
                    break
                time.sleep(.01)
            else:
                raise RuntimeError(name + ' failed to become ready.')
            ipc(hide)
            while json.loads(ipc(name + 'Status') or '{}').get('visible') and time.monotonic() < deadline:
                time.sleep(.02)
            time.sleep(.2)
    finally:
        for name, _, hide in actions:
            if json.loads(ipc(name + 'Status') or '{}').get('visible'):
                ipc(hide)
    return {'surfaces': results, 'limitations': ['Resident-shell IPC benchmark, not a login benchmark.',
              'Creation/focus roundtrip is not compositor presentation time or a physical keypress.',
              'Bluetooth discovery is on-demand; no connection, radio, audio or power action is invoked.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--benchmark', action='store_true')
    parser.add_argument('--save', metavar='LABEL', help='Save sanitized metrics outside the checkout')
    parser.add_argument('--compare', action='store_true', help='Summarize saved real-session metrics by label')
    args = parser.parse_args()
    directory = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'desktop-foundation/startup/samples'
    if args.compare:
        groups = {}
        for file in sorted(directory.glob('*.json')):
            sample = json.loads(file.read_text())
            groups.setdefault(sample['label'], {})[(sample['boot_id'], sample['niri_launch_boot_seconds'])] = sample
        for label, samples in groups.items():
            print(label, 'unique sessions:', len(samples))
            for key in next(iter(samples.values()))['metrics']:
                values = [s['metrics'][key] for s in samples.values() if s['metrics'][key] is not None]
                if values:
                    print(key, 'median/min/max:', statistics.median(values), min(values), max(values))
        return
    report = collect()
    if args.benchmark:
        report['benchmark'] = benchmark()
    if args.save:
        if not re.fullmatch(r'[A-Za-z0-9_-]+', args.save):
            parser.error('Use a simple label.')
        report['label'] = args.save
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        file = directory / (args.save + '-' + report['boot_id'] + '-' + str(report['niri_launch_boot_seconds']) + '.json')
        file.write_text(json.dumps(report, indent=2) + '\n')
        file.chmod(0o600)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
