"""One Niri-session-owned swaybg; no resident custom worker."""
import os
from pathlib import Path
import shutil
import tomllib
import hashlib
import json
import tempfile


def prepare_backdrop(image, mode):
    """Blur once per image revision, outside the resident shell render loop."""
    from PIL import Image, ImageFilter
    from theme_pipeline import file_hash
    cache = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'desktop-foundation/overview'
    cache.mkdir(parents=True, exist_ok=True)
    sizing = 'scaled' if mode in {'fill', 'fit'} else 'original'
    digest = file_hash(image)
    key = hashlib.sha256(f'{digest}:{sizing}:blur24-v1'.encode()).hexdigest()
    target = cache / f'{key}.png'
    if target.is_symlink():
        raise RuntimeError('Overview cache must not be a symlink')
    if target.exists():
        try:
            with Image.open(target) as check: check.verify()
            return target
        except (OSError, ValueError):
            pass  # Reproducible corrupt cache is replaced only after completion.
    fd, temporary = tempfile.mkstemp(prefix='.' + target.name, suffix='.tmp', dir=cache)
    try:
        with os.fdopen(fd, 'wb') as output, Image.open(image) as original:
            source = original.convert('RGB')
            if sizing == 'scaled':
                source.thumbnail((1920, 1920))
            source.filter(ImageFilter.GaussianBlur(24)).save(output, format='PNG')
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, target)
        directory = os.open(cache, os.O_DIRECTORY)
        try: os.fsync(directory)
        finally: os.close(directory)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)
    return target


def cache_backdrop(image, mode):
    target = prepare_backdrop(image, mode)
    cache = target.parent
    manifest = cache / 'backdrop.json'
    from theme_pipeline import atomic
    atomic(manifest, json.dumps({'image': target.as_uri(), 'mode': mode}))

ROOT = Path(__file__).resolve().parent.parent


def quote(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def main():
    if not os.environ.get('NIRI_SOCKET'):
        raise RuntimeError('Wallpaper startup requires a Niri session')
    from theme_runtime import config as wallpaper_config, current
    config = wallpaper_config()
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
    if current() is None:
        cache_backdrop(image, mode)
    os.execv(executable, [executable, '--image', str(image), '--mode', mode])


if __name__ == '__main__':
    main()
