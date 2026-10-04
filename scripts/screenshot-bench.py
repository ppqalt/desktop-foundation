#!/usr/bin/env python3
"""Compare screenshot helper overhead using private fixtures, never the desktop."""
import argparse
import json
import os
from pathlib import Path
import random
import statistics
import struct
import subprocess
import sys
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]
BASELINE = '1394382fc87ad737f62973675a61c99a64aeabd8'


def png():
    def chunk(kind, payload):
        return (struct.pack('>I', len(payload)) + kind + payload
                + struct.pack('>I', zlib.crc32(kind + payload)))
    width, height = 256, 256
    pixels = random.Random(0).randbytes(width * height * 3)
    rows = b''.join(b'\0' + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=int, default=24)
    args = parser.parse_args()
    if not 4 <= args.runs <= 200:
        parser.error('--runs must be between 4 and 200')
    binary = ROOT / 'native/foundation/target/release/desktop-foundationctl'
    if not binary.is_file():
        parser.error('Build the release backend first with scripts/build-backend')
    with tempfile.TemporaryDirectory(prefix='foundation-screenshot-bench-') as directory:
        base = Path(directory)
        old = base / 'baseline'
        files = ['scripts/screenshot.py', 'scripts/screenshot_backends/niri.py',
                 'scripts/screenshot_backends/hyprland.py', 'compositor/niri/screenshots.toml',
                 'config/screenshots.toml']
        for name in files:
            target = old / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(subprocess.check_output(['git', '-C', str(ROOT), 'show', f'{BASELINE}:{name}']))
        fake = base / 'bin'
        fake.mkdir()
        runtime = base / 'runtime'
        runtime.mkdir(mode=0o700)
        payload = png()
        (base / 'fixture.png').write_bytes(payload)
        fixtures = {
            'slurp': '#!/bin/sh\n[ "$#" -gt 0 ] || exit 99\nprintf "0,0 256x256\\n"\n',
            'grim': '#!/bin/sh\nfor argument do destination=$argument; done\nif [ "$destination" = - ]; then /bin/cat "$DF_FIXTURE_PNG"; else /bin/cat "$DF_FIXTURE_PNG" > "$destination"; fi\n',
            'wl-copy': '#!/bin/sh\n[ "$*" = "--type image/png" ] || exit 99\n/bin/cat > "$DF_FIXTURE_CLIPBOARD"\n',
            'hyprctl': '#!/bin/sh\n[ "$*" = "-j monitors" ] || exit 99\nprintf \'%s\\n\' \'[{"id":0,"name":"FIXTURE-1","focused":true,"disabled":false}]\'\n',
        }
        for name, source in fixtures.items():
            target = fake / name
            target.write_text(source)
            target.chmod(0o755)
        env = dict(os.environ, PATH=str(fake), HOME=str(base), XDG_RUNTIME_DIR=str(runtime),
                   HYPRLAND_INSTANCE_SIGNATURE='isolated-fixture', DF_FIXTURE_PNG=str(base / 'fixture.png'),
                   DF_FIXTURE_CLIPBOARD=str(base / 'clipboard.png'))
        for name in ['WAYLAND_DISPLAY', 'NIRI_SOCKET', 'DBUS_SESSION_BUS_ADDRESS', 'DISPLAY']:
            env.pop(name, None)
        # Both implementations read the same config. HOME and PATH ensure all
        # capture files/commands stay inside this temporary fixture.
        current = base / 'current'
        for name in ['config/screenshots.toml', 'compositor/niri/screenshots.toml']:
            target = current / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((old / name).read_bytes())
        report = {'baseline': BASELINE, 'runs_each': args.runs, 'warmups_each': 2,
                  'method': 'alternating paired order; synthetic commands and PNG; startup included',
                  'live_desktop': False, 'png_bytes': len(payload), 'results': {}}
        for case, backend, action in [('niri_region', 'niri', 'region'),
                                      ('hyprland_output', 'hyprland', 'output')]:
            pair = ([sys.executable, str(old / 'scripts/screenshot.py'), '--backend', backend, action],
                    [str(binary), '--root', str(current), 'screenshot', '--backend', backend, action])
            def invoke(command):
                result = subprocess.run(command, env=env, check=True, capture_output=True, timeout=10)
                assert (base / 'clipboard.png').read_bytes() == payload
                if backend == 'hyprland':
                    saved = Path(result.stdout.decode().strip())
                    assert saved.read_bytes() == payload
                    assert saved.stat().st_mode & 0o777 == 0o600
                    saved.unlink()
                else:
                    assert not result.stdout
            for command in pair:
                for _ in range(2):
                    invoke(command)
            samples = [[], []]
            rss = [[], []]
            for index in range(args.runs):
                for side in ([0, 1] if index % 2 == 0 else [1, 0]):
                    command = pair[side]
                    if Path('/usr/bin/time').is_file():
                        command = ['/usr/bin/time', '-f', '%M', '-o', str(base / 'rss'), *command]
                    start = time.perf_counter_ns()
                    invoke(command)
                    samples[side].append((time.perf_counter_ns() - start) / 1e6)
                    if Path('/usr/bin/time').is_file():
                        rss[side].append(int((base / 'rss').read_text()))
            report['results'][case] = {
                label: {'median_ms': round(statistics.median(samples[side]), 3),
                        'median_peak_rss_kib': statistics.median(rss[side]) if rss[side] else None}
                for side, label in enumerate(['python', 'rust'])}
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
