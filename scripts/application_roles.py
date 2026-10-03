"""Explicit application roles and write-ahead, per-key MIME ownership."""
import argparse
import configparser
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import shlex
import subprocess

ROOT = Path(__file__).resolve().parent.parent
SECTION = 'Default Applications'


def paths():
    config = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation'
    return config / 'mimeapps.list', state / 'application-roles.json'


def roles(personal=False):
    result = json.loads((ROOT / 'config/application-roles.json').read_text())
    browser = result.pop('personalBrowser')
    if personal:
        result['browser'] = browser
    return result


def desktop_path(name):
    directories = [Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))]
    directories += [Path(p) for p in os.environ.get('XDG_DATA_DIRS', '/usr/local/share:/usr/share').split(':') if p]
    return next((p / 'applications' / name for p in directories if (p / 'applications' / name).is_file()), None)


def validate_role(name, role):
    desktop = desktop_path(role['desktop'])
    if not shutil.which(role['executable']) or not desktop:
        raise RuntimeError(f"Missing {name}: install {role['package']} ({role['desktop']})")
    entry = configparser.ConfigParser(interpolation=None)
    entry.read(desktop)
    section = entry['Desktop Entry']
    if section.get('Type') != 'Application' or section.get('Hidden', 'false') == 'true' or not section.get('Exec'):
        raise RuntimeError('Invalid application desktop entry: ' + str(desktop))
    command = shlex.split(section['Exec'])
    if not command or Path(command[0]).name != role['executable']:
        raise RuntimeError('Desktop entry executable differs from role: ' + str(desktop))
    return desktop


def values(text):
    result = {}
    section = None
    for line in text.splitlines():
        if line.startswith('[') and line.endswith(']'):
            section = line[1:-1]
        elif section == SECTION and '=' in line and not line.lstrip().startswith(('#', ';')):
            key, value = line.split('=', 1)
            if key.strip() in result:
                raise RuntimeError('Duplicate MIME default: ' + key.strip())
            result[key.strip()] = value.strip()
    return result


def replace(text, changes):
    # Preserve every unrelated line/section/comment; no whole-file restoration.
    output = []
    section = None
    remaining = dict(changes)
    found = False
    for line in text.splitlines(keepends=True):
        if line.startswith('['):
            if section == SECTION:
                output += [f'{key}={value}\n' for key, value in remaining.items() if value is not None]
                remaining.clear()
            section = line.strip()[1:-1]
            found |= section == SECTION
        if section == SECTION and '=' in line and not line.lstrip().startswith(('#', ';')):
            key = line.split('=', 1)[0].strip()
            if key in remaining:
                value = remaining.pop(key)
                if value is not None:
                    output.append(f'{key}={value}\n')
                continue
        output.append(line)
    if remaining:
        if output and not output[-1].endswith('\n'):
            output[-1] += '\n'
        if not found:
            output.append('\n[' + SECTION + ']\n')
        output += [f'{key}={value}\n' for key, value in remaining.items() if value is not None]
    return ''.join(output)


def query(mime):
    return subprocess.check_output(['xdg-mime', 'query', 'default', mime], text=True).strip()


def migrate_legacy(file, journal):
    """Adopt only defaults actually changed by the old whole-file journal."""
    from theme_pipeline import atomic
    legacy = journal.parent / 'mimeapps.json'
    if not legacy.exists():
        return
    saved = json.loads(legacy.read_text())
    if str(file) != saved['path'] or saved['managed'] is None:
        raise RuntimeError('Incomplete legacy MIME transaction; review before migration')
    original_file = journal.parent / 'mimeapps.original'
    original = values(original_file.read_text()) if saved['existed'] else {}
    managed = values(saved['managed'])
    current = values(file.read_text() if file.exists() else '')
    owned = {k: {'original': original.get(k), 'managed': v} for k, v in managed.items() if original.get(k) != v}
    for key, entry in owned.items():
        if current.get(key) != entry['managed']:
            raise RuntimeError('Legacy owned MIME default changed externally: ' + key)
    if not journal.exists():
        atomic(journal, json.dumps({'profile': 'personal', 'entries': owned, 'fileExisted': saved['existed']}, indent=2) + '\n')
    # Keep old backup as recovery evidence; remove only obsolete active journal.
    legacy.rename(journal.parent / 'mimeapps.v011-migrated.json')


def ownership_conflicts():
    file, journal = paths()
    if not journal.exists():
        return []
    saved = json.loads(journal.read_text())
    current = values(file.read_text() if file.exists() else '')
    return ['MIME default changed externally: ' + key for key, entry in saved['entries'].items()
            if current.get(key) not in {entry['managed'], entry.get('pending', entry['managed'])}]


def apply(personal=False):
    from theme_pipeline import atomic
    desired = roles(personal)
    for name, role in desired.items():
        validate_role(name, role)
    file, journal = paths()
    migrate_legacy(file, journal)
    text = file.read_text() if file.exists() else ''
    current = values(text)
    saved = json.loads(journal.read_text()) if journal.exists() else {'entries': {}, 'fileExisted': file.exists()}
    changes = {mime: role['desktop'] + ';' for role in desired.values() for mime in role['mimes']}
    for mime, entry in saved['entries'].items():
        accepted = {entry['managed'], entry.get('pending', entry['managed'])}
        if current.get(mime) not in accepted:
            raise RuntimeError('MIME default changed outside installation: ' + mime)
    for mime, value in changes.items():
        entry = saved['entries'].setdefault(mime, {'original': current.get(mime), 'managed': current.get(mime)})
        entry['pending'] = value
    saved['profile'] = 'personal' if personal else 'core'
    atomic(journal, json.dumps(saved, indent=2) + '\n')
    atomic(file, replace(text, changes))
    for mime in changes:
        entry = saved['entries'][mime]
        entry['managed'] = entry.pop('pending')
    atomic(journal, json.dumps(saved, indent=2) + '\n')
    if check(personal):
        raise RuntimeError('XDG default verification failed; inspect overriding desktop-specific mimeapps.list. Backups retained.')


def check(personal=False):
    failures = ownership_conflicts()
    for failure in failures:
        print('FAIL: ' + failure)
    for name, role in roles(personal).items():
        try:
            validate_role(name, role)
            for mime in role['mimes']:
                actual = query(mime)
                if actual != role['desktop']:
                    raise RuntimeError(f"{mime} resolves to {actual or '(unset)'}, expected {role['desktop']}")
            print('PASS application role ' + name + ': ' + role['desktop'])
        except (RuntimeError, OSError, subprocess.SubprocessError) as error:
            failures.append(str(error)); print('FAIL application role ' + name + ': ' + str(error))
    expected = roles(personal)['browser']['desktop']
    actual = subprocess.check_output(['xdg-settings', 'get', 'default-web-browser'], text=True).strip()
    if actual != expected:
        failures.append('Default browser differs: ' + actual)
    return failures


def restore():
    from theme_pipeline import atomic
    file, journal = paths()
    migrate_legacy(file, journal)
    if not journal.exists():
        return
    saved = json.loads(journal.read_text()); text = file.read_text() if file.exists() else ''; current = values(text)
    changes = {}
    for mime, entry in saved['entries'].items():
        if current.get(mime) not in {entry['managed'], entry.get('pending', entry['managed']), entry['original']}:
            raise RuntimeError('MIME default changed outside installation: ' + mime)
        changes[mime] = entry['original']
    restored = replace(text, changes)
    # A newly created empty file may be removed; unrelated later edits survive.
    if not saved['fileExisted'] and not re.sub(r'(?m)^\[Default Applications\]\s*$', '', restored).strip():
        file.unlink(missing_ok=True)
    else:
        atomic(file, restored)
    journal.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['apply', 'check', 'restore', 'launch'])
    parser.add_argument('role', nargs='?', choices=['terminal', 'browser', 'files'])
    parser.add_argument('--personal', action='store_true')
    args = parser.parse_args()
    if args.action == 'check':
        return bool(check(args.personal))
    if args.action == 'launch':
        wrapper = str(ROOT / 'scripts/foundation')
        os.execv(wrapper, [wrapper, 'apps', 'launch', args.role or 'terminal'])
    _, journal = paths(); journal.parent.mkdir(parents=True, exist_ok=True)
    with (journal.parent / 'roles.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        restore() if args.action == 'restore' else apply(args.personal)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
