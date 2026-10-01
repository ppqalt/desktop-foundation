#!/usr/bin/env python3
"""Journal global desktop preferences separately from linked configuration files."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import shutil

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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'restore'])
    args = parser.parse_args()
    STATE.mkdir(parents=True, exist_ok=True)
    path = STATE / 'preferences.json'
    entries = json.loads(path.read_text()) if path.exists() else []
    if args.action == 'restore':
        for entry in entries:
            if get(entry['key']) != entry['managed']:
                raise RuntimeError('Preference changed outside installation: ' + entry['key'][1])
        record = STATE / 'mimeapps.json'
        saved = json.loads(record.read_text()) if record.exists() else None
        if saved:
            mimefile = Path(saved['path'])
            if (mimefile.read_text() if mimefile.exists() else '') != saved['managed']:
                raise RuntimeError('MIME defaults changed outside installation')
        for entry in entries:
            set_value(entry['key'], entry['original'])
        if saved:
            if saved['existed']:
                shutil.copy2(STATE / 'mimeapps.original', mimefile)
            else:
                mimefile.unlink(missing_ok=True)
            record.unlink()
        path.unlink(missing_ok=True)
        return
    changes = [(['gsettings', 'color-scheme'], "'prefer-dark'"),
               (['gsettings', 'gtk-theme'], "'Adwaita-dark'")]
    for key, value in changes:
        entry = next((e for e in entries if e['key'] == key), None)
        if entry and get(key) != entry['managed']:
            raise RuntimeError('Preference changed outside installation: ' + key[1])
        if not entry:
            entries.append({'key': key, 'original': get(key), 'managed': value})
            temporary = path.with_suffix('.tmp')
            temporary.write_text(json.dumps(entries, indent=2) + '\n')
            temporary.replace(path)
        set_value(key, value)
    mimefile = Path(os.environ.get('XDG_CONFIG_HOME', str(Path.home() / '.config'))) / 'mimeapps.list'
    backup = STATE / 'mimeapps.original'
    record = STATE / 'mimeapps.json'
    if not record.exists():
        if mimefile.exists():
            shutil.copy2(mimefile, backup)
        record.write_text(json.dumps({'path': str(mimefile), 'existed': mimefile.exists(), 'managed': None}))
    saved = json.loads(record.read_text())
    if saved['managed'] is not None and (mimefile.read_text() if mimefile.exists() else '') != saved['managed']:
        raise RuntimeError('MIME defaults changed outside installation')
    # Respect an existing default; new hosts receive available native applications.
    for mime, desktop in [('inode/directory', 'org.gnome.Nautilus.desktop'),
                          ('x-scheme-handler/http', 'firefox.desktop'),
                          ('x-scheme-handler/https', 'firefox.desktop')]:
        current = get(['mime', mime])
        directories = [Path(os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share'))) / 'applications',
                       Path('/usr/local/share/applications'), Path('/usr/share/applications')]
        if not current or not any((directory / current).is_file() for directory in directories):
            subprocess.run(['xdg-mime', 'default', desktop, mime], check=True)
    saved['managed'] = mimefile.read_text() if mimefile.exists() else ''
    record.write_text(json.dumps(saved))


if __name__ == '__main__':
    main()
