"""Production Power input and graphite layout in a private offscreen Window.

Only the layer-shell transport is replaced. The action worker is an inert fixture
that reports a deliberate error; no desktop, session or system action is invoked.
Real Qt frame signals validate delayed visibility and the fade/scale progression;
the offscreen host cannot prove physical layer-shell presentation or input delivery.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from test_clipboard_qml_focus import window_transport

ROOT = Path(__file__).resolve().parents[1]

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
    property bool shown: false
    property var entranceSamples: []
    property var lifecycle: ({powerEnabled: true})
    function check(condition, detail) {
        if (!condition) { failures++; console.error("POWER_FAIL " + detail); }
    }
    function find(node, name) {
        if (node.objectName === name) return node;
        for (const child of node.children || []) {
            const found = find(child, name);
            if (found) return found;
        }
        return null;
    }
    PowerFixture {
        id: power
        width: 900; height: 760; visible: false
        lifecycle: fixture.lifecycle
        TestCase {id: keyboard; name: "PowerInput"; optional: true; when: false}
    }
    // Rendering another window must not consume Power's entrance while its
    // own platform surface has yet to become visible.
    Window {
        width: 80; height: 80; visible: true
        Rectangle {anchors.fill: parent; color: "black"}
        FrameAnimation {running: !fixture.shown}
    }
    Connections {
        target: power
        function onFrameSwapped() {
            if (fixture.stage !== 0 || !fixture.shown) return;
            const card = power.contentItem.children.find(child => child.radius === 24);
            fixture.entranceSamples.push({opacity: card.opacity, scale: card.scale});
        }
    }
    Timer {
        interval: 250; running: true
        onTriggered: {
            const card = power.contentItem.children.find(child => child.radius === 24);
            fixture.check(!power.entered && card.opacity === 0 && card.scale === 0.97,
                          "Hidden surface consumed the entrance before its own first frame");
            fixture.shown = true;
            power.visible = true;
            power.requestActivate();
        }
    }
    Timer {
        interval: 200; running: true; repeat: true
        onTriggered: {
            if (!fixture.shown) return;
            const card = power.contentItem.children.find(child => child.radius === 24);
            if (fixture.stage === 0) {
                if (!power.entered || card.opacity !== 1 || card.scale !== 1) return;
                power.requestActivate();
                fixture.check(card && card.width === 640, "Power does not share the standard card width");
                fixture.check(card && card.opacity === 1 && card.scale === 1, "Entrance did not finish");
                fixture.check(fixture.entranceSamples.length > 2, "Entrance had no rendered intermediate frames");
                fixture.check(fixture.entranceSamples[0].opacity < 1 && fixture.entranceSamples[0].scale < 1,
                              "First rendered frame appeared at the final opacity/scale");
                fixture.check(fixture.entranceSamples.some(sample => sample.opacity > 0 && sample.opacity < 1
                              && sample.scale > 0.97 && sample.scale < 1),
                              "Rendered frames did not include the shared fade and scale transition");
                console.log("POWER_ENTRANCE " + JSON.stringify(fixture.entranceSamples));
                const list = card.children.find(child => child.model && child.count === 4);
                fixture.check(list && list.itemAtIndex(0).height === 62, "Power rows do not share the standard rhythm");
            } else if (fixture.stage === 1) {
                fixture.check(card.activeFocus, "Power has no actual Qt input focus");
                keyboard.keyClick(Qt.Key_Tab);
                fixture.check(power.selected === 1, "Tab does not reach the second action");
                keyboard.keyClick(Qt.Key_Tab, Qt.ShiftModifier);
                fixture.check(power.selected === 0, "Shift+Tab does not return to the first action");
                keyboard.keyClick(Qt.Key_End);
                fixture.check(power.selected === 3, "End does not reach the last action");
                keyboard.keyClick(Qt.Key_Home);
                keyboard.keyClick(Qt.Key_Down);
                fixture.check(power.selected === 1, "Arrow navigation is lost");
            } else if (fixture.stage === 2) {
                const directory = Quickshell.env("DF_QOL_PREVIEW_DIR");
                if (directory && !fixture.saved) {
                    fixture.saved = true;
                    card.grabToImage(result => result.saveToFile(directory + "/power.png"));
                    return;
                }
                keyboard.keyClick(Qt.Key_Space);
                fixture.check(power.busy, "Space did not invoke the inert worker");
                keyboard.keyClick(Qt.Key_Down);
                fixture.check(power.selected === 1, "Pending action did not block a second action");
            } else if (fixture.stage === 3) {
                if (power.busy) return;
                fixture.check(power.error === "Isolated action error", "Action failure is not visible");
                fixture.check(fixture.lifecycle.powerEnabled, "Failed action closed the surface");
                keyboard.keyClick(Qt.Key_Escape);
                fixture.check(power.closing && fixture.lifecycle.powerEnabled, "Escape bypasses the shared exit animation");
                keyboard.keyClick(Qt.Key_Return);
            } else if (fixture.stage === 4) {
                fixture.check(!fixture.lifecycle.powerEnabled, "Exit animation did not close the surface");
                // Re-present only the offscreen host to check the same mouse escape hint.
                power.closing = false;
                fixture.lifecycle.powerEnabled = true;
                card.forceActiveFocus();
            } else if (fixture.stage === 5) {
                const escape = fixture.find(card, "surface-footer-escape");
                fixture.check(escape !== null, "Shared escape hint is missing");
                keyboard.mouseClick(escape, escape.width / 2, escape.height / 2, Qt.LeftButton);
                fixture.check(power.closing && fixture.lifecycle.powerEnabled, "Mouse escape lost the animated close action");
            } else if (fixture.stage === 6) {
                fixture.check(!fixture.lifecycle.powerEnabled, "Mouse exit did not complete");
                console.log("POWER_RESULT " + JSON.stringify({failures: fixture.failures}));
                Qt.quit();
            }
            fixture.stage++;
        }
    }
    Timer {interval: 5000; running: true; onTriggered: {console.error("POWER_FAIL fixture timed out"); Qt.quit();}}
}
'''


@unittest.skipUnless(shutil.which("quickshell"), "offscreen input requires Quickshell/QtTest")
class PowerFocus(unittest.TestCase):
    def test_keyboard_focus_action_error_and_animated_exit(self):
        with tempfile.TemporaryDirectory(prefix="foundation-power-focus-") as directory:
            base = Path(directory)
            surface = base / "shell/surfaces"; surface.mkdir(parents=True)
            (surface / "PowerFixture.qml").write_text(window_transport((ROOT / "shell/surfaces/Power.qml").read_text()))
            for name in ("components", "services", "theme", "assets"):
                (base / "shell" / name).symlink_to(ROOT / "shell" / name, target_is_directory=True)
            (base / "shell.qml").write_text(QML)
            scripts = base / "scripts"; scripts.mkdir()
            worker = scripts / "power-action"
            worker.write_text("#!" + sys.executable + "\n" +
                              "import pathlib,sys\n" +
                              "pathlib.Path(__file__).with_name('actions.log').write_text(sys.argv[1] + '\\n')\n" +
                              "print('Isolated action error', file=sys.stderr)\n" +
                              "sys.exit(1)\n")
            worker.chmod(0o700)
            env = dict(os.environ)
            for name in ("WAYLAND_DISPLAY", "DISPLAY", "NIRI_SOCKET", "HYPRLAND_INSTANCE_SIGNATURE",
                         "DBUS_SESSION_BUS_ADDRESS", "DBUS_SYSTEM_BUS_ADDRESS", "QS_CONFIG_PATH", "QS_CONFIG_NAME"):
                env.pop(name, None)
            for name, leaf in (("HOME", "home"), ("XDG_RUNTIME_DIR", "runtime"),
                               ("XDG_CONFIG_HOME", "config"), ("XDG_STATE_HOME", "state"),
                               ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data")):
                private = base / leaf; private.mkdir(mode=0o700); env[name] = str(private)
            env.update(QT_QPA_PLATFORM="offscreen", QT_QUICK_BACKEND="software",
                       DF_FOUNDATION_ROOT=str(base), XDG_DATA_DIRS=str(base / "data"))
            result = subprocess.run(["quickshell", "--path", str(base)], env=env,
                                    capture_output=True, text=True, timeout=8)
            log = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, log)
            self.assertNotIn("POWER_FAIL", log)
            self.assertIn('POWER_RESULT {"failures":0}', log)
            self.assertNotIn("TypeError", log)
            self.assertNotIn("ReferenceError", log)
            self.assertNotIn("Failed to load configuration", log)
            self.assertEqual((scripts / "actions.log").read_text(), "logout\n")
