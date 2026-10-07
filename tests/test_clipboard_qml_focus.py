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
                clip.entered = false;
                clip.width = 220;
                Qt.callLater(() => {
                    fixture.check(fixture.card().width === 172, "Initial geometry starts a size morph");
                    clip.width = 900;
                    Qt.callLater(() => {
                        fixture.check(fixture.card().width === 640, "Initial output width animates instead of settling");
                        clip.entered = true;
                        clip.requestActivate(); clip.present();
                    });
                });
                fixture.check(s.count === 13, "Fixture history not loaded");
            } else if (fixture.stage === 1) {
                fixture.capture("clipboard.png");
                fixture.check(s.inputFocused, "Search is not the active Qt input");
                keyboard.keyClick(Qt.Key_Tab);
                fixture.check(clip.actionFocus === 1, "Tab cannot reach Pin");
                keyboard.keyClick(Qt.Key_Tab);
                fixture.check(clip.actionFocus === 2, "Tab cannot reach Copy plain");
                keyboard.keyClick(Qt.Key_Escape);
                fixture.check(!clip.closing && clip.actionFocus === 0, "Escape cannot return to normal row selection");
                keyboard.keyClick(Qt.Key_Up);
                fixture.check(clip.clearSelected, "Up cannot reach header Clear history");
                keyboard.keyClick(Qt.Key_Down);
                fixture.check(!clip.clearSelected && clip.selectedIndex === 0, "Down cannot return to first item");
                clip.navigate(4);
                keyboard.keyClick(Qt.Key_P, Qt.ControlModifier | Qt.ShiftModifier);
            } else if (fixture.stage === 2) {
                if (s.pinnedCount !== 1) return;
                fixture.capture("clipboard-pinned.png");
                fixture.check(s.selected === "item-4" && clip.selectedIndex === 0, "Pin refresh resets selection identity");
                keyboard.keyClick(Qt.Key_P, Qt.ControlModifier | Qt.ShiftModifier);
            } else if (fixture.stage === 3) {
                if (s.pinnedCount) return;
                fixture.check(s.selected === "item-4" && clip.selectedIndex === 4, "Unpin refresh resets selection identity");
                keyboard.keyClick(Qt.Key_P, Qt.ControlModifier);
                fixture.check(clip.selectedIndex === 3, "Ctrl+P no longer navigates upward");
                clip.navigate(-3);
                keyboard.keyClick(Qt.Key_C, Qt.ControlModifier | Qt.ShiftModifier);
            } else if (fixture.stage === 4) {
                if (!s.error) return;
                fixture.check(!clip.closing && s.selected === "item-0", "Copy failure closes the surface or changes selection");
                keyboard.keyClick(Qt.Key_P, Qt.ControlModifier | Qt.ShiftModifier);
            } else if (fixture.stage === 5) {
                if (s.error.includes("Pinned history is full")) {
                    fixture.check(s.error.includes("unpin an item first") && !s.error.includes("Foundation:") && !s.error.includes("trace"), "Pin budget error loses remedy or exposes unsanitized output");
                    keyboard.keyClick(Qt.Key_P, Qt.ControlModifier | Qt.ShiftModifier);
                    return;
                }
                if (s.pinnedCount !== 1) return;
                keyboard.keyClick(Qt.Key_Tab, Qt.ShiftModifier);
                fixture.check(clip.clearSelected, "Shift+Tab cannot reach header Clear history");
                keyboard.keyClick(Qt.Key_Return);
                fixture.check(clip.confirmClear && clip.clearChoice === 0, "Cleanup does not conservatively default to Cancel");
                fixture.check(fixture.card().width > 440 && fixture.card().height > 268, "Clear confirmation lost its size morph");
            } else if (fixture.stage === 6) {
                fixture.check(s.confirmationFocused && !s.inputFocused, "Confirmation has no actual Qt button focus");
                if (Quickshell.env("DF_QOL_PREVIEW_DIR") && !fixture.saved) {
                    fixture.saved = true;
                    fixture.capture("clipboard-clear.png");
                    return;
                }
                keyboard.keyClick(Qt.Key_A);
                keyboard.keyClick(Qt.Key_Delete, Qt.ControlModifier);
                fixture.check(clip.snapshot().query === "" && clip.snapshot().count === 13, "Modal typing affects hidden search or history");
                keyboard.keyClick(Qt.Key_Tab);
                fixture.check(clip.clearChoice === 1 && clip.snapshot().confirmationFocused, "Tab cannot focus Clear history");
                keyboard.keyClick(Qt.Key_Tab, Qt.ShiftModifier);
                fixture.check(clip.clearChoice === 0, "Shift+Tab cannot focus Cancel");
                keyboard.keyClick(Qt.Key_Space);
                fixture.check(!clip.confirmClear, "Space cannot activate Cancel");
                clip.setQuery("fixture-video.mp4");
            } else if (fixture.stage === 7) {
                fixture.check(s.inputFocused && s.count === 1, "Filename search or focus restoration fails");
                if (!s.error) { keyboard.keyClick(Qt.Key_Return); return; }
                fixture.check(!clip.closing && s.error.includes("Restore the original"), "Missing file copy closes surface or loses recovery message");
                keyboard.keyClick(Qt.Key_Tab); keyboard.keyClick(Qt.Key_Tab);
                fixture.check(clip.clearSelected, "Files expose a nonfunctional plain-copy control or cannot reach Clear history");
                keyboard.keyClick(Qt.Key_Return);
                keyboard.keyClick(Qt.Key_Right);
                fixture.check(clip.clearChoice === 1, "Right cannot choose normal cleanup");
                keyboard.keyClick(Qt.Key_Return);
            } else if (fixture.stage === 8) {
                clip.setQuery("");
                if (clip.snapshot().count !== 1) return;
                fixture.check(clip.snapshot().pinnedCount === 1, "Normal cleanup removed the pinned item");
                keyboard.keyClick(Qt.Key_Delete, Qt.ControlModifier | Qt.ShiftModifier);
                fixture.check(clip.confirmClear && clip.clearChoice === 0, "Reopened cleanup does not start at Cancel");
                keyboard.keyClick(Qt.Key_End);
                fixture.check(clip.clearChoice === 2 && clip.snapshot().confirmationFocused, "Keyboard cannot reach explicit Clear everything");
                keyboard.keyClick(Qt.Key_Return);
            } else if (fixture.stage === 9) {
                if (s.count) return;
                fixture.check(s.pinnedCount === 0 && s.inputFocused, "Clear everything did not finish or restore focus");
                console.log("FOCUS_RESULT " + JSON.stringify({failures: fixture.failures}));
                Qt.quit();
            }
            fixture.stage++;
        }
    }
    Timer {interval: 12000; running: true; onTriggered: {console.error("FOCUS_FAIL fixture timed out at " + fixture.stage); Qt.quit();}}

}
'''


@unittest.skipUnless(shutil.which('quickshell'), 'offscreen input requires Quickshell/QtTest')
class ClipboardFocus(unittest.TestCase):
    def test_pins_row_actions_and_safe_cleanup_use_real_qt_focus(self):
        with tempfile.TemporaryDirectory(prefix='foundation-clipboard-focus-') as directory:
            base = Path(directory)
            surface = base / 'shell/surfaces'; surface.mkdir(parents=True)
            (surface / 'ClipboardFixture.qml').write_text(window_transport((ROOT / 'shell/surfaces/Clipboard.qml').read_text()))
            for name in ('components', 'services', 'theme', 'assets'):
                (base / 'shell' / name).symlink_to(ROOT / 'shell' / name, target_is_directory=True)
            (base / 'shell.qml').write_text(QML)
            state = base / 'clipboard'; state.mkdir()
            entries = [dict(id='item-' + str(index), preview='Fixture text ' + str(index),
                            mime='text/plain', kind='text', pinned=False, size=32, image='', formats=['text/plain']) for index in range(13)]
            entries[0].update(kind='rich', mime='text/html', label='Formatted fixture', formats=['text/plain', 'text/html'])
            entries[1].update(kind='files', mime='text/uri-list', label='settings.toml, README.md + 1', preview='settings.toml README.md logs.zip', fileCount=3, detail='3 files · TOML, Markdown, ZIP', formats=['text/uri-list'])
            entries[2].update(kind='image', mime='image/png', label='Copied image', preview='', detail='PNG · 124 KB', formats=['image/png'])
            entries[12].update(kind='files', mime='text/uri-list', label='fixture-video.mp4', preview='fixture-video.mp4', fileCount=1, detail='1 file · Video', formats=['text/uri-list'], unavailable=True)
            (state / 'index.json').write_text(json.dumps(entries))
            worker = base / 'worker.py'
            worker.write_text('#!' + sys.executable + '\n' +
                              'import json,pathlib,sys\n'
                              'state=pathlib.Path(sys.argv[sys.argv.index("--state")+1])\n'
                              'action=sys.argv[sys.argv.index("--state")+2]\n'
                              'with (state/"actions.log").open("a") as log: log.write(action+"\\n")\n'
                              'if action == "copy-plain": print("fixture copy failed", file=sys.stderr); sys.exit(1)\n'
                              'if action == "copy": print("Foundation: A copied file or folder is no longer available", file=sys.stderr); sys.exit(1)\n'
                              'if action == "pin" and sys.argv[-1] == "item-0" and not (state/"budget-probe").exists():\n'
                              '    (state/"budget-probe").touch(); print("Foundation: Pinned history is full (100 items or 32 MiB); unpin an item first\\ntrace ignored", file=sys.stderr); sys.exit(1)\n'
                              'entries=json.loads((state/"index.json").read_text())\n'
                              'if action in ("pin", "unpin"):\n'
                              '    item=next(item for item in entries if item["id"] == sys.argv[-1]); item["pinned"]=(action == "pin")\n'
                              '    entries.sort(key=lambda item: (not item["pinned"], int(item["id"].split("-")[1])))\n'
                              'elif action == "clear": entries=[item for item in entries if item["pinned"]]\n'
                              'elif action == "clear-all": entries=[]\n'
                              'if action != "init":\n'
                              '    candidate=state/"candidate.json"; candidate.write_text(json.dumps(entries)); candidate.replace(state/"index.json")\n')
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
                                    capture_output=True, text=True, timeout=16)
            log = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, log)
            self.assertNotIn('FOCUS_FAIL', log)
            self.assertIn('FOCUS_RESULT {"failures":0}', log)
            self.assertNotIn('TypeError', log)
            self.assertNotIn('ReferenceError', log)
            self.assertNotIn('Failed to load configuration', log)
            self.assertEqual((state / 'actions.log').read_text(), 'init\npin\nunpin\ncopy-plain\npin\npin\ncopy\nclear\nclear-all\n')
