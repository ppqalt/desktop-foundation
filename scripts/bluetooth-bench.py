#!/usr/bin/env python3
"""Finite old/new Bluetooth helper benchmark using only synthetic executables."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BASELINE = 'v0.12-1'
ADDRESS = 'AB:CD:EF:01:02:03'
DEVICE = '/org/bluez/hci7/dev_' + ADDRESS.replace(':', '_')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=int, default=24)
    args = parser.parse_args()
    if not 4 <= args.runs <= 200:
        parser.error('--runs must be between 4 and 200')
    binary = ROOT / 'native/foundation/target/release/desktop-foundationctl'
    if not binary.is_file():
        parser.error('Build the release backend first with scripts/build-backend')
    with tempfile.TemporaryDirectory(prefix='foundation-bluetooth-bench-') as directory:
        base = Path(directory)
        for name in ['bluetooth-power.py', 'bluetooth-action.py']:
            source = subprocess.check_output(['git', '-C', str(ROOT), 'show', f'{BASELINE}:scripts/{name}'])
            (base / name).write_bytes(source)
        fake = base / 'bin'
        fake.mkdir()
        busctl = fake / 'busctl'
        busctl.write_text('''#!/bin/sh
case "$*" in
  *tree*) printf '%s\\n' /org/bluez/hci2 /org/bluez/hci7 ;;
  *set-property*) : ;;
  *Powered*) printf '%s\\n' '{"type":"b","data":false}' ;;
  *Paired*) printf '%s\\n' '{"type":"b","data":true}' ;;
  *Address*) printf '%s\\n' '{"type":"s","data":"AB:CD:EF:01:02:03"}' ;;
  *) exit 99 ;;
esac
''')
        cards = [{'properties': {'api.bluez5.address': ADDRESS}, 'profiles': {
            'a2dp-sink': {'description': 'codec LDAC', 'available': True},
            'a2dp-sink-aac': {'available': True},
            'a2dp-sink-sbc': {'available': True},
        }}]
        pactl = fake / 'pactl'
        pactl.write_text('#!/bin/sh\n[ "$*" = "--format=json list cards" ] || exit 99\nprintf \'%s\\n\' \' ' + json.dumps(cards) + '\'\n')
        for executable in [busctl, pactl]:
            executable.chmod(0o755)
        # No real Bluetooth/audio executable is reachable, even on fixture error.
        env = dict(os.environ, PATH=str(fake))
        cases = {
            'power_off_two_adapters': ([sys.executable, str(base / 'bluetooth-power.py'), 'off'],
                                       [str(binary), '--root', str(ROOT), 'bluetooth', 'power', 'off']),
            'codec_discovery': ([sys.executable, str(base / 'bluetooth-action.py'), 'codecs', DEVICE],
                                [str(binary), '--root', str(ROOT), 'bluetooth', 'codecs', DEVICE]),
        }
        report = {'baseline': BASELINE, 'runs_each': args.runs, 'warmups_each': 2,
                  'method': 'alternating paired order; synthetic shell commands; process startup included',
                  'live_hardware': False, 'results': {}}
        for name, pair in cases.items():
            expected = None
            for command in pair:
                for _ in range(2):
                    result = json.loads(subprocess.check_output(command, env=env, timeout=15))
                    if expected is None:
                        expected = result
                    assert result == expected
            samples = [[], []]
            rss = [[], []]
            for index in range(args.runs):
                for side in ([0, 1] if index % 2 == 0 else [1, 0]):
                    command = pair[side]
                    if Path('/usr/bin/time').is_file():
                        command = ['/usr/bin/time', '-f', '%M', '-o', str(base / 'rss'), *command]
                    start = time.perf_counter_ns()
                    result = subprocess.check_output(command, env=env, timeout=15)
                    samples[side].append((time.perf_counter_ns() - start) / 1e6)
                    assert json.loads(result) == expected
                    if Path('/usr/bin/time').is_file():
                        rss[side].append(int((base / 'rss').read_text()))
            report['results'][name] = {
                label: {'median_ms': round(statistics.median(samples[side]), 3),
                        'median_peak_rss_kib': statistics.median(rss[side]) if rss[side] else None}
                for side, label in enumerate(['python', 'rust'])
            }
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
