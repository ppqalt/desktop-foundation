#!/usr/bin/env python3
"""Finite live launcher test with temporary desktop entry and uinput keyboard."""
import fcntl
import json
import os
import signal
from pathlib import Path
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    return subprocess.check_output(args, text=True)


def ipc(method, *args):
    return run('quickshell', 'ipc', '--path', str(ROOT / 'shell'), 'call', 'foundation', method, *args)


def wait(predicate):
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.04)
    raise RuntimeError('Timed out waiting for launcher/input state: ' + ipc('launcherStatus'))


def state():
    return json.loads(ipc('launcherStatus'))


def main():
    original = json.loads(run('hyprctl', '-j', 'activeworkspace'))
    cursor = json.loads(run('hyprctl', '-j', 'cursorpos'))
    apps = Path(os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share'))) / 'applications'
    apps.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix='foundation-launcher-test-', suffix='.desktop', dir=apps)
    os.close(descriptor)
    fixture = Path(name)
    terminal_fixture = fixture.with_name(fixture.stem + "-terminal.desktop")
    fixture.write_text('[Desktop Entry]\nType=Application\nName=Foundation Launcher Validation\nExec=kitty --override confirm_os_window_close=0 --class foundation-launcher-validation sh -c "sleep 30"\nIcon=utilities-terminal\nTerminal=false\n')
    fd = None
    terminal = None
    def emit(code, value):
        os.write(fd, struct.pack('llHHi', 0, 0, 1, code, value))
        os.write(fd, struct.pack('llHHi', 0, 0, 0, 0, 0))
        time.sleep(.02)
    def key(code, mods=()):
        for mod in mods:
            emit(mod, 1)
        emit(code, 1)
        emit(code, 0)
        for mod in reversed(mods):
            emit(mod, 0)
        time.sleep(.12)
    try:
        fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
        fcntl.ioctl(fd, 0x40045564, 1)
        for code in range(1, 256):
            fcntl.ioctl(fd, 0x40045565, code)
        fcntl.ioctl(fd, 0x405c5503, struct.pack('HHHH80sI', 3, 0x1209, 1, 1, b'foundation-launcher-validation', 0))
        fcntl.ioctl(fd, 0x40045564, 2)
        for axis in [0, 1]: fcntl.ioctl(fd, 0x40045566, axis)
        fcntl.ioctl(fd, 0x40045565, 272)
        fcntl.ioctl(fd, 0x5501)
        wait(lambda: any(k['name'] == 'foundation-launcher-validation' for k in json.loads(run('hyprctl', '-j', 'devices'))['keyboards']))
        ipc('hideLauncher')
        wait(lambda: not json.loads(ipc('status'))['launcherAlive'])
        key(57, (125,))  # Real Super+Space binding.
        wait(lambda: state().get('visible'))
        # Real Finnish keyboard text input, exact ranking and cursor navigation.
        for code in [37, 23, 20, 20, 21]:
            key(code)
        wait(lambda: state().get('query') == 'kitty')
        assert state()['selected'] == 'kitty'
        key(1)  # Escape destroys the lazy surface.
        wait(lambda: not json.loads(ipc('status'))['launcherAlive'])
        key(57, (125,))
        wait(lambda: state().get('visible'))
        first = state()['selected']
        key(108)
        assert state()['selected'] != first
        key(103)
        assert state()['selected'] == first
        ipc('launcherQuery', 'Foundation Launcher Validation')
        wait(lambda: state().get('count') == 1 and state().get('selected') == fixture.stem)
        key(28)
        wait(lambda: not json.loads(ipc('status'))['launcherAlive'])
        window = wait(lambda: next((w for w in json.loads(run('hyprctl', '-j', 'clients')) if w['class'] == 'foundation-launcher-validation'), None))
        run('hyprctl', 'dispatch', 'hl.dsp.window.close({window="address:' + window['address'] + '"})')
        terminal_fixture.write_text('[Desktop Entry]\nType=Application\nName=Foundation Launcher Validation Terminal\nExec=/usr/bin/sleep 30\nTerminal=true\n')
        before_terminal = {w['address'] for w in json.loads(run('hyprctl', '-j', 'clients'))}
        ipc('showLauncher')
        ipc('launcherQuery', 'Foundation Launcher Validation Terminal')
        wait(lambda: state().get('count') == 1 and state().get('selected') == terminal_fixture.stem)
        key(28)
        wait(lambda: not json.loads(ipc('status'))['launcherAlive'])
        terminal = wait(lambda: next((w for w in json.loads(run('hyprctl', '-j', 'clients')) if w['class'] == 'kitty' and w['address'] not in before_terminal), None))
        os.kill(terminal['pid'], signal.SIGTERM)
        wait(lambda: all(w['address'] != terminal['address'] for w in json.loads(run('hyprctl', '-j', 'clients'))))
        ipc('showLauncher')
        ipc('launcherQuery', 'no-such-app-zzzzzz')
        wait(lambda: state().get('count') == 0)
        key(28)
        assert state()['visible']  # Empty results must not launch or dismiss.
        key(1)
        wait(lambda: not json.loads(ipc('status'))['launcherAlive'])
        ipc('showLauncher')
        wait(lambda: state().get('visible'))
        current = json.loads(run('hyprctl', '-j', 'cursorpos'))
        for axis, delta in [(0, -int(current['x']) - 200), (1, -int(current['y']) - 200)]:
            os.write(fd, struct.pack('llHHi', 0, 0, 2, axis, delta))
        os.write(fd, struct.pack('llHHi', 0, 0, 0, 0, 0))
        time.sleep(.1)
        key(272)
        wait(lambda: not json.loads(ipc('status'))['launcherAlive'])
        print('PASS: real Super+Space, fi typing, arrows, Escape, Enter launch, native desktop-entry updates, empty state and lazy teardown')
    finally:
        if terminal and any(w['address'] == terminal['address'] for w in json.loads(run('hyprctl', '-j', 'clients'))):
            os.kill(terminal['pid'], signal.SIGTERM)
        fixture.unlink(missing_ok=True)
        terminal_fixture.unlink(missing_ok=True)
        if fd is not None:
            for code in [125, 42, 29]:
                emit(code, 0)
            current = json.loads(run('hyprctl', '-j', 'cursorpos'))
            for axis, delta in [(0, int(cursor['x'] - current['x'])), (1, int(cursor['y'] - current['y']))]:
                os.write(fd, struct.pack('llHHi', 0, 0, 2, axis, delta))
            os.write(fd, struct.pack('llHHi', 0, 0, 0, 0, 0))
            fcntl.ioctl(fd, 0x5502)
            os.close(fd)
        ipc('hideLauncher')
        for window in json.loads(run('hyprctl', '-j', 'clients')):
            if window['class'] == 'foundation-launcher-validation' or window['title'] == 'foundation-launcher-terminal-validation':
                run('hyprctl', 'dispatch', 'hl.dsp.window.close({window="address:' + window['address'] + '"})')
        run('hyprctl', 'dispatch', 'hl.dsp.focus({workspace=' + str(original['id']) + '})')


if __name__ == '__main__':
    main()
