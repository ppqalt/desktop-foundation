"""On-demand Hyprland capture; no compositor commands in the shared driver."""
import json
import math
import os
import re
import subprocess


class Cancelled(Exception):
    pass


def snapshot(resource):
    result = subprocess.run(['hyprctl', '-j', resource], capture_output=True, text=True, check=True, timeout=3)
    return json.loads(result.stdout)


def capture(action, destination):
    if not os.environ.get('HYPRLAND_INSTANCE_SIGNATURE'):
        raise RuntimeError('Hyprland session unavailable')
    command = ['grim', '-t', 'png']
    if action == 'region':
        selected = subprocess.run(['slurp', '-b', '#080b1080', '-c', '#b8ceeebb', '-s', '#b8ceee18', '-w', '1'], capture_output=True, text=True)
        if selected.returncode:
            # slurp returns 1 on Escape and on initialization errors; distinguish
            # the known cancellation message from a broken Wayland connection.
            if selected.stderr.strip() == 'selection cancelled':
                raise Cancelled()
            raise RuntimeError(selected.stderr.strip() or 'Region selection failed')
        geometry = selected.stdout.strip()
        if not re.fullmatch(r'-?\d+,-?\d+ [1-9]\d*x[1-9]\d*', geometry):
            raise RuntimeError('No valid rectangular region selected')
        command += ['-g', geometry]
    elif action == 'output':
        monitor = next((m for m in snapshot('monitors') if m.get('focused') and not m.get('disabled')), None)
        if not monitor:
            raise RuntimeError('No focused output available')
        command += ['-o', monitor['name']]
    elif action == 'window':
        window = snapshot('activewindow')
        if not window.get('address') or not window.get('mapped') or window.get('hidden'):
            raise RuntimeError('No focused window available')
        monitor = next((m for m in snapshot('monitors') if m['id'] == window.get('monitor')), None)
        if not monitor:
            raise RuntimeError('Focused window output is unavailable')
        # Capture visible pixels of the focused window, clipping scrolled-off
        # columns to their owning output. This is a screen crop, not a hidden
        # window-buffer capture; covering surfaces remain visible in the image.
        width, height = monitor['width'], monitor['height']
        if monitor.get('transform', 0) % 2:
            width, height = height, width
        mx, my, mw, mh = monitor['x'], monitor['y'], width / monitor['scale'], height / monitor['scale']
        wx, wy = window['at']; ww, wh = window['size']
        x, y = math.ceil(max(wx, mx)), math.ceil(max(wy, my))
        right, bottom = math.floor(min(wx + ww, mx + mw)), math.floor(min(wy + wh, my + mh))
        if right <= x or bottom <= y:
            raise RuntimeError('Focused window has no visible area')
        command += ['-g', f'{x},{y} {right-x}x{bottom-y}']
    subprocess.run([*command, str(destination)], check=True, timeout=20)
