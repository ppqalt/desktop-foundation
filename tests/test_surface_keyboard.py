"""Isolated checks of production QML keyboard policies, without a desktop session.

The real Keys.onPressed bodies and navigation/action functions run in Node with
state and action spies. This covers policy, not Qt focus or physical key delivery.
"""
import json
from pathlib import Path
import re
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


def block(source, marker):
    start = source.index('{', source.index(marker))
    depth = 1
    index = start + 1
    # These production blocks use simple strings and have no braces in comments.
    quote = None
    escaped = False
    while depth:
        char = source[index]
        if quote:
            if escaped:
                escaped = False
            elif char == '\\':
                escaped = True
            elif char == quote:
                quote = None
        elif char in ('"', "'"):
            quote = char
        elif char == '{':
            depth += 1
        elif char == '}':
            depth -= 1
        index += 1
    return source[start + 1:index - 1]


def production(surface, names):
    source = (ROOT / 'shell/surfaces' / (surface + '.qml')).read_text()
    functions = []
    for name in names:
        signature = re.search(r'function ' + name + r'\((.*?)\)\s*:\s*(?:void|bool|var|string)\s*\{', source)
        if signature is None:
            raise AssertionError('Missing production function: ' + name)
        arguments = re.sub(r':\s*(?:int|real|string|bool|var)\b', '', signature.group(1))
        functions.append('function ' + name + '(' + arguments + ') {' + block(source, 'function ' + name + '(') + '}')
    entry = re.search(r'Keys.onPressed:\s*event\s*=>\s*([^\n]+)', source).group(1)
    handler = block(source, 'Keys.onPressed:') if entry.startswith('{') else entry + ';'
    return '\n'.join(functions) + '\nhandler = event => {' + handler + '};'


@unittest.skipUnless(shutil.which('node'), 'isolated keyboard policy checks require Node')
class SurfaceKeyboard(unittest.TestCase):
    def run_policy(self, surface, names, checks):
        code = r'''
const vm = require('node:vm');
const assert = require('node:assert/strict');
const events = [];
const Qt = {ControlModifier: 1, ShiftModifier: 2};
['Escape', 'Down', 'Up', 'Left', 'Right', 'N', 'P', 'Return', 'Enter',
 'Space', 'Tab', 'Backtab', 'Delete', 'PageUp', 'PageDown', 'Home', 'End',
 'A', '1', '2', '3', '4'].forEach((name, index) => Qt['Key_' + name] = index + 100);
const context = {
    Qt, Math, closing: false, confirmClear: false, clearSelected: false,
    clearChoice: true, selectedIndex: 0, selected: 0, busy: false, error: '',
    results: Array.from({length: 12}, (_, index) => ({id: 'item-' + index})),
    actions: ['suspend', 'logout', 'reboot', 'poweroff'].map(id => ({id})),
    search: {focus: true, text: 'query', forceActiveFocus() {events.push('focus');}},
    list: {height: 198, positionViewAtIndex(index) {events.push(['visible', index]);}},
    ListView: {Contain: 1}, exitAnimation: {start() {events.push('dismiss');}},
    applications: {launch(entry) {events.push(['launch', entry.id]); return true;}},
    history: {entries: Array.from({length: 12}, (_, index) => ({id: 'item-' + index})),
              busy: false, request(action, identity) {events.push([action, identity]);}},
    lifecycle: {powerEnabled: true}, actionProcess: {command: [], running: false},
    Quickshell: {env() {return '/isolated/fixture';}}
};
context.root = context;
vm.createContext(context);
vm.runInContext(PRODUCTION, context);
function key(name, modifiers = 0, repeated = false) {
    const event = {key: Qt['Key_' + name], modifiers, isAutoRepeat: repeated, accepted: false};
    context.handler(event);
    return event.accepted;
}
'''
        code = code.replace('PRODUCTION', json.dumps(production(surface, names))) + checks
        result = subprocess.run(['node', '-e', code], capture_output=True, text=True,
                                timeout=10, env={'PATH': '/usr/bin:/bin'})
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_launcher_reverse_and_page_navigation_preserve_search_editing(self):
        self.run_policy('Launcher', ['dismiss', 'launchSelected', 'navigate'], r'''
assert.equal(key('PageDown'), true);
assert.equal(context.selectedIndex, 3);
assert.equal(key('Backtab'), true);
assert.equal(context.selectedIndex, 2);
key('Tab', Qt.ShiftModifier); assert.equal(context.selectedIndex, 1);
key('Down', 0, true); assert.equal(context.selectedIndex, 2);
key('P', Qt.ControlModifier); assert.equal(context.selectedIndex, 1);
key('PageUp'); assert.equal(context.selectedIndex, 0);
for (const name of ['Home', 'End', 'Left', 'Right', 'Space', 'A'])
    assert.equal(key(name), false, name + ' belongs to the search field');
key('Return', 0, true); assert.equal(events.filter(e => e[0] === 'launch').length, 0);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'launch'), ['launch', 'item-0']);
assert.equal(context.closing, true);
key('Return'); assert.equal(events.filter(e => e[0] === 'launch').length, 1);
''')

    def test_clipboard_all_actions_are_reachable_and_confirmation_is_modal(self):
        self.run_policy('Clipboard', ['dismiss', 'launchSelected', 'navigate', 'tabNavigate',
                                    'beginClear', 'acceptClear', 'deleteSelected', 'keyboardNavigate', 'handleKey'], r'''
key('PageDown'); assert.equal(context.selectedIndex, 3);
key('PageUp'); assert.equal(context.selectedIndex, 0);
key('Up'); assert.equal(context.clearSelected, true);
key('Down'); assert.equal(context.clearSelected, false); assert.equal(context.selectedIndex, 0);
key('Backtab'); assert.equal(context.clearSelected, true);
key('Tab'); assert.equal(context.clearSelected, false); assert.equal(context.selectedIndex, 0);
key('Tab', Qt.ShiftModifier); assert.equal(context.clearSelected, true);
key('Return'); assert.equal(context.confirmClear, true);
const before = context.selectedIndex;
key('A'); key('Delete', Qt.ControlModifier); key('PageDown');
assert.equal(context.search.text, 'query'); assert.equal(context.selectedIndex, before);
assert.equal(events.some(e => e[0] === 'delete' || e[0] === 'copy'), false);
key('Backtab'); assert.equal(context.clearChoice, false);
key('Space'); assert.equal(context.confirmClear, false);
assert.equal(events.some(e => e[0] === 'clear'), false);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier, true);
assert.equal(context.confirmClear, false);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier); assert.equal(context.confirmClear, true);
key('Return', 0, true); assert.equal(events.some(e => e[0] === 'clear'), false);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'clear'), ['clear', '']);
assert.equal(context.confirmClear, false);
key('Delete', Qt.ControlModifier, true); assert.equal(events.some(e => e[0] === 'delete'), false);
key('Delete', Qt.ControlModifier); assert.deepEqual(events.find(e => e[0] === 'delete'), ['delete', 'item-0']);
for (const name of ['Home', 'End', 'Left', 'Right', 'Space', 'A'])
    assert.equal(key(name), false, name + ' belongs to the search field');
key('Return', 0, true); assert.equal(events.some(e => e[0] === 'copy'), false);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'copy'), ['copy', 'item-0']);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier); key('Escape');
assert.equal(context.confirmClear, false); assert.equal(context.closing, false);
key('Escape'); assert.equal(context.closing, true);
''')
        source = (ROOT / 'shell/surfaces/Clipboard.qml').read_text()
        self.assertIn('readOnly: root.confirmClear', source)
        self.assertIn('enabled: !root.confirmClear', source)

    def test_clipboard_empty_filtered_results_still_reach_clear_all(self):
        self.run_policy('Clipboard', ['dismiss', 'launchSelected', 'navigate', 'tabNavigate',
                                    'beginClear', 'acceptClear', 'deleteSelected', 'keyboardNavigate', 'handleKey'], r'''
context.results = [];
key('Tab'); assert.equal(context.clearSelected, true);
key('Return'); assert.equal(context.confirmClear, true);
key('Left'); assert.equal(context.clearChoice, false);
key('Home'); assert.equal(context.clearChoice, false);
key('End'); assert.equal(context.clearChoice, true);
key('Escape'); assert.equal(context.confirmClear, false);
context.history.entries = [];
context.clearSelected = false;
key('Tab'); key('Delete', Qt.ControlModifier | Qt.ShiftModifier);
assert.equal(context.confirmClear, false);
assert.equal(events.some(e => ['clear', 'copy', 'delete'].includes(e[0])), false);
''')

    def test_power_keyboard_navigation_and_actions_only_call_harmless_spies(self):
        self.run_policy('Power', ['navigate', 'dismiss', 'choose', 'execute'], r'''
key('Backtab'); assert.equal(context.selected, 3);
key('Tab'); assert.equal(context.selected, 0);
key('Tab', Qt.ShiftModifier); assert.equal(context.selected, 3);
key('Home'); assert.equal(context.selected, 0);
key('Right'); assert.equal(context.selected, 1);
key('Down', 0, true); assert.equal(context.selected, 2);
key('Left'); assert.equal(context.selected, 1);
key('End'); assert.equal(context.selected, 3);
key('PageUp'); assert.equal(context.selected, 0);
key('PageDown'); assert.equal(context.selected, 3);
key('Return', 0, true); assert.equal(context.busy, false);
key('Space'); assert.equal(context.busy, true);
assert.equal(context.actionProcess.command[1], 'poweroff');
key('Home'); assert.equal(context.selected, 3);
for (let index = 1; index <= 4; index++) {
    context.busy = false;
    key(String(index), 0, true); assert.equal(context.busy, false);
    key(String(index)); assert.equal(context.actionProcess.command[1], context.actions[index - 1].id);
}
context.busy = false;
key('Escape'); assert.equal(context.lifecycle.powerEnabled, false);
''')

    def test_bluetooth_keyboard_actions_selection_and_focus_handoff(self):
        source = (ROOT / 'shell/surfaces/BluetoothPopup.qml').read_text()
        checks = r'''
Object.assign(context, {
    selectionKey: '', hoverNavigationEnabled: false, lastPointer: {x: -1, y: -1},
    radioBusy: false, controlsOpening: false, pendingActivation: '', pendingControls: null,
    controlDevice: null, discoveryDone: true, controlPaths: ['/first'], unsupportedPaths: [],
    adapterAvailable: true, batteryQueue: [], batteryProbe: {running: false},
    radioAction: {command: [], running: false}, action: {command: [], running: false},
    devices: [{dbusPath: '/first', name: 'First', connected: true, adapter: {enabled: true}},
              {dbusPath: '/second', name: 'Second', connected: false, adapter: {enabled: true}}],
    Bluetooth: {adapters: {values: [{enabled: true}]}},
    controlsLoader: {item: null}, closeTimer: {start() {events.push('dismiss');}}
});
context.Quickshell.execDetached = args => events.push(['detached', args]);
const callbacks = [];
Qt.point = (x, y) => ({x, y});
Qt.callLater = callback => callbacks.push(callback);
context.card = {forceActiveFocus() {events.push('parent-focus');}};
key('Home'); assert.equal(context.selected, -1);
key('Space', 0, true); assert.equal(context.radioAction.running, false);
key('Space'); assert.equal(context.radioAction.running, true);
assert.equal(context.radioAction.command.at(-1), 'off');
context.radioBusy = false;
key('End'); assert.equal(context.selected, 2);
key('Backtab'); assert.equal(context.selected, 1);
key('Tab', Qt.ShiftModifier); assert.equal(context.selected, 0);
key('PageDown'); assert.equal(context.selected, 2);
key('PageUp'); assert.equal(context.selected, -1);
key('Down', 0, true); assert.equal(context.selected, 0);
key('Return', 0, true); assert.equal(context.controlDevice, null);
key('Return'); assert.equal(context.controlDevice.dbusPath, '/first');
assert.equal(context.action.running, false);
context.controlDevice = null;
key('Down'); key('Return'); assert.equal(context.action.running, true);
assert.equal(context.action.command.at(-2), 'connect');
assert.equal(context.action.command.at(-1), '/second');
context.busy = false;
context.controlsOpening = true; context.action.running = false;
key('Return'); assert.equal(context.action.running, false);
context.controlsOpening = false;
context.selected = 1; context.rememberSelection();
context.devices = [{dbusPath: '/new'}, {dbusPath: '/first'}, {dbusPath: '/second'}];
DEVICES_CHANGED;
assert.equal(context.selected, 2); assert.equal(context.selectionKey, '/second');
context.selected = 3; context.rememberSelection();
context.devices = [{dbusPath: '/second'}];
DEVICES_CHANGED;
assert.equal(context.selected, 1);
context.pointerSelection(0, 50, 50); assert.equal(context.selected, 1);
context.pointerSelection(0, 50, 50); assert.equal(context.selected, 1);
context.pointerSelection(0, 52, 50); assert.equal(context.selected, 0);
context.controlDevice = {};
context.pointerSelection(-1, 55, 50); assert.equal(context.selected, 0);
context.restoreFocus(); assert.equal(events.includes('parent-focus'), false);
callbacks.shift()(); assert.equal(events.includes('parent-focus'), false);
context.controlDevice = null; context.restoreFocus(); callbacks.shift()();
assert.equal(events.filter(e => e === 'parent-focus').length, 1);
context.pointerSelection(-1, 70, 70); assert.equal(context.selected, 0);
context.closing = true; context.restoreFocus(); callbacks.shift()();
assert.equal(events.filter(e => e === 'parent-focus').length, 1);
context.closing = false;
key('End'); key('Space');
assert.deepEqual(Array.from(events.find(e => e[0] === 'detached')[1]), ['blueman-manager']);
assert.equal(context.closing, true);
'''
        checks = checks.replace('DEVICES_CHANGED;',
                                'vm.runInContext(' + json.dumps('(function() {' + block(source, 'onDevicesChanged:') + '})()') + ', context);')
        self.run_policy('BluetoothPopup', ['navigate', 'selectIndex', 'rememberSelection',
                                          'pointerSelection', 'restoreFocus', 'dismiss',
                                          'supportsControls', 'activate', 'toggleRadio'], checks)
