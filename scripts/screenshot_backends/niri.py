"""Niri region capture: Wayland selection and screencopy, PNG in memory only."""
import fcntl
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def region():
    config = tomllib.loads((ROOT / 'compositor/niri/screenshots.toml').read_text())
    runtime = Path(os.environ['XDG_RUNTIME_DIR'])
    with (runtime / 'desktop-foundation-region.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return 0
        selection = subprocess.run([
            'slurp', '-d', '-b', config['background'], '-c', config['border'],
            '-s', config['selection'], '-B', config['label_background'],
            '-F', config['font'], '-w', str(config['border_width']),
        ], input=b'', capture_output=True)
        # No stdin boxes: every invocation starts with an empty selection.
        if selection.returncode != 0:
            return 0
        geometry = selection.stdout.decode().strip()
        if not geometry:
            return 0
        # stdout contains PNG only; no intermediate file or persistent save.
        image = subprocess.run(['grim', '-g', geometry, '-'], capture_output=True, check=True).stdout
        subprocess.run(['wl-copy', '--type', 'image/png'], input=image, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return 0
