#!/usr/bin/env python3
"""Live test using only two owned Kitty windows; restore original focus."""
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent

def hypr(code):
    result = subprocess.run(['hyprctl', 'dispatch', code], capture_output=True, text=True, check=True)
    if result.stdout.strip() != 'ok':
        raise RuntimeError(result.stdout + result.stderr)

def ipc(method, *args):
    return subprocess.run(['quickshell', 'ipc', '--path', str(ROOT / 'shell'), 'call', 'foundation', method, *args], capture_output=True, text=True, check=True).stdout

def status():
    return json.loads(ipc('status'))

def wait_for(predicate):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        result = status()
        if predicate(result):
            return result
        time.sleep(0.03)
    raise RuntimeError('Timed out waiting for compositor event/state')

original = status()
niri = original['backend'] == 'niri'
used = {w['id'] for w in original['workspaces']}
workspace = (next(w['id'] for w in original['workspaces'] if not any(v['workspaceId'] == w['id'] for v in original['windows']))
             if niri else next(str(i) for i in range(91, 120) if str(i) not in used))
processes = []
try:
    ipc('focusWorkspace', workspace)
    wait_for(lambda s: s['activeWorkspace']['id'] == workspace)
    for index in range(2):
        process = subprocess.Popen([str(ROOT / 'scripts/launch'), 'kitty', '--override', 'confirm_os_window_close=0', '--class', f'foundation-validation-{index}', '--title', f'Foundation validation {index}', 'sh', '-c', 'sleep 30'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        processes.append(process)
    state = wait_for(lambda s: len([w for w in s['windows'] if w['appId'].startswith('foundation-validation-')]) == 2)
    windows = [w for w in state['windows'] if w['appId'].startswith('foundation-validation-')]
    first, second = [w['id'] for w in windows]
    ipc('focusWindow', first)
    wait_for(lambda s: s['focusedWindow'] is not None and s['focusedWindow']['id'] == first)
    if niri:
        ipc('moveDirection', 'right')
        ipc('moveDirection', 'left')
    else:
        hypr('hl.dsp.layout("swapcol r")')
        hypr('hl.dsp.layout("move +col")')
    ipc('focusWindow', second)
    wait_for(lambda s: s['focusedWindow'] is not None and s['focusedWindow']['id'] == second)
    for method, field in [('setFullscreen', 'fullscreen'), ('setMaximized', 'maximized'), ('setFloating', 'floating')]:
        if method in ('setFullscreen', 'setMaximized') and not original['capabilities'].get(method, False):
            continue
        ipc(method, second, 'true')
        wait_for(lambda s: any(w['id'] == second and w['state'][field] for w in s['windows']))
        ipc(method, second, 'true')
        wait_for(lambda s: any(w['id'] == second and w['state'][field] for w in s['windows']))
        ipc(method, second, 'false')
        wait_for(lambda s: any(w['id'] == second and not w['state'][field] for w in s['windows']))
    ipc('moveWindow', first, original['activeWorkspace']['id'])
    wait_for(lambda s: any(w['id'] == first and w['workspaceId'] == original['activeWorkspace']['id'] for w in s['windows']))
    ipc('closeWindow', first)
    wait_for(lambda s: all(w['id'] != first for w in s['windows']))
    ipc('closeWindow', second)
    wait_for(lambda s: all(w['id'] != second for w in s['windows']))
    ipc('showProbe')
    time.sleep(0.5)  # Allow compositor animations; this is only a finite test.
    if niri:
        assert status()['probeAlive']
    else:
        layers = json.loads(subprocess.check_output(['hyprctl', '-j', 'layers'], text=True))
        assert any(layer['namespace'] == 'desktop-foundation-probe' and layer['alpha'] > 0 for output in layers.values() for group in output['levels'].values() for layer in group)
    ipc('hideProbe')
    time.sleep(0.5)
    assert not status()['probeAlive']
    if not niri:
        layers = json.loads(subprocess.check_output(['hyprctl', '-j', 'layers'], text=True))
        assert not any(layer['namespace'] == 'desktop-foundation-probe' for output in layers.values() for group in output['levels'].values() for layer in group)
    print('PASS: Kitty launch, event-driven focus/workspace/window state, scrolling swap/move, targeted move/close, lazy probe creation/removal')
finally:
    ipc('hideProbe')
    # Native spawn wrappers can exit before the client. Close only our new IDs.
    for window in status()['windows']:
        if window['appId'].startswith('foundation-validation-') and window['id'] not in {w['id'] for w in original['windows']}:
            ipc('closeWindow', window['id'])
    for process in processes:
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=5)
    ipc('focusWorkspace', original['activeWorkspace']['id'])
    if original['focusedWindow']:
        ipc('focusWindow', original['focusedWindow']['id'])
