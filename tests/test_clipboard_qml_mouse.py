"""Private Qt mouse and viewport regressions against production clipboard QML.

The fixture replaces only the layer-shell window and clipboard worker. QtTest
delivers native input to the actual row controls without accessing a desktop,
compositor, clipboard, or user's history.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from PIL import Image

from test_clipboard_qml_focus import window_transport

ROOT = Path(__file__).resolve().parents[1]

INSPECTION = r'''
    function fixtureRow(identity) {
        for (let i = 0; i < results.length; ++i) {
            const wrapper = list.itemAtIndex(i);
            if (wrapper && wrapper.modelData.id === identity)
                return wrapper.children.find(child => child.entry !== undefined);
        }
        return null;
    }
    function fixtureViewport() {
        const visible = [];
        for (let i = 0; i < results.length; ++i) {
            const wrapper = list.itemAtIndex(i);
            if (!wrapper) continue;
            const top = wrapper.y + (wrapper.groupGap || 0) - list.contentY;
            if (top + wrapper.height - (wrapper.groupGap || 0) > 0 && top < list.height)
                visible.push({id: wrapper.modelData.id, top: top, pinned: wrapper.modelData.pinned, gap: wrapper.groupGap || 0});
        }
        return {contentY: list.contentY, originY: list.originY, height: list.height,
                count: results.length, selected: selectedIdentity, rows: visible};
    }
    function fixtureSetViewport(y) { list.contentY = y; }
    function fixtureBusy() { return history.busy; }
'''

COMMON = r'''
import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import Quickshell.Io
import "shell/surfaces"
ShellRoot {
    id: fixture
    property int stage: 0
    property int failures: 0
    property var baseline: null
    property var lifecycle: ({clipboardEnabled: true, clipboardAlive: false})
    function check(condition, detail) {
        if (!condition) { failures++; console.error("CLIPBOARD_FAIL " + detail); }
    }
    function descendant(item, predicate) {
        if (predicate(item)) return item;
        for (const child of item.children || []) {
            const found = descendant(child, predicate);
            if (found) return found;
        }
        return null;
    }
    function external(action, identity) {
        updater.command = ["python3", Quickshell.env("DF_CLIPBOARD_WORKER"), "--state", Quickshell.env("DF_CLIPBOARD_STATE"), action, identity || ""];
        updater.running = true;
    }
    Process { id: updater }
    ClipboardFixture {
        id: clip
        width: 900; height: 850; visible: true
        lifecycle: fixture.lifecycle
        TestCase { id: input; name: "ClipboardMouse"; optional: true; when: false }
    }
    Timer { interval: 14000; running: true; onTriggered: {console.error("CLIPBOARD_FAIL timed out stage " + fixture.stage); Qt.quit();} }
    Timer {
        interval: 250; running: true; repeat: true
        onTriggered: {
            if (clip.snapshot().loading || clip.fixtureBusy() || updater.running) return;
            SCRIPT
        }
    }
}
'''

MOUSE_SCRIPT = r'''
            const order = ["item-0", "item-1", "item-2"];
            if (fixture.stage === 0) { clip.requestActivate(); clip.present(); fixture.stage++; return; }
            const step = fixture.stage - 1;
            if (step < 12) {
                const identity = order[Math.floor(step / 4)];
                const row = clip.fixtureRow(identity);
                fixture.check(!!row, "Target pin row missing: " + identity);
                if (!row) { Qt.quit(); return; }
                if (step % 4 === 0 || step % 4 === 2) {
                    const button = fixture.descendant(row, item => item.Accessible && ["Pin item", "Unpin item"].includes(item.Accessible.name));
                    fixture.check(!!button, "Native mouse pin button missing");
                    if (!button) { Qt.quit(); return; }
                    input.mouseClick(button, button.width / 2, button.height / 2);
                } else {
                    const expected = step % 4 === 3;
                    fixture.check(row.entry.pinned === expected, "Target did not toggle independently: " + identity);
                    fixture.check(clip.snapshot().selected === identity, "Pin reorder changed clicked row selection identity");
                    fixture.check(clip.snapshot().pinnedCount === (expected ? 3 : 2), "Toggling one row changed all pins");
                    for (const other of order.filter(id => id !== identity))
                        fixture.check(clip.fixtureRow(other)?.entry.pinned === true, "Unrelated pin changed: " + other);
                    const canvas = fixture.descendant(row, item => typeof item.requestPaint === "function" && item.width === 16 && item.height === 18);
                    fixture.check(!!canvas, "Pin canvas missing");
                    if (canvas) {
                        const destination = Quickshell.env("DF_CLIPBOARD_PAINT_DIR") + "/" + identity + "-" + (expected ? "pinned" : "unpinned") + ".png";
                        canvas.grabToImage(result => result.saveToFile(destination));
                    }
                    fixture.check(!fixture.descendant(row, item => item.Accessible?.name === "Delete item"), "Individual delete button remains visible in row");
                }
                fixture.stage++; return;
            }
            clip.setQuery("Fixture text 1");
            fixture.check(clip.snapshot().selected === "item-1", "Search selection identity fails after pin reorder");
            clip.setQuery("");
            fixture.stage++;
            console.log("CLIPBOARD_RESULT " + JSON.stringify({failures: fixture.failures}));
            Qt.quit();
'''

VIEWPORT_SCRIPT = r'''
            const view = clip.fixtureViewport();
            if (fixture.stage === 0) {
                clip.requestActivate(); clip.present(); clip.navigate(1);
                fixture.check(clip.snapshot().selected === "item-1", "Could not select first ordinary item");
                fixture.check(Math.abs(view.contentY - view.originY) < 1, "Fixture did not start at native origin");
                fixture.check(view.rows[1]?.gap === 12, "Pinned section lacks its dedicated spacing");
                fixture.external("insert", "incoming-top");
            } else if (fixture.stage === 1) {
                if (view.count !== 31) return;
                fixture.check(clip.snapshot().selected === "item-1", "External copy changed selected identity at top");
                fixture.check(Math.abs(view.contentY - view.originY) < 1, "External copy scrolled top viewport away from pinned section");
                fixture.check(view.rows[0]?.id === "item-0" && view.rows[0]?.pinned, "Pinned row became hidden after incoming copy");
                fixture.check(view.rows[1]?.gap === 12, "Incoming history item erased pinned section spacing");
                clip.navigate(10);
                clip.fixtureSetViewport(clip.fixtureViewport().contentY + 17);
            } else if (fixture.stage === 2) {
                fixture.baseline = clip.fixtureViewport();
                fixture.check(fixture.baseline.contentY > fixture.baseline.originY + 10, "Scrolled fixture failed to scroll");
                fixture.external("insert", "incoming-scroll");
            } else if (fixture.stage === 3) {
                if (view.count !== 32) return;
                fixture.check(view.selected === fixture.baseline.selected, "External copy changed selected ID when scrolled");
                const anchor = view.rows.find(row => row.id === fixture.baseline.rows[0].id);
                fixture.check(!!anchor && Math.abs(anchor.top - fixture.baseline.rows[0].top) < 1,
                              "First visible item/pixel offset jumps when a new copy arrives");
                clip.setQuery("Fixture text 20");
            } else if (fixture.stage === 4) {
                fixture.check(view.count === 1 && view.selected === "item-20", "Search result selection did not reset correctly");
                clip.setQuery("");
            } else if (fixture.stage === 5) {
                fixture.check(view.selected === "item-0", "Search cancellation does not select first pinned row");
                fixture.check(Math.abs(view.contentY - view.originY) < 1 && view.rows[0]?.id === "item-0", "Search cancellation failed to restore native top");
                clip.navigate(2);
                fixture.baseline = clip.snapshot().selected;
                input.keyClick(Qt.Key_Delete, Qt.ControlModifier);
            } else if (fixture.stage === 6) {
                if (view.count !== 31) return;
                fixture.check(view.selected !== fixture.baseline, "Keyboard delete retained a removed selected identity");
                fixture.check(clip.snapshot().pinnedCount === 1 && !clip.snapshot().closing, "Keyboard delete lost pin or closed surface");
                fixture.check(Math.abs(view.contentY - view.originY) < 1 && view.rows[0]?.id === "item-0", "Keyboard delete moved viewport away from pinned section");
                console.log("CLIPBOARD_RESULT " + JSON.stringify({failures: fixture.failures}));
                Qt.quit();
            }
            fixture.stage++;
'''

WORKER = r'''
import json, pathlib, sys
state = pathlib.Path(sys.argv[sys.argv.index("--state") + 1])
action = sys.argv[sys.argv.index("--state") + 2]
entries = json.loads((state / "index.json").read_text())
if action in ("pin", "unpin"):
    identity = sys.argv[-1]
    next(item for item in entries if item["id"] == identity)["pinned"] = action == "pin"
    entries.sort(key=lambda item: (not item["pinned"], item["rank"]))
elif action == "insert":
    entries.append(dict(id=sys.argv[-1], label="Incoming copy", preview="Incoming copy", kind="text", mime="text/plain", formats=["text/plain"], pinned=False, rank=-len(entries), size=30, image=""))
    entries.sort(key=lambda item: (not item["pinned"], item["rank"]))
elif action == "delete":
    entries = [item for item in entries if item["id"] != sys.argv[-1]]
elif action == "clear":
    entries = [item for item in entries if item["pinned"]]
if action != "init":
    candidate = state / "next.json"
    candidate.write_text(json.dumps(entries))
    candidate.replace(state / "index.json")
'''


@unittest.skipUnless(shutil.which('quickshell'), 'offscreen input requires Quickshell/QtTest')
class ClipboardMouseAndViewport(unittest.TestCase):
    def run_fixture(self, script, pinned=3):
        with tempfile.TemporaryDirectory(prefix='foundation-clipboard-mouse-') as directory:
            base = Path(directory)
            surfaces = base / 'shell/surfaces'; surfaces.mkdir(parents=True)
            production = window_transport((ROOT / 'shell/surfaces/Clipboard.qml').read_text())
            body, ending = production.rsplit('}', 1)
            (surfaces / 'ClipboardFixture.qml').write_text(body + INSPECTION + '}' + ending)
            for name in ('components', 'services', 'theme', 'assets'):
                (base / 'shell' / name).symlink_to(ROOT / 'shell' / name, target_is_directory=True)
            (base / 'shell.qml').write_text(COMMON.replace('SCRIPT', script))
            state = base / 'clipboard'; state.mkdir()
            paint = base / 'paint'; paint.mkdir()
            entries = [dict(id=f'item-{index}', label=f'Fixture text {index}', preview=f'Fixture text {index}',
                            mime='text/plain', kind='text', pinned=index < pinned, rank=index,
                            size=32, image='', formats=['text/plain']) for index in range(30)]
            (state / 'index.json').write_text(json.dumps(entries))
            worker = base / 'worker.py'; worker.write_text(WORKER)
            env = dict(os.environ)
            for name in ('WAYLAND_DISPLAY', 'DISPLAY', 'NIRI_SOCKET', 'HYPRLAND_INSTANCE_SIGNATURE',
                         'DBUS_SESSION_BUS_ADDRESS', 'DBUS_SYSTEM_BUS_ADDRESS', 'QS_CONFIG_PATH', 'QS_CONFIG_NAME'):
                env.pop(name, None)
            for name, leaf in (('HOME', 'home'), ('XDG_RUNTIME_DIR', 'runtime'),
                               ('XDG_CONFIG_HOME', 'config'), ('XDG_STATE_HOME', 'state'),
                               ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data')):
                private = base / leaf; private.mkdir(mode=0o700); env[name] = str(private)
            env.update(QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software', DF_FOUNDATION_ROOT=str(base),
                       DF_CLIPBOARD_WORKER=str(worker), DF_CLIPBOARD_STATE=str(state),
                       DF_CLIPBOARD_PAINT_DIR=str(paint), XDG_DATA_DIRS=str(base / 'data'))
            result = subprocess.run(['quickshell', '--path', str(base)], env=env,
                                    capture_output=True, text=True, timeout=18)
            log = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, log)
            self.assertNotIn('CLIPBOARD_FAIL', log)
            self.assertIn('CLIPBOARD_RESULT {"failures":0}', log)
            for error in ('TypeError', 'ReferenceError', 'Failed to load configuration'):
                self.assertNotIn(error, log)
            if script == MOUSE_SCRIPT:
                for identity in ('item-0', 'item-1', 'item-2'):
                    for pinned in (False, True):
                        with Image.open(paint / f'{identity}-{"pinned" if pinned else "unpinned"}.png') as image:
                            alpha = image.convert('RGBA').getpixel((8, 7))[3]
                            self.assertGreater(alpha, 200) if pinned else self.assertLess(alpha, 20)

    def test_first_middle_last_mouse_pin_toggles_only_one_item_and_repaints(self):
        self.run_fixture(MOUSE_SCRIPT)

    def test_external_copy_preserves_native_top_and_scrolled_anchor(self):
        self.run_fixture(VIEWPORT_SCRIPT, pinned=1)
