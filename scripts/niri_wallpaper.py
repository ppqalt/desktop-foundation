"""One Niri-session-owned swaybg; no resident custom worker."""
import os
from pathlib import Path
import shutil
import tomllib
import hashlib
import json


def cache_backdrop(image, mode):
    """Blur once per image revision, outside the resident shell render loop."""
    from PIL import Image, ImageFilter
    cache = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'desktop-foundation/overview'
    cache.mkdir(parents=True, exist_ok=True)
    stat = image.stat()
    sizing = 'scaled' if mode in {'fill', 'fit'} else 'original'
    key = hashlib.sha256(f'{image}:{stat.st_mtime_ns}:{stat.st_size}:{sizing}:blur24-v1'.encode()).hexdigest()
    target = cache / f'{key}.png'
    if not target.exists():
        with Image.open(image) as source:
            source = source.convert('RGB')
            if sizing == 'scaled':
                source.thumbnail((1920, 1920))
            source.filter(ImageFilter.GaussianBlur(24)).save(target)
    manifest = cache / 'backdrop.json'
    temporary = manifest.with_suffix('.tmp')
    temporary.write_text(json.dumps({'image': target.as_uri(), 'mode': mode}))
    temporary.replace(manifest)
    for old in cache.glob('*.png'):
        if old != target:
            old.unlink()

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
    cache_backdrop(image, mode)
    os.execv(executable, [executable, '--image', str(image), '--mode', mode])


if __name__ == '__main__':
    main()
