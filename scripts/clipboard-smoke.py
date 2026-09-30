#!/usr/bin/env python3
"""Live clipboard UI tests in isolated storage; preserve original clipboard."""
import base64
import fcntl
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def run(*args, **kwargs):
    if args[0] == 'wl-copy':
        # Background clipboard owners inherit pipes; never wait for stdout EOF.
        return subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True, timeout=5, **kwargs)
    return subprocess.run(args, capture_output=True, check=True, timeout=10, **kwargs)


def wait(predicate):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.04)
    raise RuntimeError('Clipboard test timed out')


def main():
    types = subprocess.run(['wl-paste', '--list-types'], capture_output=True).stdout.decode().splitlines()
    original_type = next((t for t in ['text/plain;charset=utf-8', 'text/plain', 'image/png', 'image/jpeg'] if t in types), types[0] if types else '')
    original = run('wl-paste', '--no-newline', '--type', original_type).stdout if original_type else b''
    processes = []
    fd = None
    shell = None
    def resident(method):
        return run('quickshell', 'ipc', '--path', str(ROOT / 'shell'), 'call', 'foundation', method).stdout
    def key(code, mods=()):
        def emit(keycode, value):
            os.write(fd, struct.pack('llHHi', 0, 0, 1, keycode, value))
            os.write(fd, struct.pack('llHHi', 0, 0, 0, 0, 0))
            time.sleep(.02)
        for modifier in mods: emit(modifier, 1)
        emit(code, 1); emit(code, 0)
        for modifier in reversed(mods): emit(modifier, 0)
        time.sleep(.15)
    try:
        fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
        fcntl.ioctl(fd, 0x40045564, 1)
        for code in range(1, 256): fcntl.ioctl(fd, 0x40045565, code)
        fcntl.ioctl(fd, 0x405c5503, struct.pack('HHHH80sI', 3, 0x1209, 2, 1, b'foundation-clipboard-validation', 0))
        fcntl.ioctl(fd, 0x5501)
        wait(lambda: any(k['name'] == 'foundation-clipboard-validation' for k in json.loads(run('hyprctl', '-j', 'devices').stdout)['keyboards']))
        resident('hideClipboard'); resident('hideLauncher')
        wait(lambda: not json.loads(resident('status'))['clipboardAlive'])
        key(47, (125,))  # Actual Super+V.
        wait(lambda: json.loads(resident('status'))['clipboardAlive'])
        key(1)
        wait(lambda: not json.loads(resident('status'))['clipboardAlive'])
        run('systemctl', '--user', 'stop', 'desktop-foundation-clipboard@text.service', 'desktop-foundation-clipboard@image.service')
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            state = base / 'history'
            env = {**os.environ, 'DF_CLIPBOARD_STATE': str(state)}
            run('python3', str(ROOT / 'scripts/clipboard.py'), '--state', str(state), 'init')
            def entries(): return json.loads((state / 'index.json').read_text())
            for kind in ['text', 'image']:
                processes.append(subprocess.Popen([str(ROOT / 'scripts/clipboard-watch'), kind], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
            payload = 'Finnish: ä ö å €\nsecond line\n'.encode()
            run('wl-copy', '--type', 'text/plain;charset=utf-8', input=payload)
            wait(lambda: any(e['preview'] == payload.decode() for e in entries()))
            count = len(entries())
            run('wl-copy', '--type', 'text/plain;charset=utf-8', input=payload)
            time.sleep(.3)
            assert len(entries()) == count
            run('wl-copy', '--sensitive', input=b'clipboard-sensitive-fixture')
            time.sleep(.4)
            assert all('clipboard-sensitive-fixture' not in e['preview'] for e in entries())
            png = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aHioAAAAASUVORK5CYII=')
            run('wl-copy', '--type', 'image/png', input=png)
            wait(lambda: any(e['mime'] == 'image/png' for e in entries()))
            (base / 'shell.qml').write_text('''pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "''' + (ROOT / 'shell/surfaces').as_uri() + '''" as Surfaces
ShellRoot {
 id: root
 property bool clipboardEnabled: false
 property bool clipboardAlive: false
 readonly property Surfaces.Clipboard surface: loader.item as Surfaces.Clipboard
 LazyLoader { id: loader; active: root.clipboardEnabled; Surfaces.Clipboard { lifecycle: root; targetScreen: Quickshell.screens[0] } }
 IpcHandler {
 target: "clipboardTest"
 function openSurface(): void { root.clipboardEnabled = true; }
 function query(value: string): void { root.surface.setQuery(value); }
 function status(): string { return JSON.stringify(root.surface ? root.surface.snapshot() : {visible:false}); }
 function confirmClear(): void { root.surface.confirmClear = true; }
 function hide(): void { if (root.surface) root.surface.dismiss(); }
 }
}
''')
            with (base / 'qml.log').open('w') as log:
                shell = subprocess.Popen(['quickshell', '--path', str(base)], env=env, stdout=log, stderr=log)
                def ipc(method, *args):
                    result = subprocess.run(['quickshell', 'ipc', '--pid', str(shell.pid), 'call', 'clipboardTest', method, *args], capture_output=True, timeout=3)
                    if result.returncode:
                        raise RuntimeError(result.stderr.decode())
                    return result.stdout
                def status():
                    try: return json.loads(ipc('status'))
                    except (json.JSONDecodeError, RuntimeError): return {}
                try:
                    wait(lambda: status().get('visible') is False)
                except RuntimeError:
                    raise RuntimeError((base / 'qml.log').read_text())
                ipc('openSurface')
                try:
                    wait(lambda: status().get('count', 0) > 0 and not status().get('loading'))
                except RuntimeError:
                    raise RuntimeError(str(status()) + '\n' + (base / 'qml.log').read_text())
                ipc('query', 'Finnish')
                wait(lambda: status().get('count') == 1)
                key(28)
                wait(lambda: status().get('visible') is False)
                assert run('wl-paste', '--no-newline').stdout == payload
                ipc('openSurface'); wait(lambda: status().get('visible'))
                ipc('query', 'image/png'); wait(lambda: status().get('count') == 1)
                key(28); wait(lambda: status().get('visible') is False)
                assert run('wl-paste', '--type', 'image/png').stdout == png
                ipc('openSurface'); wait(lambda: status().get('visible'))
                ipc('query', 'Finnish'); wait(lambda: status().get('count') == 1)
                key(111, (29,))  # Ctrl+Delete removes selected item.
                wait(lambda: status().get('count') == 0)
                ipc('query', ''); wait(lambda: status().get('count', 0) > 0)
                ipc('confirmClear'); key(1)
                assert status()['confirmClear'] is False and entries()
                ipc('confirmClear'); key(28)
                wait(lambda: entries() == [])
                wait(lambda: status().get('count') == 0)
                # Demonstration uses only controlled fixtures; never screenshot user history.
                for text in ['Keep the desktop clean. Show only what I need.', 'https://quickshell.org/docs/', 'Finnish layout: ä ö å · Shift+7 → /', 'git status --short', 'A quiet surface. A quick interaction.']:
                    run('python3', str(ROOT / 'scripts/clipboard.py'), '--state', str(state), 'store', 'text', input=text.encode())
                image = Path('/home/ppq/Documents/Codex/2026-09-30/f/outputs/launcher-search-preview.png')
                if image.exists():
                    run('python3', str(ROOT / 'scripts/clipboard.py'), '--state', str(state), 'store', 'image', input=image.read_bytes())
                wait(lambda: status().get('count', 0) >= 5)
                time.sleep(.4)
                run('grim', '-g', '640,251 640x578', '/home/ppq/Documents/Codex/2026-09-30/f/outputs/clipboard-preview.png')
                ipc('hide'); wait(lambda: status().get('visible') is False)
            print('PASS: Super+V, sensitive exclusion, dedup, exact Unicode/newline/image copy, search, delete, clear confirmation/cancel and lazy teardown')
    finally:
        if shell and shell.poll() is None: shell.terminate(); shell.wait(timeout=5)
        for process in processes:
            process.terminate(); process.wait(timeout=5)
        if original_type: run('wl-copy', *(['--sensitive'] if 'x-kde-passwordManagerHint' in types else []), '--type', original_type, input=original)
        else: run('wl-copy', '--clear')
        run('systemctl', '--user', 'start', 'desktop-foundation-clipboard@text.service', 'desktop-foundation-clipboard@image.service')
        if fd is not None:
            fcntl.ioctl(fd, 0x5502); os.close(fd)


if __name__ == '__main__':
    main()
