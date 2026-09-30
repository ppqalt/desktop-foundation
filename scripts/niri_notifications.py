"""Start one minimal native toast provider for the Niri session only."""
import os
from pathlib import Path
import shutil
import subprocess

from niri_wallpaper import quote

ROOT = Path(__file__).resolve().parent.parent
UNIT = 'desktop-foundation-notifications.service'


def main():
    if not os.environ.get('NIRI_SOCKET'):
        raise RuntimeError('Notification startup requires a Niri session')
    active = subprocess.run(['systemctl', '--user', 'is-active', '--quiet', UNIT]).returncode == 0
    owner = subprocess.check_output(['busctl', '--user', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', 's', 'org.freedesktop.Notifications'], text=True).strip()
    if owner == 'b true' and not active:
        print('Existing notification provider retained; no duplicate started.')
        return
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation'
    executable = shutil.which('mako') or str(state / 'bin/mako')
    if not Path(executable).is_file():
        raise RuntimeError('mako unavailable: install it with scripts/bootstrap')
    unit = Path(os.environ['XDG_RUNTIME_DIR']) / 'systemd/user' / UNIT
    unit.parent.mkdir(parents=True, exist_ok=True)
    config = ROOT / 'compositor/niri/notifications.conf'
    content = ('[Unit]\nDescription=Desktop foundation Niri notification toasts\n'
               'PartOf=niri.service graphical-session.target\nConditionEnvironment=NIRI_SOCKET\n'
               'StartLimitIntervalSec=60\nStartLimitBurst=3\n'
               '[Service]\nType=dbus\nBusName=org.freedesktop.Notifications\n'
               f'ExecStart={quote(executable)} --config {quote(config)}\n'
               'Restart=on-failure\nRestartSec=1\n')
    changed = not unit.exists() or unit.read_text() != content
    if changed:
        temporary = unit.with_suffix('.tmp')
        temporary.write_text(content)
        temporary.replace(unit)
        subprocess.run(['systemctl', '--user', 'daemon-reload'], check=True)
    subprocess.run(['systemctl', '--user', 'restart' if changed else 'start', UNIT], check=True)


if __name__ == '__main__':
    main()
