#!/usr/bin/env python3
"""Journal global desktop preferences separately from linked configuration files."""
import argparse
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import subprocess

STATE = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state'))) / 'desktop-foundation'


def run(*args):
    return subprocess.check_output(args, text=True, timeout=10).strip()


def get(key):
    return run('gsettings', 'get', 'org.gnome.desktop.interface', key[1]) if key[0] == 'gsettings' else run('xdg-mime', 'query', 'default', key[1])


def set_value(key, value):
    if key[0] == 'gsettings':
        subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', key[1], value], check=True, timeout=10)
    elif value:
        subprocess.run(['xdg-mime', 'default', value, key[1]], check=True, timeout=10)
    else:
        # MIME settings are restored by the deploy journal's file backup instead.
        pass


def manage_setting(key, value):
    """Per-setting original ownership; journal before the supported native write."""
    with preference_lock():
        _manage_setting(key, value)


@contextmanager
def preference_lock():
    STATE.mkdir(parents=True, exist_ok=True)
    fd = os.open(STATE / 'preferences.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def _manage_setting(key, value):
    from theme_pipeline import atomic
    path = STATE / 'preferences.json'
    entries = json.loads(path.read_text()) if path.exists() else []
    current = get(key)
    entry = next((e for e in entries if e['key'] == key), None)
    if entry and current not in {entry['managed'], entry.get('pending', entry['managed'])}:
        raise RuntimeError('Preference changed outside installation: ' + key[1])
    if not entry:
        entry = {'key': key, 'original': current, 'managed': current}
        entries.append(entry)
    entry['pending'] = value
    atomic(path, json.dumps(entries, indent=2) + '\n')
    if current != value:
        set_value(key, value)
    entry['managed'] = entry.pop('pending')
    atomic(path, json.dumps(entries, indent=2) + '\n')


def restore_preferences():
    from theme_pipeline import atomic
    from application_roles import restore as restore_roles
    with preference_lock():
        path = STATE / 'preferences.json'
        entries = json.loads(path.read_text()) if path.exists() else []
        for entry in entries:
            if get(entry['key']) not in {entry['managed'], entry.get('pending', entry['managed'])}:
                raise RuntimeError('Preference changed outside installation: ' + entry['key'][1])
        restore_roles()
        while entries:
            entry = entries[0]
            current = get(entry['key'])
            if current not in {entry['managed'], entry.get('pending', entry['managed'])}:
                raise RuntimeError('Preference changed outside installation: ' + entry['key'][1])
            # Write-ahead restoration makes a crash after the native write
            # resumable: the original value is then an accepted pending value.
            entry['pending'] = entry['original']
            atomic(path, json.dumps(entries, indent=2) + '\n')
            if current != entry['original']:
                set_value(entry['key'], entry['original'])
            entries.pop(0)
            atomic(path, json.dumps(entries, indent=2) + '\n')
        path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'restore'])
    parser.add_argument('--theme-only', action='store_true', help='Apply reversible dark preferences without changing MIME defaults')
    parser.add_argument('--browser', default=None, help='Explicit verified desktop entry for personal installation')
    args = parser.parse_args()
    STATE.mkdir(parents=True, exist_ok=True)
    if args.action == 'restore':
        restore_preferences()
        return
    changes = [(['gsettings', 'color-scheme'], "'prefer-dark'"),
               (['gsettings', 'gtk-theme'], "'adw-gtk3-dark'")]
    for key, value in changes:
        manage_setting(key, value)
    if args.theme_only:
        return
    from application_roles import apply as apply_roles, roles
    if args.browser and args.browser not in {roles(False)['browser']['desktop'], roles(True)['browser']['desktop']}:
        raise RuntimeError('Unknown browser role; configure config/application-roles.json')
    apply_roles(personal=args.browser == roles(True)['browser']['desktop'])



if __name__ == '__main__':
    main()
