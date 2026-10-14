"""Bootstrap role metadata and compatibility calls into the Rust backend."""
import argparse
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
BACKEND = ROOT / 'native/foundation/target/release/desktop-foundationctl'


def paths():
    config = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation'
    return config / 'mimeapps.list', state / 'application-roles.json'


def roles(personal=False):
    # Package planning runs before the backend is built on a fresh installation.
    result = json.loads((ROOT / 'config/application-roles.json').read_text())
    browser = result.pop('personalBrowser')
    if personal:
        result['browser'] = browser
    return result


def desktop_path(name):
    directories = [Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))]
    directories += [Path(p) for p in os.environ.get('XDG_DATA_DIRS', '/usr/local/share:/usr/share').split(':') if p]
    return next((p / 'applications' / name for p in directories if (p / 'applications' / name).is_file()), None)


def native(action, *args, value=None):
    completed = subprocess.run(
        [str(BACKEND), '--root', str(ROOT), 'app-roles', action, *args],
        input=json.dumps(value) if value is not None else None,
        text=True, capture_output=True, timeout=90,
    )
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(completed.stderr.strip() or 'Application backend returned invalid JSON') from None
    if completed.returncode and not (action == 'check' and result.get('failures')):
        raise RuntimeError(completed.stderr.strip() or 'Application backend failed')
    return result


def validate_role(name, role):
    return Path(native('validate', name, value=role))


def ownership_conflicts():
    _, journal = paths()
    if not journal.exists() and not journal.is_symlink():
        return []
    return native('conflicts')


def apply(personal=False):
    return native('apply', *(['--personal'] if personal else []))


def check(personal=False):
    result = native('check', *(['--personal'] if personal else []))
    for message in result['messages']:
        print(message)
    return result['failures']


def restore():
    return native('restore')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['apply', 'check', 'restore', 'launch'])
    parser.add_argument('role', nargs='?', choices=['terminal', 'browser', 'files', 'pdf', 'image', 'text'])
    parser.add_argument('--personal', action='store_true')
    args = parser.parse_args()
    if args.action == 'launch':
        os.execv(str(BACKEND), [str(BACKEND), '--root', str(ROOT), 'apps', 'launch', args.role or 'terminal'])
    if args.action == 'check':
        return bool(check(args.personal))
    restore() if args.action == 'restore' else apply(args.personal)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
