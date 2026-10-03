#!/usr/bin/env python3
"""Journal global desktop preferences separately from linked configuration files."""
import argparse
import json
import os
from pathlib import Path
import subprocess

STATE = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state'))) / 'desktop-foundation'


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def get(key):
    return run('gsettings', 'get', 'org.gnome.desktop.interface', key[1]) if key[0] == 'gsettings' else run('xdg-mime', 'query', 'default', key[1])


def set_value(key, value):
    if key[0] == 'gsettings':
        subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', key[1], value], check=True)
    elif value:
        subprocess.run(['xdg-mime', 'default', value, key[1]], check=True)
    else:
        # MIME settings are restored by the deploy journal's file backup instead.
        pass


def manage_setting(key, value):
    """Per-setting original ownership; journal before the supported native write."""
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'restore'])
    parser.add_argument('--theme-only', action='store_true', help='Apply reversible dark preferences without changing MIME defaults')
    parser.add_argument('--browser', default=None, help='Explicit verified desktop entry for personal installation')
    args = parser.parse_args()
    STATE.mkdir(parents=True, exist_ok=True)
    path = STATE / 'preferences.json'
    entries = json.loads(path.read_text()) if path.exists() else []
    if args.action == 'restore':
        for entry in entries:
            if get(entry['key']) not in {entry['managed'], entry.get('pending', entry['managed'])}:
                raise RuntimeError('Preference changed outside installation: ' + entry['key'][1])
        from application_roles import restore as restore_roles
        restore_roles()
        for entry in entries:
            set_value(entry['key'], entry['original'])
        path.unlink(missing_ok=True)
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
