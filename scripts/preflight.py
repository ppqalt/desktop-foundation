#!/usr/bin/env python3
"""Validate runtime dependencies and native compositor support before deployment."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from render_niri import render

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--profile', default='default')
parser.add_argument('--update', action='store_true', help='Validate deployment requirements; report missing optional runtime tools without installing them')
parser.add_argument('--compositor', choices=['niri', 'hyprland'], default='niri')
args = parser.parse_args()
required = ['pactl', 'busctl', 'blueman-manager', 'niri', 'quickshell', 'kitty', 'slangc', 'fish', 'fastfetch', 'lua', 'wl-copy', 'wl-paste',
            'wl-clip-persist', 'swaybg', 'mako', 'grim', 'slurp', 'wpctl', 'playerctl', 'notify-send',
            'dbus-update-activation-environment', 'gsettings', 'xdg-mime', 'fc-cache', 'pacman-conf', 'xwayland-satellite']
if args.compositor == 'hyprland':
    required[required.index('niri')] = 'Hyprland'
missing = [name for name in required if not shutil.which(name)]
backend = ROOT / 'native/foundation/target/release/desktop-foundationctl'
if not backend.is_file() or not os.access(backend, os.X_OK):
    raise SystemExit('Shared Rust backend is not built. Run scripts/build-backend; nothing deployed.')
if args.update:
    # These are executed by rendering/deployment itself. Missing optional tools
    # must not prevent an existing desktop from receiving source/config updates.
    mandatory = {'niri' if args.compositor == 'niri' else 'Hyprland', 'quickshell',
                 'kitty', 'slangc', 'fish', 'lua', 'fc-cache'}
    blocking = [name for name in missing if name in mandatory]
    optional = [name for name in missing if name not in mandatory]
    if blocking:
        raise SystemExit('Missing deployment tools: ' + ', '.join(blocking)
                         + '. Install these separately; df-update does not run package managers.')
    if optional:
        print('Optional runtime tools unavailable: ' + ', '.join(optional)
              + '. Their dependent controls remain unavailable; no packages installed.')
elif missing:
    raise SystemExit('Missing runtime tools: ' + ', '.join(missing) + '. Run scripts/bootstrap.')
with tempfile.NamedTemporaryFile(mode='w', suffix='.kdl' if args.compositor == 'niri' else '.lua') as candidate:
    if args.compositor == 'niri':
        candidate.write(render(ROOT, args.profile))
    else:
        from deploy import lua_string
        candidate.write(f'FOUNDATION_ROOT = {lua_string(ROOT)}\nFOUNDATION_PROFILE = {lua_string(args.profile)}\ndofile(FOUNDATION_ROOT .. "/compositor/hyprland/hyprland.lua")\n')
    candidate.flush()
    command = ['niri', 'validate', '-c', candidate.name] if args.compositor == 'niri' else ['Hyprland', '--verify-config', '-c', candidate.name]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f'Installed {args.compositor} cannot validate this configuration. Nothing deployed.\n'
                         + ('This setup requires the blur/background-effect-capable Niri build used by CachyOS.\n' if args.compositor == 'niri' else '')
                         + 'See docs/installation.md.\n' + result.stderr)
print(f'PASS runtime tools and native {args.compositor} configuration. Original user configuration can now be backed up and deployed.')
