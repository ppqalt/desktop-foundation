#!/usr/bin/env python3
"""Measure Bluetooth action helper overhead with private synthetic services only."""
import argparse
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BASELINE = '72f6d8d7f21448a5a7af4812ad0a34ea2aca65ee'
ADDRESS = 'AB:CD:EF:01:02:03'
DEVICE = '/org/bluez/hci7/dev_' + ADDRESS.replace(':', '_')


def stopped(pid):
    try:
        return 'State:\tZ' in Path(f'/proc/{pid}/status').read_text()
    except FileNotFoundError:
        return True


def cleanup_subscription(path):
    if path.exists():
        pid = int(path.read_text())
        if not stopped(pid):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            return False
    return True


def invoke(command, env, base, expected):
    (base / 'connected').write_text('true' if env['DF_BENCH_ACTION'] == 'disconnect' else 'false')
    (base / 'mutations').write_bytes(b'')
    subscription = base / 'subscription.pid'
    subscription.unlink(missing_ok=True)
    process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=10)
    except BaseException:
        # The subprocess group belongs exclusively to this fixture invocation.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=2)
        raise
    finally:
        subscriber_stopped = cleanup_subscription(subscription)
    if process.returncode:
        raise RuntimeError(f'Fixture helper failed: {stderr.decode(errors="replace")}')
    if not subscriber_stopped:
        raise AssertionError('Fixture subscription survived the finite helper')
    result = json.loads(stdout)
    if result != expected:
        raise AssertionError(f'Unexpected fixture result: {result!r}; expected {expected!r}')
    return [[argument.decode() for argument in line.rstrip(b'\0').split(b'\0')]
            for line in (base / 'mutations').read_bytes().splitlines()]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=int, default=24)
    args = parser.parse_args()
    if not 4 <= args.runs <= 200:
        parser.error('--runs must be between 4 and 200')
    binary = ROOT / 'native/foundation/target/release/desktop-foundationctl'
    if not binary.is_file():
        parser.error('Build the release backend first with scripts/build-backend')
    with tempfile.TemporaryDirectory(prefix='foundation-bluetooth-actions-bench-') as directory:
        base = Path(directory)
        source = subprocess.check_output(
            ['git', '-C', str(ROOT), 'show', f'{BASELINE}:scripts/bluetooth-action.py'], timeout=5)
        legacy = base / 'bluetooth-action.py'
        legacy.write_bytes(source)
        fake = base / 'bin'
        fake.mkdir()
        for name in ['runtime', 'config', 'cache', 'state']:
            (base / name).mkdir(mode=0o700)
        # Only shell builtins and explicit absolute fixture tools are available.
        # Any unrecognized operation fails rather than reaching the real system.
        busctl = fake / 'busctl'
        busctl.write_text('''#!/bin/sh
case "$*" in
  *get-property*Paired) printf '%s\\n' '{"type":"b","data":true}' ;;
  *get-property*Powered) printf '%s\\n' '{"type":"b","data":true}' ;;
  *get-property*Blocked) printf '%s\\n' '{"type":"b","data":false}' ;;
  *get-property*Address) printf '%s\\n' '{"type":"s","data":"AB:CD:EF:01:02:03"}' ;;
  *get-property*UUIDs)
    if [ "$DF_BENCH_AUDIO" = 1 ]; then
      printf '%s\\n' '{"type":"as","data":["0000110b-0000-1000-8000-00805f9b34fb"]}'
    else printf '%s\\n' '{"type":"as","data":[]}'; fi ;;
  *get-property*Connected)
    read -r connected < "$DF_BENCH_BASE/connected"
    printf '{"type":"b","data":%s}\\n' "$connected" ;;
  *' call '*' Connect')
    printf '%s\\0' busctl "$@" >> "$DF_BENCH_BASE/mutations"
    printf '\\n' >> "$DF_BENCH_BASE/mutations"
    printf true > "$DF_BENCH_BASE/connected" ;;
  *' call '*' Disconnect')
    printf '%s\\0' busctl "$@" >> "$DF_BENCH_BASE/mutations"
    printf '\\n' >> "$DF_BENCH_BASE/mutations"
    printf false > "$DF_BENCH_BASE/connected" ;;
  *) exit 99 ;;
esac
''')
        card = {'name': 'bluez_card.fixture', 'active_profile': 'a2dp-sink',
                'properties': {'api.bluez5.path': DEVICE}, 'profiles': {
                    'a2dp-sink': {'description': 'codec LDAC', 'available': True, 'priority': 1},
                    'a2dp-sink-aac': {'available': True, 'priority': 10},
                    'headset-head-unit': {'available': True, 'priority': 999}}}
        sink = {'name': 'bluez_output.fixture', 'properties': {
                    'api.bluez5.address': ADDRESS, 'api.bluez5.profile': 'a2dp-sink',
                    'api.bluez5.codec': 'ldac'}}
        pactl = fake / 'pactl'
        pactl.write_text('''#!/bin/sh
case "$*" in
  subscribe)
    printf '%s' "$$" > "$DF_BENCH_BASE/subscription.pid"
    exec /bin/sleep 30 ;;
  '--format=json list cards') printf '%s\\n' 'CARDS' ;;
  '--format=json list sinks') printf '%s\\n' 'SINKS' ;;
  'set-card-profile bluez_card.fixture a2dp-sink'|'set-default-sink bluez_output.fixture')
    printf '%s\\0' pactl "$@" >> "$DF_BENCH_BASE/mutations"
    printf '\\n' >> "$DF_BENCH_BASE/mutations" ;;
  *) exit 99 ;;
esac
'''.replace('CARDS', json.dumps([card])).replace('SINKS', json.dumps([sink])))
        busctl.chmod(0o755)
        pactl.chmod(0o755)
        # Deliberately do not inherit desktop, D-Bus, browser or audio endpoints.
        env = {'PATH': str(fake), 'HOME': str(base), 'LC_ALL': 'C', 'LANG': 'C',
               'TMPDIR': str(base), 'XDG_RUNTIME_DIR': str(base / 'runtime'),
               'XDG_CONFIG_HOME': str(base / 'config'), 'XDG_CACHE_HOME': str(base / 'cache'),
               'XDG_STATE_HOME': str(base / 'state'), 'DF_BENCH_BASE': str(base)}
        cases = [('connect_non_audio', 'connect', False),
                 ('disconnect', 'disconnect', False),
                 ('connect_immediate_a2dp', 'connect', True)]
        report = {'baseline': BASELINE, 'runs_each': args.runs, 'warmups_each': 2,
                  'method': 'alternating paired order; synthetic shell services; helper startup included',
                  'live_desktop': False, 'live_hardware': False,
                  'reconnect_measured': False, 'results': {}}
        for case, action, audio in cases:
            case_env = dict(env, DF_BENCH_ACTION=action, DF_BENCH_AUDIO='1' if audio else '0')
            pair = ([sys.executable, str(legacy), action, DEVICE],
                    [str(binary), '--root', str(base), 'bluetooth', action, DEVICE])
            expected = {'success': True, 'connected': action != 'disconnect'}
            if audio:
                expected.update(codec='ldac', routed=True)
            mutation = ['busctl', '--system',
                        '--timeout=15s' if action == 'disconnect' else '--timeout=40s',
                        'call', 'org.bluez', DEVICE, 'org.bluez.Device1',
                        'Disconnect' if action == 'disconnect' else 'Connect']
            expected_mutations = [mutation]
            if audio:
                expected_mutations.extend([
                    ['pactl', 'set-card-profile', 'bluez_card.fixture', 'a2dp-sink'],
                    ['pactl', 'set-default-sink', 'bluez_output.fixture']])
            for command in pair:
                for _ in range(2):
                    assert invoke(command, case_env, base, expected) == expected_mutations
            samples, rss = [[], []], [[], []]
            for index in range(args.runs):
                for side in ([0, 1] if index % 2 == 0 else [1, 0]):
                    command = pair[side]
                    if Path('/usr/bin/time').is_file():
                        command = ['/usr/bin/time', '-f', '%M', '-o', str(base / 'rss'), *command]
                    start = time.perf_counter_ns()
                    mutations = invoke(command, case_env, base, expected)
                    samples[side].append((time.perf_counter_ns() - start) / 1e6)
                    assert mutations == expected_mutations
                    if Path('/usr/bin/time').is_file():
                        rss[side].append(int((base / 'rss').read_text()))
            report['results'][case] = {
                label: {'median_ms': round(statistics.median(samples[side]), 3),
                        'median_peak_rss_kib': statistics.median(rss[side]) if rss[side] else None}
                for side, label in enumerate(['python', 'rust'])}
            report['results'][case].update(result_parity=True, mutation_argv_parity=True,
                                          expected_result=expected)
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
