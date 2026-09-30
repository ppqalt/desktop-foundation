"""One Niri-session-owned swaybg; no resident custom worker."""
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def quote(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def main():
    if not os.environ.get('NIRI_SOCKET'):
        raise RuntimeError('Wallpaper startup requires a Niri session')
    config = tomllib.loads((ROOT / 'compositor/niri/wallpaper.toml').read_text())
    image = Path(config['image']).expanduser()
    if not image.is_file():
        raise RuntimeError(f'Wallpaper image unavailable: {image}')
    mode = config.get('mode', 'fill')
    if mode not in {'fill', 'fit', 'center', 'tile'}:
        raise ValueError('Choose an aspect-preserving wallpaper mode: fill, fit, center or tile')
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation'
    executable = shutil.which('swaybg') or str(state / 'bin/swaybg')
    if not Path(executable).is_file():
        raise RuntimeError('swaybg unavailable: install it with scripts/bootstrap')
    unit = Path(os.environ['XDG_RUNTIME_DIR']) / 'systemd/user/desktop-foundation-wallpaper.service'
    unit.parent.mkdir(parents=True, exist_ok=True)
    content = ('[Unit]\nDescription=Desktop foundation Niri wallpaper\n'
               'PartOf=niri.service graphical-session.target\nConditionEnvironment=NIRI_SOCKET\n'
               'StartLimitIntervalSec=60\nStartLimitBurst=3\n'
               '[Service]\nType=simple\n'
               f'ExecStart={quote(executable)} --image {quote(image)} --mode {mode}\n'
               'Restart=on-failure\nRestartSec=1\n')
    changed = not unit.exists() or unit.read_text() != content
    if changed:
        temporary = unit.with_suffix('.tmp')
        temporary.write_text(content)
        temporary.replace(unit)
        subprocess.run(['systemctl', '--user', 'daemon-reload'], check=True)
    subprocess.run(['systemctl', '--user', 'restart' if changed else 'start', 'desktop-foundation-wallpaper.service'], check=True)


if __name__ == '__main__':
    main()
