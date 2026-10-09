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
const Qt = {ControlModifier: 1, ShiftModifier: 2, callLater(fn) {fn();}};
['Escape', 'Down', 'Up', 'Left', 'Right', 'N', 'P', 'Return', 'Enter',
 'Space', 'Tab', 'Backtab', 'Delete', 'PageUp', 'PageDown', 'Home', 'End',
 'A', 'C', '1', '2', '3', '4'].forEach((name, index) => Qt['Key_' + name] = index + 100);
const context = {
    Qt, Math, closing: false, confirmClear: false, clearSelected: false,
    clearChoice: 0, clearChoiceCount: 3, pinnedCount: 1, actionFocus: 0, selectedIdentity: "item-0", selectedViewportOffset: 0, selectionRevision: 0, resultsQuery: "query", viewportRestorePending: false, viewportAtTop: true, viewportIdentity: "", viewportOffset: 0, viewportContentY: 0, selectedIndex: 0, selected: 0, busy: false, error: '',
    results: Array.from({length: 12}, (_, index) => ({id: 'item-' + index, kind: index === 0 ? 'rich' : 'text', mime: 'text/plain', pinned: false})),
    actions: ['suspend', 'logout', 'reboot', 'poweroff'].map(id => ({id})),
    search: {focus: true, text: 'query', forceActiveFocus() {events.push('focus');}},
    list: {height: 198, contentHeight: 792, originY: 0, contentY: 0, forceLayout() {}, indexAt(x, y) {return Math.floor(y / 66);}, itemAtIndex(index) {return {y: index * 66, height: 62};}, positionViewAtBeginning() {this.contentY = 0;}, positionViewAtIndex(index, mode) {events.push(['visible', index]); if (mode === 2) this.contentY = index * 66;}},
    ListView: {Contain: 1, Beginning: 2}, exitAnimation: {start() {events.push('dismiss');}},
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

    CLIPBOARD_FUNCTIONS = ['dismiss', 'launchSelected', 'navigate', 'tabNavigate',
                           'beginClear', 'acceptClear', 'deleteSelected', 'keyboardNavigate',
                           'handleKey', 'plainAvailable', 'pinSelected', 'copyPlainSelected',
                           'rememberSelection', 'captureViewport', 'selectIndex', 'reconcileSelection', 'queryChanged']

    def test_clipboard_actions_pin_plain_text_and_keyboard_focus(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
key('PageDown'); assert.equal(context.selectedIndex, 3);
key('PageUp'); assert.equal(context.selectedIndex, 0);
key('P', Qt.ControlModifier); assert.equal(context.clearSelected, true, 'Ctrl+P remains previous-row navigation');
key('Down'); assert.equal(context.clearSelected, false); assert.equal(context.selectedIndex, 0);
key('P', Qt.ControlModifier | Qt.ShiftModifier, true); assert.equal(events.some(e => e[0] === 'pin'), false);
key('P', Qt.ControlModifier | Qt.ShiftModifier); assert.deepEqual(events.find(e => e[0] === 'pin'), ['pin', 'item-0']);
context.results[0].pinned = true;
key('P', Qt.ControlModifier | Qt.ShiftModifier); assert.deepEqual(events.find(e => e[0] === 'unpin'), ['unpin', 'item-0']);
key('Tab'); assert.equal(context.actionFocus, 1, 'Tab reaches Pin');
key('Tab'); assert.equal(context.actionFocus, 2, 'Tab reaches Copy plain text');
key('Return', 0, true); assert.equal(events.some(e => e[0] === 'copy-plain'), false);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'copy-plain'), ['copy-plain', 'item-0']);
key('Tab'); assert.equal(context.selectedIndex, 1); assert.equal(context.actionFocus, 0);
key('Backtab'); assert.equal(context.selectedIndex, 0); assert.equal(context.actionFocus, 2);
key('Escape'); assert.equal(context.actionFocus, 0); assert.equal(context.closing, false);
for (const name of ['Home', 'End', 'Left', 'Right', 'Space', 'A'])
    assert.equal(key(name), false, name + ' belongs to the search field');
key('C', Qt.ControlModifier | Qt.ShiftModifier); assert.equal(events.filter(e => e[0] === 'copy-plain').length, 2);
key('Delete', Qt.ControlModifier, true); assert.equal(events.some(e => e[0] === 'delete'), false);
key('Delete', Qt.ControlModifier); assert.deepEqual(events.find(e => e[0] === 'delete'), ['delete', 'item-0']);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'copy'), ['copy', 'item-0']);
context.results[0].kind = 'files'; context.actionFocus = 1;
key('Tab'); assert.equal(context.selectedIndex, 1); assert.equal(context.actionFocus, 0, 'Files do not expose copy-plain');
key('Escape'); assert.equal(context.closing, true);
''')

    def test_clipboard_cleanup_defaults_cancel_and_explicitly_preserves_pins(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
key('Up'); assert.equal(context.clearSelected, true);
key('Return'); assert.equal(context.confirmClear, true); assert.equal(context.clearChoice, 0);
const before = context.selectedIndex;
key('A'); key('Delete', Qt.ControlModifier); key('PageDown'); key('P', Qt.ControlModifier | Qt.ShiftModifier);
assert.equal(context.search.text, 'query'); assert.equal(context.selectedIndex, before);
assert.equal(events.some(e => ['delete', 'pin', 'copy'].includes(e[0])), false);
key('Return'); assert.equal(context.confirmClear, false); assert.equal(events.some(e => e[0] === 'clear'), false);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier, true); assert.equal(context.confirmClear, false);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier); assert.equal(context.clearChoice, 0);
key('Right'); assert.equal(context.clearChoice, 1);
key('Return', 0, true); assert.equal(events.some(e => e[0] === 'clear'), false);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'clear'), ['clear', '']);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier); key('End'); assert.equal(context.clearChoice, 2);
key('Space'); assert.deepEqual(events.find(e => e[0] === 'clear-all'), ['clear-all', '']);
key('Delete', Qt.ControlModifier | Qt.ShiftModifier); key('Escape');
assert.equal(context.confirmClear, false); assert.equal(context.closing, false);
''')
        source = (ROOT / 'shell/surfaces/Clipboard.qml').read_text()
        self.assertIn('readOnly: root.confirmClear', source)
        self.assertIn('enabled: !root.confirmClear', source)

    def test_clipboard_navigation_stops_at_header_and_last_item(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
key('Up'); assert.equal(context.clearSelected, true);
for (let i = 0; i < 3; i++) {
    key('Up', 0, true); key('Backtab'); key('PageUp');
    assert.equal(context.clearSelected, true);
    assert.equal(context.selectedIndex, 0, 'Clear must not wrap to the last item');
}
key('Down'); assert.equal(context.clearSelected, false); assert.equal(context.selectedIndex, 0);
key('Backtab'); assert.equal(context.clearSelected, true);
key('Tab'); assert.equal(context.clearSelected, false); assert.equal(context.selectedIndex, 0);
context.selectIndex(context.results.length - 1);
for (let i = 0; i < 3; i++) {
    key('Down', 0, true); key('N', Qt.ControlModifier); key('PageDown');
    assert.equal(context.clearSelected, false);
    assert.equal(context.selectedIndex, context.results.length - 1, 'Last item must not wrap through Clear');
}
key('Tab'); assert.equal(context.actionFocus, 1);
for (let i = 0; i < 3; i++) key('Tab');
assert.equal(context.selectedIndex, context.results.length - 1);
assert.equal(context.actionFocus, 1); assert.equal(context.clearSelected, false);
key('Backtab'); assert.equal(context.actionFocus, 0);
key('Up'); assert.equal(context.selectedIndex, context.results.length - 2);
context.clearSelected = true;
key('PageDown'); assert.equal(context.selectedIndex, 2, 'Paging from Clear starts above the first item');
assert.equal(context.clearSelected, false);
''')

    def test_clipboard_empty_filtered_results_still_reach_cleanup(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
context.results = [];
key('Tab'); assert.equal(context.clearSelected, true);
key('Return'); assert.equal(context.confirmClear, true); assert.equal(context.clearChoice, 0);
key('Right'); assert.equal(context.clearChoice, 1);
key('End'); assert.equal(context.clearChoice, 2);
key('Home'); assert.equal(context.clearChoice, 0);
key('Escape'); assert.equal(context.confirmClear, false);
context.history.entries = []; context.clearSelected = false;
key('Tab'); key('Delete', Qt.ControlModifier | Qt.ShiftModifier);
assert.equal(context.confirmClear, false);
assert.equal(events.some(e => ['clear', 'clear-all', 'copy', 'delete'].includes(e[0])), false);
''')

    def test_clipboard_all_pinned_history_can_reach_explicit_clear_everything(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
context.results = [{id: 'pin-only', pinned: true, kind: 'image'}];
context.history.entries = context.results; context.pinnedCount = 1;
context.selectedIndex = 0; context.selectedIdentity = 'pin-only';
key('Up'); assert.equal(context.clearSelected, true);
key('Return'); assert.equal(context.confirmClear, true); assert.equal(context.clearChoice, 0);
key('Tab'); assert.equal(context.clearChoice, 1);
key('Tab'); assert.equal(context.clearChoice, 2);
key('Return'); assert.deepEqual(events.find(e => e[0] === 'clear-all'), ['clear-all', '']);
''')

    def test_clipboard_background_copy_preserves_top_and_scrolled_viewports(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
context.results[0].pinned = true;
context.selectIndex(1); context.rememberSelection(); context.captureViewport();
context.results.splice(1, 0, {id: 'new-top', pinned: false, kind: 'text'});
context.reconcileSelection();
assert.equal(context.selectedIdentity, 'item-1'); assert.equal(context.selectedIndex, 2);
assert.equal(context.list.contentY, 0, 'Incoming copy must not hide the top pinned item');
context.list.contentY = 264; context.captureViewport();
assert.equal(context.viewportIdentity, 'item-3');
context.results.splice(1, 0, {id: 'new-scrolled', pinned: false, kind: 'text'});
context.reconcileSelection();
assert.equal(context.list.contentY, 330, 'First visible item keeps its old viewport offset');
assert.equal(context.selectedIdentity, 'item-1');
context.results.splice(context.selectedIndex, 1); context.reconcileSelection();
assert.equal(context.selectedIdentity, 'item-2', 'Deletion chooses a neighboring item');
''')

    def test_clipboard_new_query_cancels_stale_history_scroll_restore(self):
        self.run_policy('Clipboard', self.CLIPBOARD_FUNCTIONS, r'''
const deferred = []; Qt.callLater = callback => deferred.push(callback);
context.selectIndex(5); context.list.contentY = 500; context.rememberSelection();
context.captureViewport(); context.reconcileSelection();
context.results = [context.results[5]];
context.queryChanged(); assert.equal(context.list.contentY, 0);
for (const callback of deferred) callback();
assert.equal(context.selectedIdentity, 'item-5');
assert.equal(context.list.contentY, 0, 'Older index refresh must not scroll new search results');
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
key('Escape'); assert.equal(context.closing, true);
assert.equal(events.includes('dismiss'), true);
const lastCommand = Array.from(context.actionProcess.command);
key('Return'); assert.deepEqual(Array.from(context.actionProcess.command), lastCommand);
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
context.updateDeviceSelection();
assert.equal(context.selected, 2); assert.equal(context.selectionKey, '/second');
context.selected = 3; context.rememberSelection();
context.devices = [{dbusPath: '/second'}];
context.updateDeviceSelection();
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
        self.assertIn('onDevicesChanged: updateDeviceSelection()', source)
        self.run_policy('BluetoothPopup', ['navigate', 'selectIndex', 'rememberSelection', 'updateDeviceSelection',
                                          'pointerSelection', 'restoreFocus', 'dismiss',
                                          'supportsControls', 'activate', 'toggleRadio'], checks)

    def test_bluetooth_async_start_defaults_to_first_device_without_stealing_user_choice(self):
        source = (ROOT / 'shell/surfaces/BluetoothPopup.qml').read_text()
        self.assertIn('property int selected: -1', source)
        self.assertIn('property bool defaultSelectionPending: true', source)
        self.assertIn('root.updateDeviceSelection();', block(source, 'Component.onCompleted:'))
        self.run_policy('BluetoothPopup', ['navigate', 'selectIndex', 'rememberSelection',
                                          'updateDeviceSelection', 'pointerSelection'], r'''
Object.assign(context, {
    devices: [], selected: -1, selectionKey: '', defaultSelectionPending: true,
    controlDevice: null, hoverNavigationEnabled: false, lastPointer: {x: -1, y: -1}
});
Qt.point = (x, y) => ({x, y});
context.updateDeviceSelection();
assert.equal(context.selected, -1, 'empty initial list uses radio, never Manage');
assert.equal(context.defaultSelectionPending, true);
context.updateDeviceSelection();
assert.equal(context.defaultSelectionPending, true, 'empty updates do not consume first-device choice');
context.devices = [{dbusPath: '/first'}, {dbusPath: '/second'}];
context.updateDeviceSelection();
assert.equal(context.selected, 0);
assert.equal(context.selectionKey, '/first');
assert.equal(context.defaultSelectionPending, false);
context.devices = [{dbusPath: '/earlier'}, {dbusPath: '/first'}, {dbusPath: '/second'}];
context.updateDeviceSelection();
assert.equal(context.selected, 1, 'selected device identity survives later insertions');
context.devices = [{dbusPath: '/second'}];
context.updateDeviceSelection();
assert.equal(context.selected, 0, 'removed device falls back to a device, not Manage');

// An explicit Manage choice during initial loading must survive the arrival.
Object.assign(context, {devices: [], selected: -1, selectionKey: '', defaultSelectionPending: true});
context.selectIndex(0); context.rememberSelection();
assert.equal(context.selectionKey, 'manage');
context.devices = [{dbusPath: '/first'}, {dbusPath: '/second'}];
context.updateDeviceSelection();
assert.equal(context.selected, 2);

// So must choosing the radio while loading, even when its index is unchanged.
Object.assign(context, {devices: [], selected: -1, selectionKey: '', defaultSelectionPending: true});
context.selectIndex(-1); context.rememberSelection();
context.devices = [{dbusPath: '/first'}];
context.updateDeviceSelection();
assert.equal(context.selected, -1);
assert.equal(context.selectionKey, 'radio');

// A stationary pointer does not cancel the default; deliberate movement does.
Object.assign(context, {devices: [], selected: -1, selectionKey: '', defaultSelectionPending: true,
                        lastPointer: {x: -1, y: -1}});
context.pointerSelection(0, 10, 10);
assert.equal(context.defaultSelectionPending, true);
context.pointerSelection(0, 10, 10);
assert.equal(context.defaultSelectionPending, true);
context.pointerSelection(0, 13, 10); context.rememberSelection();
assert.equal(context.defaultSelectionPending, false);
context.devices = [{dbusPath: '/first'}];
context.updateDeviceSelection();
assert.equal(context.selected, 1, 'deliberate Manage hover remains selected');

// Reopening creates fresh state and chooses the top row, even after Manage.
Object.assign(context, {selected: -1, selectionKey: '', defaultSelectionPending: true});
context.updateDeviceSelection();
assert.equal(context.selected, 0);
assert.equal(context.selectionKey, '/first');
''')
