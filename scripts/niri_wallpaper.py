"""One Niri-session-owned swaybg; no resident custom worker."""
import os
from pathlib import Path
import shutil
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
    os.execv(executable, [executable, '--image', str(image), '--mode', mode])


if __name__ == '__main__':
    main()
