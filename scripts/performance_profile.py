#!/usr/bin/env python3
"""Standalone Lucky38 binfmt transaction. No bootloader, kernel or GPU edits."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile

STATE = Path('/var/lib/desktop-foundation/performance-lucky38')
WINE = Path('/etc/binfmt.d/wine.conf')
CONDITION = Path('/etc/systemd/system/systemd-binfmt.service.d/40-desktop-foundation-performance.conf')
VENDOR = Path('/usr/lib/binfmt.d/wine.conf')
BINFMT = '/usr/lib/systemd/systemd-binfmt'
# Failure to read effective configuration must run the normal service, not skip
# unrelated handlers. A per-file mask leaves explicit wine/Proton invocations intact.
CONDITION_TEXT = '''[Service]
ExecCondition=/bin/sh -c 'config=$$(/usr/lib/systemd/systemd-binfmt --cat-config --no-pager) || exit 0; printf "%%s\\n" "$$config" | /usr/bin/grep -q "^[^\u0023[:space:]]"; result=$$?; [ "$$result" -ne 1 ]'
'''


def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT, timeout=30)


def hardware():
    board = Path('/sys/class/dmi/id/board_name').read_text().strip()
    cpu = Path('/proc/cpuinfo').read_text()
    gpu = run('/usr/bin/nvidia-smi', '--query-gpu=name', '--format=csv,noheader').strip()
    if board != 'TUF B450-PRO GAMING' or 'AMD Ryzen 7 5700X3D' not in cpu or gpu != 'NVIDIA GeForce RTX 3060':
        raise ValueError('Lucky38 performance profile does not match this board/CPU/GPU')
    return {'board': board, 'cpu': 'Ryzen 7 5700X3D', 'gpu': gpu, 'root': run('/usr/bin/findmnt', '-n', '-o', 'UUID', '/').strip()}


def identity(path):
    if path.is_symlink():
        return {'symlink': os.readlink(path)}
    if path.exists():
        info = path.stat()
        if not stat.S_ISREG(info.st_mode):
            raise ValueError('Unexpected file type: ' + str(path))
        return {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'mode': stat.S_IMODE(info.st_mode)}
    return None


def safe_parent(path):
    for parent in reversed(path.parent.parents):
        if parent == Path('/'):
            continue
        if parent.exists():
            m = parent.lstat()
            if not stat.S_ISDIR(m.st_mode) or m.st_uid != 0 or m.st_mode & 0o022:
                raise ValueError('Unprotected parent: ' + str(parent))
    path.parent.mkdir(parents=True, exist_ok=True)
    m = path.parent.lstat()
    if not stat.S_ISDIR(m.st_mode) or m.st_uid != 0 or m.st_mode & 0o022:
        raise ValueError('Unprotected directory: ' + str(path.parent))


def atomic(path, content):
    safe_parent(path)
    fd, name = tempfile.mkstemp(dir=path.parent, prefix='.df-performance-')
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(content)
            os.fchmod(stream.fileno(), 0o600 if path.parent == STATE else 0o644)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, path)
        fd = os.open(path.parent, os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def binfmt_changes():
    lines = [s.strip() for s in VENDOR.read_text().splitlines() if s.strip() and not s.lstrip().startswith('#')]
    if lines != [':DOSWin:M::MZ::/usr/bin/wine:']:
        raise ValueError('Wine vendor registration changed; review before applying')
    return {WINE: None, CONDITION: CONDITION_TEXT}


def save(journal):
    atomic(STATE / 'journal.json', (json.dumps(journal, indent=2) + '\n').encode())


def check_files(journal, wanted):
    for path, record in journal['files'].items():
        current = identity(Path(path))
        if current != record[wanted] and not (journal['phase'] in ('prepared', 'restoring') and current is None and record['original'] is None):
            raise ValueError('Externally modified performance file: ' + path)


def transaction(action):
    machine = hardware()
    if action == 'check':
        for path, content in binfmt_changes().items():
            wanted = {'symlink': '/dev/null'} if content is None else {'sha256': hashlib.sha256(content.encode()).hexdigest(), 'mode': 0o644}
            if identity(path) != wanted or path.lstat().st_uid != 0:
                raise ValueError('Performance policy is absent or externally modified')
        print('Lucky38 performance policy is effective; no privileged changes needed')
        return
    if action == 'plan':
        print(json.dumps({'profile': 'lucky38', 'hardware': machine, 'files': {str(p): v for p, v in binfmt_changes().items()}, 'tradeoff': 'Direct exec of .exe files uses explicit wine instead; Proton and Wine launchers remain available.'}, indent=2))
        return
    safe_parent(STATE / 'lock')
    STATE.mkdir(exist_ok=True, mode=0o700)
    lock = os.open(STATE / 'lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        journal_path = STATE / 'journal.json'
        if journal_path.is_symlink():
            raise ValueError('Unexpected journal symlink')
        journal = json.loads(journal_path.read_text()) if journal_path.exists() else None
        if journal and (journal['version'] != 1 or journal['hardware'] != machine or set(journal['files']) != {str(WINE), str(CONDITION)}):
            raise ValueError('Saved performance transaction identity changed')
        if action == 'status':
            if journal and journal['phase'] != 'restored':
                check_files(journal, 'managed')
            print(json.dumps(journal, indent=2))
            return
        if action == 'restore':
            if not journal or journal['phase'] == 'restored':
                print('Nothing to restore')
                return
            # All original paths were absent. Never replace administrator files.
            check_files(journal, 'managed')
            journal['phase'] = 'restoring'
            save(journal)
            for path, record in journal['files'].items():
                if record['original'] is not None:
                    raise ValueError('Unsupported original file; journal preserved')
                Path(path).unlink(missing_ok=True)
                record['managed'] = None
                save(journal)
            run('/usr/bin/systemctl', 'daemon-reload')
            # No binfmt handler was unregistered at apply; live Wine stays intact.
            journal['phase'] = 'restored'
            save(journal)
            print('Lucky38 performance files restored; no session or boot restart')
            return
        if journal and journal['phase'] == 'installed':
            check_files(journal, 'managed')
            binfmt_changes()  # Revalidate vendor contract after package updates.
            print('Lucky38 performance profile already installed; rollback retained')
            return
        if journal and journal['phase'] != 'restored':
            raise ValueError('Interrupted transaction: restore before retrying')
        changes = binfmt_changes()
        for path in changes:
            safe_parent(path)
            if identity(path) is not None:
                raise ValueError('Existing administrator file retained: ' + str(path))
        journal = {'version': 1, 'phase': 'prepared', 'hardware': machine, 'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'files': {str(p): {'original': None, 'managed': None} for p in changes}}
        save(journal)
        for path, content in changes.items():
            # Write intent first, so interrupted writes remain recoverable.
            expected = {'symlink': '/dev/null'} if content is None else {'sha256': hashlib.sha256(content.encode()).hexdigest(), 'mode': 0o644}
            journal['files'][str(path)]['managed'] = expected
            save(journal)
            if content is None:
                path.symlink_to('/dev/null')
            else:
                atomic(path, content.encode())
        run('/usr/bin/systemctl', 'daemon-reload')
        run('/usr/bin/systemd-analyze', 'verify', 'systemd-binfmt.service')
        journal['phase'] = 'installed'
        save(journal)
        print('Lucky38 binfmt optimization installed for next boot; current Wine handler unchanged')
    finally:
        os.close(lock)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['plan', 'check', 'apply', 'status', 'restore'])
    args = parser.parse_args()
    if os.geteuid() != 0 and args.action not in ('plan', 'check'):
        parser.error('Administrator authentication required')
    transaction(args.action)


if __name__ == '__main__':
    main()
