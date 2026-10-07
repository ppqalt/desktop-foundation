#!/usr/bin/env python3
"""Journal global desktop preferences separately from linked configuration files."""
import argparse
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
STATE = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state'))) / 'desktop-foundation'


def native(*arguments, lock=None):
    """Finite backend call; inherited recovery lock spans MIME restoration."""
    command = [str(ROOT / 'scripts/foundation'), 'preferences', '--state', str(STATE), *arguments]
    descriptors = ()
    if lock is not None:
        descriptors = (lock.fileno(),)
        command += ['--lock-fd', str(lock.fileno())]
    try:
        result = subprocess.run(command, capture_output=True, text=True, check=True,
                                timeout=75, pass_fds=descriptors)
    except subprocess.CalledProcessError as error:
        raise RuntimeError(error.stderr.strip() or 'Native preference operation failed') from error
    except subprocess.TimeoutExpired as error:
        raise RuntimeError('Native preference operation timed out; recovery journal retained') from error
    value = json.loads(result.stdout)
    if not isinstance(value, dict):
        raise RuntimeError('Native preference backend returned an invalid response')
    return value


def manage_setting(key, value):
    native('set', *key, value)


@contextmanager
def preference_lock():
    STATE.mkdir(parents=True, exist_ok=True)
    fd = os.open(STATE / 'preferences.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield lock


def restore_preferences():
    from application_roles import restore as restore_roles
    with preference_lock() as lock:
        # Check every owned preference before changing the separate MIME journal.
        native('prepare-restore', lock=lock)
        restore_roles()
        native('restore', lock=lock)


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
