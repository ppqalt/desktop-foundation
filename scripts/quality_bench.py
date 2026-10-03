#!/usr/bin/env python3
"""Finite, interleaved synthetic comparisons; never reads the live clipboard.

Build native/foundation's release binary first. The baseline is read through
git show, without checking it out or changing the running desktop. Measurements
include process startup and private filesystem writes on the checkout's disk.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def summarize(samples):
    return {key: statistics.median(s[key] for s in samples) for key in samples[0]}


def run(command, payload, environment, directory):
    driver = ROOT / 'native/foundation/target/release/foundation-measure'
    with tempfile.NamedTemporaryFile(dir=directory) as source:
        source.write(payload); source.flush()
        result = subprocess.run([str(driver), source.name, *command], env=environment,
                                capture_output=True, text=True, timeout=15)
        if result.returncode: raise RuntimeError(result.stderr)
        return json.loads(result.stdout)


def comparable(state):
    entries = json.loads((state / 'index.json').read_text())
    for entry in entries:
        entry.pop('updated')
        if entry['image']: entry['image'] = entry['image'].rsplit('/', 1)[-1]
    return entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline-ref', required=True)
    parser.add_argument('--samples', type=int, default=24)
    parser.add_argument('--image-samples', type=int, default=8)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not (2 <= args.samples <= 100 and 2 <= args.image_samples <= 100):
        parser.error('Use 2–100 finite samples')
    binary = ROOT / 'native/foundation/target/release/desktop-foundationctl'
    if not binary.is_file(): parser.error('Run scripts/build-backend first')
    subprocess.run(['cargo', '+1.98.1', 'build', '--locked', '--release', '--features', 'bench',
                    '--bin', 'foundation-measure', '--manifest-path', str(ROOT / 'native/foundation/Cargo.toml')], check=True)
    work = ROOT / 'work'; work.mkdir(exist_ok=True)
    reference = subprocess.check_output(['git', 'rev-parse', '--verify', args.baseline_ref + '^{commit}'], cwd=ROOT, text=True).strip()
    report = {'baseline_ref': reference, 'candidate_ref': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'method': 'Interleaved old/new order, 2 warmups, fresh private DBs on checkout filesystem, small Rust wait4 parent per child, synthetic data; median, no resident-shell claim',
              'benchmarks': {}}
    with tempfile.TemporaryDirectory(prefix='quality-bench-', dir=work) as temporary:
        base = Path(temporary)
        old = base / 'clipboard.py'
        old.write_bytes(subprocess.check_output(['git', 'show', reference + ':scripts/clipboard.py'], cwd=ROOT))
        cases = [('text', ('ä ö å €\0🙂\nsynthetic clipboard\n' * 256).encode(), args.samples),
                 ('image', b'\x89PNG\r\n\x1a\n' + bytes(1024 * 1024 - 8), args.image_samples)]
        env = dict(os.environ, CLIPBOARD_STATE='data')
        for kind, payload, count in cases:
            values = {'before': [], 'after': []}
            expected = None
            for iteration in range(count + 2):
                order = ['before', 'after'] if iteration % 2 == 0 else ['after', 'before']
                for implementation in order:
                    state = base / f'{kind}-{iteration}-{implementation}'
                    command = ['python3', str(old)] if implementation == 'before' else [str(binary), '--root', str(ROOT), 'clipboard']
                    command += ['--state', str(state), 'store', kind]
                    result = run(command, payload, env, base)
                    entries = comparable(state)
                    if expected is None: expected = entries
                    if entries != expected: raise RuntimeError('Clipboard projection compatibility failed')
                    if iteration >= 2: values[implementation].append(result)
            report['benchmarks'][kind] = {'payload_bytes': len(payload), 'samples_per_implementation': count,
                                         'before': summarize(values['before']), 'after': summarize(values['after']),
                                         'samples': values, 'projection_equal': True}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: {k: v for k, v in value.items() if k != 'samples'} for key, value in report['benchmarks'].items()}, indent=2))


if __name__ == '__main__':
    main()
