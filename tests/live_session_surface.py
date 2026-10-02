#!/usr/bin/env python3
"""Opt-in Niri keyboard regressions; no power, radio or earbud changes."""
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import struct
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('surface_input', ROOT / 'tests/live_nothing_surface.py')
ui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)


def wait(name, predicate):
    end = time.monotonic() + 5
    while time.monotonic() < end:
        state = ui.state(name)
        if predicate(state):
            return state
        time.sleep(.04)
    raise AssertionError((name, state))


def main():
    # Keyboard-only device: creating it emits no pointer position/motion.
    fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
    fcntl.ioctl(fd, 0x40045564, 1)
    for code in range(1, 256):
        fcntl.ioctl(fd, 0x40045565, code)
    fcntl.ioctl(fd, 0x40045564, 2)
    fcntl.ioctl(fd, 0x40045566, 8)
    fcntl.ioctl(fd, 0x405c5503, struct.pack('HHHH80sI', 3, 0x1209, 29, 1, b'foundation-startup-keyboard-validation', 0))
    fcntl.ioctl(fd, 0x5501)
    ui.fd = fd
    time.sleep(.3)
    try:
        ui.key(57, (125,))
        first = wait('launcher', lambda s: s.get('visible') and s.get('inputFocused'))
        ui.key(108)
        wait('launcher', lambda s: s['selected'] != first['selected'])
        for code in [37, 23, 20, 20, 21]:
            ui.key(code)
        wait('launcher', lambda s: s['query'] == 'kitty')
        ui.key(1)
        wait('launcher', lambda s: not s.get('visible'))
        ui.key(47, (125,))
        wait('clipboard', lambda s: s.get('visible') and s.get('inputFocused') and not s.get('loading'))
        ui.key(1)
        wait('clipboard', lambda s: not s.get('visible'))
        ui.key(48, (125,))
        wait('bluetooth', lambda s: s.get('visible'))
        ui.key(103)
        wait('bluetooth', lambda s: s['selected'] == -1)
        ui.key(108)
        wait('bluetooth', lambda s: s['selected'] == 0)
        ui.key(1)
        wait('bluetooth', lambda s: not s.get('visible'))
        ui.key(16, (125, 42))
        wait('power', lambda s: s.get('visible'))
        ui.key(108)
        ui.key(1)
        wait('power', lambda s: not s.get('visible'))
        print('PASS: four real keybindings; keyboard-only launcher open/focus/typing, arrows, clipboard readiness, Bluetooth switch navigation, power navigation and Escape cleanup')
    finally:
        ui.key(1)
        for code in [125, 42, 29]:
            ui.event(1, code, 0)
        fcntl.ioctl(fd, 0x5502)
        os.close(fd)


if __name__ == '__main__':
    main()
