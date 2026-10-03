"""Niri region capture: Wayland selection and screencopy, PNG in memory only."""
import fcntl
import os
from pathlib import Path
import subprocess
import re
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
            diagnostic = selection.stderr.decode(errors='replace').strip()
            if selection.returncode == 1 and (not diagnostic or diagnostic.lower() in {'selection cancelled', 'selection canceled'}):
                return 0
            raise RuntimeError('Region selection failed: ' + (diagnostic or str(selection.returncode)))
        geometry = selection.stdout.decode().strip()
        if not geometry:
            return 0
        if not re.fullmatch(r'-?\d+,-?\d+ [1-9]\d*x[1-9]\d*', geometry):
            raise ValueError('Region selection returned invalid geometry')
        # stdout contains PNG only; no intermediate file or persistent save.
        image = subprocess.run(['grim', '-g', geometry, '-'], capture_output=True, check=True, timeout=20).stdout
        if len(image) < 24 or image[:8] != b'\x89PNG\r\n\x1a\n' or image[12:16] != b'IHDR':
            raise RuntimeError('Capture did not produce a valid PNG')
        subprocess.run(['wl-copy', '--type', 'image/png'], input=image, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
        return 0
