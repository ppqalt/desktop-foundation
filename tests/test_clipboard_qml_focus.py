"""Clipboard's production content and Qt input in a private offscreen Window.

Only the layer-shell transport is replaced. The production search, confirmation,
focus scopes, handlers and history service run unchanged with an inert worker.
This cannot validate compositor exclusivity or physical keyboard delivery.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def window_transport(source):
    source = source.replace('PanelWindow {', 'Window {', 1)
    source = source.replace('import QtQuick\n', 'import QtQuick\nimport QtQuick.Window\n', 1)
    source, count = re.subn(r'    anchors \{\n        top: true\n        bottom: true\n        left: true\n        right: true\n    \}\n', '', source, count=1)
    if count != 1:
        raise AssertionError('Layer-shell transport changed; review the offscreen host')
    return '\n'.join(line for line in source.splitlines()
                     if not line.startswith(('    screen:', '    exclusionMode:', '    WlrLayershell.'))) + '\n'


QML = r'''
import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "shell/surfaces"
ShellRoot {
    id: fixture
    property int stage: 0
    property int failures: 0
    property bool saved: false
    property var lifecycle: ({clipboardEnabled: true, clipboardAlive: false})
    function check(condition, detail) {
        if (!condition) { failures++; console.error("FOCUS_FAIL " + detail); }
    }
    function card() { return clip.contentItem.children.find(child => child.radius === 24); }
    function capture(name) {
        const directory = Quickshell.env("DF_QOL_PREVIEW_DIR");
        const card = clip.contentItem.children.find(child => child.radius === 24);
        if (directory && card) card.grabToImage(result => result.saveToFile(directory + "/" + name));
    }
    ClipboardFixture {
        id: clip
        width: 900; height: 760; visible: true
        lifecycle: fixture.lifecycle
        TestCase { id: keyboard; name: "ClipboardInput"; optional: true; when: false }
    }
    Timer {
        interval: 150; running: true; repeat: true
        onTriggered: {
            const s = clip.snapshot();
            if (fixture.stage === 0) {
                if (s.loading) return;
                // The layer-shell host can deliver output geometry after QML
                // construction. It must settle without the clear-dialog morph.
                clip.entered = false;
                clip.width = 220;
                Qt.callLater(() => {
                    fixture.check(fixture.card().width === 172, "Initial geometry starts a size morph: " + fixture.card().width);
                    clip.width = 900;
                    Qt.callLater(() => {
                        fixture.check(fixture.card().width === 640, "Initial output width animates instead of settling");
                        clip.entered = true;
                        clip.requestActivate(); clip.present();
                    });
                });
                fixture.check(s.count === 13, "Fixture history not loaded");
            } else if (fixture.stage === 1) {
                fixture.check(clip.snapshot().inputFocused, "Search is not the active Qt input");
                keyboard.keyClick(Qt.Key_Up);
                fixture.check(clip.clearSelected, "Up cannot reach header Clear all");
                keyboard.keyClick(Qt.Key_Down);
                fixture.check(!clip.clearSelected && clip.selectedIndex === 0, "Down cannot return to first item");
                keyboard.keyClick(Qt.Key_Tab, Qt.ShiftModifier);
                fixture.check(clip.clearSelected, "Shift+Tab cannot reach header Clear all");
                keyboard.keyClick(Qt.Key_Return);
                fixture.check(clip.confirmClear, "Enter on header does not open confirmation");
                fixture.check(fixture.card().width > 440 && fixture.card().height > 268, "Clear confirmation lost its size morph");
            } else if (fixture.stage === 2) {
                fixture.check(clip.snapshot().confirmationFocused && !clip.snapshot().inputFocused, "Confirmation has no actual Qt button focus");
                if (Quickshell.env("DF_QOL_PREVIEW_DIR") && !fixture.saved) {
                    fixture.saved = true;
                    fixture.capture("clipboard-clear.png");
                    return;
                }
                keyboard.keyClick(Qt.Key_Left);
                fixture.check(!clip.clearChoice && clip.snapshot().confirmationFocused, "Left cannot focus Cancel");
                keyboard.keyClick(Qt.Key_Right);
                fixture.check(clip.clearChoice && clip.snapshot().confirmationFocused, "Right cannot focus Clear history");
                keyboard.keyClick(Qt.Key_A);
                keyboard.keyClick(Qt.Key_Delete, Qt.ControlModifier);
                fixture.check(clip.snapshot().query === "" && clip.snapshot().count === 13, "Modal typing affects hidden search or history");
                keyboard.keyClick(Qt.Key_Tab);
                fixture.check(!clip.clearChoice, "Tab cannot reach Cancel");
                keyboard.keyClick(Qt.Key_Space);
                fixture.check(!clip.confirmClear, "Space cannot activate Cancel");
            } else if (fixture.stage === 3) {
                fixture.check(clip.snapshot().inputFocused, "Cancel does not restore search focus");
                keyboard.keyClick("x");
                fixture.check(clip.snapshot().query === "x", "Ordinary search typing is lost");
                keyboard.keyClick(Qt.Key_Backspace);
                fixture.check(clip.snapshot().count === 13, "Backspace does not restore results");
                keyboard.keyClick(Qt.Key_Delete, Qt.ControlModifier | Qt.ShiftModifier);
                fixture.check(clip.confirmClear, "Clear shortcut lost after cancel");
                keyboard.keyClick(Qt.Key_Escape);
                fixture.check(!clip.confirmClear && !clip.closing, "Escape closes surface instead of cancelling modal");
                clip.setQuery("no-matching-fixture");
                keyboard.keyClick(Qt.Key_Tab);
                fixture.check(clip.clearSelected, "Filtered empty list cannot reach Clear all");
                keyboard.keyClick(Qt.Key_Return);
            } else if (fixture.stage === 4) {
                fixture.check(clip.snapshot().confirmationFocused && clip.clearChoice, "Reopened confirmation lacks clear-button focus");
                keyboard.keyClick(Qt.Key_Return);
                fixture.check(!clip.confirmClear, "Enter cannot activate Clear history");
            } else if (fixture.stage === 7) {
                clip.setQuery("");
                fixture.check(clip.snapshot().count === 0, "Clear history did not reach isolated worker");
                fixture.check(clip.snapshot().inputFocused, "Clear does not restore normal input");
                console.log("FOCUS_RESULT " + JSON.stringify({failures: fixture.failures}));
                Qt.quit();
            }
            fixture.stage++;
        }
    }
    Timer {interval: 8000; running: true; onTriggered: {console.error("FOCUS_FAIL fixture timed out"); Qt.quit();}}
}
'''


@unittest.skipUnless(shutil.which('quickshell'), 'offscreen input requires Quickshell/QtTest')
class ClipboardFocus(unittest.TestCase):
    def test_clear_is_reachable_through_real_qt_search_and_button_focus(self):
        with tempfile.TemporaryDirectory(prefix='foundation-clipboard-focus-') as directory:
            base = Path(directory)
            surface = base / 'shell/surfaces'; surface.mkdir(parents=True)
            (surface / 'ClipboardFixture.qml').write_text(window_transport((ROOT / 'shell/surfaces/Clipboard.qml').read_text()))
            for name in ('components', 'services', 'theme', 'assets'):
                (base / 'shell' / name).symlink_to(ROOT / 'shell' / name, target_is_directory=True)
            (base / 'shell.qml').write_text(QML)
            state = base / 'clipboard'; state.mkdir()
            entries = [dict(id='item-' + str(index), preview='Fixture text ' + str(index),
                            mime='text/plain') for index in range(13)]
            (state / 'index.json').write_text(json.dumps(entries))
            worker = base / 'worker.py'
            worker.write_text('#!' + sys.executable + '\n' +
                              'import json,pathlib,sys\n'
                              'state=pathlib.Path(sys.argv[sys.argv.index("--state")+1])\n'
                              'action=sys.argv[sys.argv.index("--state")+2]\n'
                              'with (state/"actions.log").open("a") as log: log.write(action+"\\n")\n'
                              'if action == "clear": (state/"index.json").write_text("[]")\n')
            env = dict(os.environ)
            for name in ('WAYLAND_DISPLAY', 'DISPLAY', 'NIRI_SOCKET', 'HYPRLAND_INSTANCE_SIGNATURE',
                         'DBUS_SESSION_BUS_ADDRESS', 'DBUS_SYSTEM_BUS_ADDRESS', 'QS_CONFIG_PATH', 'QS_CONFIG_NAME'):
                env.pop(name, None)
            for name, leaf in (('HOME', 'home'), ('XDG_RUNTIME_DIR', 'runtime'),
                               ('XDG_CONFIG_HOME', 'config'), ('XDG_STATE_HOME', 'state'),
                               ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data')):
                private = base / leaf; private.mkdir(mode=0o700); env[name] = str(private)
            env.update(QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                       DF_FOUNDATION_ROOT=str(base), DF_CLIPBOARD_WORKER=str(worker),
                       DF_CLIPBOARD_STATE=str(state), XDG_DATA_DIRS=str(base / 'data'))
            result = subprocess.run(['quickshell', '--path', str(base)], env=env,
                                    capture_output=True, text=True, timeout=12)
            log = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, log)
            self.assertNotIn('FOCUS_FAIL', log)
            self.assertIn('FOCUS_RESULT {"failures":0}', log)
            self.assertNotIn('TypeError', log)
            self.assertNotIn('ReferenceError', log)
            self.assertNotIn('Failed to load configuration', log)
            self.assertEqual((state / 'actions.log').read_text(), 'clear\n')
