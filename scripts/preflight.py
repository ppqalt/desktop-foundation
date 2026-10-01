#!/usr/bin/env python3
"""Validate runtime dependencies and native compositor support before deployment."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
from render_niri import render

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--profile', default='default')
args = parser.parse_args()
required = ['niri', 'quickshell', 'kitty', 'fish', 'fastfetch', 'lua', 'wl-copy', 'wl-paste',
            'wl-clip-persist', 'swaybg', 'mako', 'grim', 'slurp', 'wpctl', 'playerctl', 'notify-send',
            'dbus-update-activation-environment', 'gsettings', 'xdg-mime', 'fc-cache', 'pacman-conf', 'xwayland-satellite']
missing = [name for name in required if not shutil.which(name)]
if missing:
    raise SystemExit('Missing runtime tools: ' + ', '.join(missing) + '. Run scripts/bootstrap.')
with tempfile.NamedTemporaryFile(mode='w', suffix='.kdl') as candidate:
    candidate.write(render(ROOT, args.profile))
    candidate.flush()
    result = subprocess.run(['niri', 'validate', '-c', candidate.name], capture_output=True, text=True)
    if result.returncode:
        raise SystemExit('Installed Niri cannot validate this configuration. Nothing deployed.\n'
                         'This setup requires the blur/background-effect-capable Niri build used by CachyOS.\n'
                         'See docs/installation.md; do not substitute an unverified compositor build.\n' + result.stderr)
print('PASS runtime tools and native Niri configuration. Original user configuration can now be backed up and deployed.')
