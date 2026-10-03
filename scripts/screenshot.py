#!/usr/bin/env python3
"""Logical screenshot actions; shared storage intent, explicit native backend."""
import argparse
from datetime import datetime
import fcntl
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def save_image(image, config):
    directory = Path(config['directory']).expanduser()
    if not directory.is_absolute():
        raise ValueError('Screenshot directory must be absolute or start with ~/')
    filename = datetime.now().astimezone().strftime(config['filename'])
    if Path(filename).name != filename or not filename.endswith('.png'):
        raise ValueError('Screenshot filename must be a PNG basename')
    directory.mkdir(parents=True, exist_ok=True)
    destination = directory / filename
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=directory, prefix='.capture-', suffix='.tmp', delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(image)
            stream.flush()
            os.fsync(stream.fileno())
        # Publish only complete files; never overwrite a colliding screenshot.
        os.link(temporary, destination)
    finally:
        if temporary:
            temporary.unlink(missing_ok=True)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', default=os.environ.get('DF_COMPOSITOR', 'niri'))
    parser.add_argument('action', choices=['region', 'window', 'output'])
    args = parser.parse_args()
    if args.backend == 'niri':
        if args.action == 'region':
            from screenshot_backends import niri
            try:
                return niri.region()
            except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
                print(f'Screenshot: {error}', file=sys.stderr)
                return 1
        mapping = {'window': 'screenshot-window', 'output': 'screenshot-screen'}
        try:
            command = ['niri', 'msg', 'action', mapping[args.action]]
            command += ['--show-pointer', 'false', '--write-to-disk', 'false']
            return subprocess.run(command, check=True, timeout=5).returncode
        except (OSError, subprocess.SubprocessError) as error:
            print(f'Screenshot: {error}', file=sys.stderr)
            return 1
    if args.backend != 'hyprland':
        parser.error(f'{args.backend} screenshot backend is not implemented')
    from screenshot_backends import hyprland
    os.umask(0o077)
    try:
        config = tomllib.loads((ROOT / 'config/screenshots.toml').read_text())
        runtime = Path(os.environ['XDG_RUNTIME_DIR'])
        with (runtime / 'desktop-foundation-screenshot.lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return 0  # A selection/capture is already in progress.
            with tempfile.TemporaryDirectory(prefix='foundation-capture-', dir=runtime) as temporary:
                capture = Path(temporary) / 'capture.png'
                hyprland.capture(args.action, capture)
                image = capture.read_bytes()
                if len(image) < 24 or image[:8] != b'\x89PNG\r\n\x1a\n' or image[12:16] != b'IHDR':
                    raise RuntimeError('Capture did not produce a valid PNG')
                destination = save_image(image, config)
                try:
                    # wl-copy forks an owner: inherited pipes must not keep a
                    # calling QProcess/terminal waiting for stdout/stderr EOF.
                    subprocess.run(['wl-copy', '--type', 'image/png'], input=image, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True, timeout=5)
                except (OSError, subprocess.SubprocessError):
                    raise RuntimeError(f'Image saved at {destination}, but copying to the clipboard failed')
                print(destination)
        return 0
    except hyprland.Cancelled:
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'Screenshot: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
