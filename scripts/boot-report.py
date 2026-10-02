#!/usr/bin/env python3
"""Read-only boot evidence, with separate kernel-source and journal timelines."""
import argparse
import json
from pathlib import Path
import re
import statistics
import subprocess
from startup_report import collect


def rows(boot):
    output = subprocess.check_output(['journalctl', '-b', str(boot), '-o', 'json', '--no-pager'], text=True)
    return [json.loads(line) for line in output.splitlines()]


def seconds(value):
    return sum(float(n) * {'min': 60, 's': 1, 'ms': .001, 'us': .000001}[unit]
               for n, unit in re.findall(r'([\d.]+)(min|ms|us|s)', value))


def collect_boot(boot):
    records = rows(boot)
    events, kernel, totals, durations = {}, {}, {}, {}
    for row in records:
        message = row.get('MESSAGE', '')
        if not isinstance(message, str):
            continue
        stamp = int(row['__MONOTONIC_TIMESTAMP']) / 1e6
        for name, needle in [('root_ready', 'Reached target Initrd Root File System.'),
                             ('initrd_default', 'Reached target Initrd Default Target.'),
                             ('udev_cleanup', 'Starting Cleanup udev Database...'),
                             ('switch_root', 'Switching root.'),
                             ('greetd_started', 'Started Greeter daemon.'),
                             ('binfmt_start', 'Starting Set Up Additional Binary Formats...'),
                             ('binfmt_end', 'Finished Set Up Additional Binary Formats.')]:
            if needle in message:
                events.setdefault(name, stamp)
        if 'Startup finished in' in message and '(firmware)' in message:
            for value, stage in re.findall(r'([\d.a-z ]+) \((firmware|loader|kernel|initrd|userspace)\)', message):
                totals[stage] = seconds(value)
            totals['total'] = seconds(message.split(' = ', 1)[1].rstrip('.'))
        if row.get('_TRANSPORT') == 'kernel':
            # Early kernel messages are buffered; journal receipt timestamps
            # cannot be used to time decompression or driver initialization.
            source = int(row.get('_SOURCE_MONOTONIC_TIMESTAMP', row['__MONOTONIC_TIMESTAMP'])) / 1e6
            for name, needle in [('unpack_start', 'Trying to unpack rootfs image'),
                                 ('unpack_end', 'Freeing initrd memory:'),
                                 ('init_process', 'Run /init as init process'),
                                 ('amdgpu_init', 'initializing kernel modesetting'),
                                 ('gpu_stb', 'STB initialized'),
                                 ('gpu_ready', 'fb0: amdgpudrmfb frame buffer device')]:
                if needle in message:
                    kernel.setdefault(name, source)
    for name, data, start, end in [('initramfs_unpack_interval', kernel, 'unpack_start', 'unpack_end'),
                                  ('amdgpu_initialization', kernel, 'amdgpu_init', 'gpu_ready'),
                                  ('root_ready_to_switch_root', events, 'root_ready', 'switch_root'),
                                  ('binfmt', events, 'binfmt_start', 'binfmt_end')]:
        if start in data and end in data:
            durations[name] = round(data[end] - data[start], 6)
    return {'boot_id': next((r['_BOOT_ID'] for r in records if '_BOOT_ID' in r), None),
            'stages_seconds': totals, 'journal_events_seconds': events,
            'kernel_source_events_seconds': kernel, 'intervals_seconds': durations}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--boots', type=int, default=4)
    parser.add_argument('--save', help='Label for snapshot outside the checkout')
    args = parser.parse_args()
    if not 1 <= args.boots <= 20:
        parser.error('Choose between one and twenty boots')
    boots = [collect_boot(-i) for i in range(args.boots)]
    medians = {key: statistics.median([b['stages_seconds'][key] for b in boots if key in b['stages_seconds']])
               for key in boots[0]['stages_seconds']}
    report = {'boots': boots, 'stage_medians_seconds': medians, 'current_session': collect(),
              'limitations': ['Bootloader time includes time spent choosing entries.',
                              'greetd_started is a service timestamp, not a measured first visible frame.',
                              'Kernel source timestamps and journal receipt timestamps are separate timelines.',
                              'Initramfs unpack interval includes other overlapping kernel work.',
                              'Saved labels do not by themselves prove a change was deployed.']}
    data = json.dumps(report, indent=2) + '\n'
    if args.save:
        if not re.fullmatch(r'[a-zA-Z0-9_-]+', args.save):
            parser.error('Invalid snapshot label')
        folder = Path.home() / '.cache/desktop-foundation/boot-audit/optimization'
        folder.mkdir(parents=True, exist_ok=True)
        target = folder / (args.save + '-' + boots[0]['boot_id'] + '.json')
        target.write_text(data)
        target.chmod(0o600)
        print('Saved ' + str(target))
    print(data, end='')


if __name__ == '__main__':
    main()
