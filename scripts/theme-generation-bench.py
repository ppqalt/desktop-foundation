#!/usr/bin/env python3
"""Finite palette benchmark: frozen Python vs Rust, synthetic Matugen/private cache."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BASELINE = '1ca553c842d4dc56577f56dbcb0f6ac69c448457'
MATERIAL = {'primary': '#98ccf9', 'secondary': '#bdc7d5'}


def run(command, env, payload):
    result = subprocess.run(command, env=env, input=payload, capture_output=True,
                            text=True, check=True, timeout=10)
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=int, default=24)
    args = parser.parse_args()
    if not 4 <= args.runs <= 200:
        parser.error('--runs must be between 4 and 200')
    binary = ROOT / 'native/foundation/target/release/desktop-foundationctl'
    measure = ROOT / 'native/foundation/target/release/foundation-measure'
    if not binary.is_file() or not measure.is_file():
        parser.error('Build the release backend and optional --features bench --bin foundation-measure first')
    with tempfile.TemporaryDirectory(prefix='foundation-theme-bench-') as directory:
        base = Path(directory)
        checkout = base / 'checkout'
        fallback = checkout / 'theme/fallback'; fallback.mkdir(parents=True)
        shutil.copy2(ROOT / 'theme/fallback/semantic.json', fallback / 'semantic.json')
        scripts = checkout / 'scripts'; scripts.mkdir()
        legacy = scripts / 'theme_pipeline.py'
        legacy.write_bytes(subprocess.check_output(
            ['git', '-C', str(ROOT), 'show', f'{BASELINE}:scripts/theme_pipeline.py'], timeout=5))
        driver = scripts / 'legacy-driver.py'
        driver.write_text('''import json, runpy, sys
from pathlib import Path
theme = runpy.run_path(str(Path(__file__).with_name('theme_pipeline.py')))
if sys.argv[1] == 'derive':
    result = theme['derive'](json.load(sys.stdin), 'fixture', '1' * 64)
else:
    palette, cached = theme['generate'](Path(sys.argv[2]))
    result = {'palette': palette, 'cached': cached}
print(json.dumps(result))
''')
        tools = base / 'bin'; tools.mkdir()
        fake = tools / 'matugen'
        fake.write_text('#!/bin/sh\nprintf \'%s\\n\' ' + "'" +
                        json.dumps({'colors': {'dark': MATERIAL}}) + "'\n")
        fake.chmod(0o755)
        image = checkout / 'wallpaper.bin'
        image.write_bytes(b'private synthetic source\n' * 43691)
        digest = hashlib.sha256(image.read_bytes()).hexdigest()
        payload = json.dumps(MATERIAL)
        stdin = base / 'material.json'; stdin.write_text(payload)
        envs = []
        for side in ('python', 'rust'):
            home = base / side; home.mkdir()
            for name in ('config', 'cache', 'state', 'runtime'):
                (home / name).mkdir(mode=0o700)
            envs.append({'HOME': str(home), 'PATH': str(tools), 'TMPDIR': str(home),
                         'LANG': 'C', 'LC_ALL': 'C',
                         'XDG_CONFIG_HOME': str(home / 'config'),
                         'XDG_CACHE_HOME': str(home / 'cache'),
                         'XDG_STATE_HOME': str(home / 'state'),
                         'XDG_RUNTIME_DIR': str(home / 'runtime')})
        commands = {
            'derive': ([sys.executable, str(driver), 'derive'],
                       [str(binary), '--root', str(checkout), 'theme', 'derive',
                        '--source', 'fixture', '--hash', '1' * 64]),
            'generation_miss': ([sys.executable, str(driver), 'generate', str(image)],
                                [str(binary), '--root', str(checkout), 'theme', 'generate', str(image)]),
            'generation_hit': ([sys.executable, str(driver), 'generate', str(image)],
                               [str(binary), '--root', str(checkout), 'theme', 'generate', str(image)]),
        }
        caches = [Path(env['XDG_CACHE_HOME']) / 'desktop-foundation/themes' /
                  (digest + '-graphite-v1.json') for env in envs]
        report = {'baseline': BASELINE, 'live_desktop': False, 'real_matugen': False,
                  'resident_process_created': False, 'runs_each': args.runs, 'warmups_each': 2,
                  'image_bytes': image.stat().st_size,
                  'method': 'Rust wait4 measure parent; alternating paired order; process startup included',
                  'limitations': ['Warm filesystem cache; synthetic shell Matugen.',
                                  'Excludes image decoding, Matugen color extraction and runtime apply/reloads.'],
                  'results': {}}
        for case, pair in commands.items():
            parity = []
            for side, command in enumerate(pair):
                for _ in range(2):
                    if case == 'generation_miss':
                        caches[side].unlink(missing_ok=True)
                    result = run(command, envs[side], payload)
                parity.append(result)
            if parity[0] != parity[1]:
                raise AssertionError(f'{case} palette/cache parity failed: {parity}')
            if case.startswith('generation'):
                assert parity[0]['cached'] == (case == 'generation_hit')
            samples = [[], []]
            for index in range(args.runs):
                for side in ([0, 1] if index % 2 == 0 else [1, 0]):
                    if case == 'generation_miss':
                        caches[side].unlink(missing_ok=True)
                    command = [str(measure), str(stdin), *pair[side]]
                    samples[side].append(run(command, envs[side], ''))
            report['results'][case] = {
                label: {field: round(statistics.median(item[field] for item in samples[side]), 3)
                        for field in ('elapsed_ms', 'cpu_ms', 'peak_rss_kib')}
                for side, label in enumerate(('python', 'rust'))}
            report['results'][case]['result_parity'] = True
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
