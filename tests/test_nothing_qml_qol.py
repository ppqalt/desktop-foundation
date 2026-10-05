"""Actual earbud QML in an offscreen Window with inert backend executables.

No PanelWindow, compositor, system bus, radio, audio or real user state is used.
QtTest sends ordinary key events through the focused Qt window and the actual
Keys handler. Synthetic auto-repeat and backend responses are injected into the
same policy functions; real layer-shell focus remains outside this fixture.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

QML = r'''
import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "app/surfaces"
import "app/components"
ShellRoot {
    id: fixture
    property int stage: 0
    property int failures: 0
    property int backCount: 0
    property real initialHeight: 0
    property bool visualPending: false
    property var state: ({model: "Fixture earbuds", modelCode: "B173", firmware: "test",
        anc: 5, bass: {enabled: false, level: 3}, eq: 0, customEq: [0, 0, 0],
        inEar: false, latency: false, dual: false, personal: false, superMic: false,
        autoTransparency: false, spatial: 0, quality: 2, gestures: [{device: 2, kind: 2, action: 8}],
        findAvailable: true, fitAvailable: true,
        battery: {left: {percent: 80, charging: false}, right: {percent: 75, charging: false}}})
    function check(condition, detail) {
        if (!condition) { failures++; console.error("QOL_FAIL " + detail); }
    }
    function key(name, modifiers = 0, repeated = false) {
        if (repeated) {
            const event = {key: name, modifiers: modifiers, isAutoRepeat: true, accepted: false};
            controls.handleKey(event);
            return event.accepted;
        }
        keyboard.keyClick(name, modifiers, 0);
        return true;
    }
    function select(keyName) {
        controls.selected = controls.rows.findIndex(row => row.key === keyName);
        check(controls.selected >= 0, "Missing row " + keyName);
    }
    function copy(value) { return JSON.parse(JSON.stringify(value)); }
    function capture(name, complete) {
        const directory = Quickshell.env("DF_QOL_VISUAL_DIR");
        if (!directory) { if (complete) complete(); return; }
        visualPending = true;
        const started = controls.grabToImage(result => {
            fixture.check(result.saveToFile(directory + "/nothing-" + name + ".png"), "Offscreen capture failed: " + name);
            fixture.visualPending = false;
            if (complete) complete();
        });
        if (!started) {
            check(false, "Offscreen capture could not start: " + name);
            visualPending = false;
            if (complete) complete();
        }
    }
    Window {
        id: window
        width: 900; height: 760; visible: true; color: "#171b22"
        TestCase { id: keyboard; optional: true; when: false }
        NothingControls {
            id: controls
            anchors.centerIn: parent
            device: ({name: "Fixture earbuds", address: "AB:CD:EF:01:02:03",
                      dbusPath: "/org/bluez/hci7/dev_AB_CD_EF_01_02_03"})
            availableWidth: window.width; availableHeight: window.height
            audio: "LDAC"
            onBack: fixture.backCount++
            onFailed: message => { fixture.failures++; console.error("QOL_FAIL " + message); }
        }
        ApplicationRow {
            id: sample
            visible: false
            width: 600; height: 62
            entry: ({name: "Low latency", genericName: "Reported state", icon: ""})
            selected: false; settingState: false
        }
    }
    Timer {
        interval: 100; repeat: true; running: true
        onTriggered: {
            if (fixture.visualPending)
                return;
            if (fixture.stage === 0) {
                fixture.initialHeight = controls.height;
                controls.handleBackendMessage({event: "identified", state: {model: "Fixture earbuds", inEar: true}});
                controls.handleBackendMessage({event: "state", state: fixture.state});
                fixture.check(!controls.ready && controls.rows.length === 0, "Startup exposes partial settings");
                fixture.check(controls.earState.inEar === undefined, "Partial state published before ready");
                fixture.check(controls.height === fixture.initialHeight, "Loading card changes geometry");
                fixture.capture("loading", () => controls.handleBackendMessage({event: "ready", state: fixture.state}));
            } else if (fixture.stage === 1) {
                fixture.check(controls.ready && controls.rows.length > 12, "Ready settings not revealed");
                fixture.check(controls.height === fixture.initialHeight, "Ready reveal changes geometry");
                fixture.check(controls.activeFocus, "Controls lack item focus in offscreen Window");
                fixture.check(controls.rows.find(row => row.key === "inEar").settingState === false, "Off toggle lacks reported state");
                fixture.key(Qt.Key_End);
                fixture.check(controls.rows[controls.selected].key === "disconnect", "End misses final action");
                fixture.key(Qt.Key_Left); fixture.key(Qt.Key_Right);
                fixture.check(!controls.leaving && !controls.busy, "Adjust key activates disconnect");
                fixture.key(Qt.Key_Home);
                fixture.check(controls.rows[controls.selected].key === "anc", "Home misses first setting");
                fixture.key(Qt.Key_PageDown); fixture.check(controls.selected > 0, "PageDown does not navigate");
                fixture.key(Qt.Key_PageUp); fixture.check(controls.selected === 0, "PageUp does not return");
                fixture.key(Qt.Key_Tab); fixture.key(Qt.Key_Backtab);
                fixture.check(controls.selected === 0, "Backtab does not reverse Tab");
                fixture.key(Qt.Key_Tab); fixture.key(Qt.Key_Tab, Qt.ShiftModifier);
                fixture.check(controls.selected === 0, "Shift+Tab does not reverse Tab");
                fixture.key(Qt.Key_Down); fixture.key(Qt.Key_Up);
                fixture.check(controls.selected === 0, "Up/Down key delivery does not navigate");
                fixture.select("inEar");
                fixture.key(Qt.Key_Right, 0, true);
                fixture.check(!controls.busy, "Held adjustment repeats writes");
                fixture.key(Qt.Key_Right);
                fixture.check(controls.busy && controls.pendingRowId === "inEar:", "Toggle has no pending state");
                fixture.check(controls.earState.inEar === false, "Toggle updates optimistically");
                fixture.check(controls.confirmedRowId === "", "Write highlighted before readback");
                const actual = fixture.copy(fixture.state); actual.inEar = true;
                controls.handleBackendMessage({event: "state", state: actual});
                fixture.check(controls.earState.inEar === false, "Busy response causes partial visual update");
                controls.handleBackendMessage({event: "complete", success: true});
                fixture.check(controls.earState.inEar === true && controls.confirmedRowId === "inEar:", "Confirmed toggle not visible");
                fixture.check(!controls.busy && controls.pendingRowId === "", "Pending state survives completion");
                fixture.check(controls.rows[controls.selected].key === "inEar", "Readback steals selection");
            } else if (fixture.stage === 2) {
                fixture.select("latency"); fixture.key(Qt.Key_Space);
                fixture.check(controls.busy, "Space does not activate toggle");
                const actual = fixture.copy(controls.earState); actual.latency = false;
                controls.handleBackendMessage({event: "state", state: actual});
                controls.handleBackendMessage({event: "error", message: "Fixture rejected"});
                fixture.check(!controls.busy && !controls.earState.latency, "Error hides actual reported state");
                fixture.check(controls.confirmedRowId === "" && controls.error === "Fixture rejected", "Error reports saved toggle");
                fixture.select("latency"); fixture.key(Qt.Key_Return);
                controls.handleBackendMessage({event: "complete", success: true});
                fixture.check(controls.confirmedRowId === "", "Acknowledgement without readback treated as confirmed");
                controls.error = "";
                controls.playbackCodecs = ["sbc", "sbc_xq"];
                controls.page = "quality";
                fixture.select("sbc_xq");
                fixture.key(Qt.Key_Left); fixture.key(Qt.Key_Right);
                fixture.check(!controls.busy, "Adjust key starts codec action");
                controls.pendingRowId = "sbc_xq:"; controls.requestedPlaybackCodec = "sbc_xq";
                controls.busy = true;
                controls.handlePlaybackResult({success: true, routed: true, codec: "SBC XQ"});
                controls.finishPlayback(0);
                fixture.check(controls.playbackCodec === "sbc_xq" && controls.confirmedRowId === "sbc_xq:", "Verified host codec not reflected");
                fixture.check(controls.rows.find(row => row.key === "sbc_xq").settingValue === "Selected", "Host codec selected badge missing");
                fixture.check(controls.rows.find(row => row.key === "quality" && row.extra === 2).settingValue === "", "Firmware preference also shown selected");
                controls.audio = "AAC";
                fixture.check(controls.playbackCodec === "aac", "Local readback overrides later native codec");
                controls.page = "main";
                fixture.select("info"); fixture.key(Qt.Key_Right);
                fixture.check(controls.page === "main", "Right opens subpage");
                fixture.key(Qt.Key_Return);
                fixture.check(controls.page === "info" && controls.rows[controls.selected].key === "none", "Information page skips keyboard-readable details");
                fixture.key(Qt.Key_End);
                fixture.check(controls.rows[controls.selected].key === "back", "Information page has no keyboard Back");
                fixture.key(Qt.Key_Home);
                fixture.check(controls.selected === 0, "Home cannot return to informational rows");
                fixture.key(Qt.Key_Escape, 0, true);
                fixture.check(controls.page === "info", "Held Escape exits a page");
                fixture.key(Qt.Key_Escape);
                fixture.check(controls.page === "main" && controls.rows[controls.selected].key === "info", "Escape loses parent selection");
                sample.settingState = true; sample.settingValue = "Level 3";
                sample.pending = true;
                fixture.check(sample.hasSettingState && sample.hasSetting && sample.pending, "Shared row loses switch during pending request");
                sample.pending = false; sample.confirmed = true;
                fixture.check(sample.border.width === 1, "Confirmed shared row has no visible border");
            } else if (fixture.stage === 16) {
                controls.navigateEdge(false);
            } else if (fixture.stage === 17) {
                fixture.check(controls.confirmedRowId === "", "Confirmed highlight stays resident");
                fixture.capture("ready");
            } else if (fixture.stage === 18) {
                controls.ready = false;
                fixture.key(Qt.Key_Escape);
                fixture.check(controls.leaving, "Escape cannot cancel startup");
            } else if (fixture.stage === 20) {
                fixture.check(fixture.backCount > 0, "Backend shutdown does not return");
                console.log("QOL_RESULT " + JSON.stringify({failures: fixture.failures}));
                Qt.quit();
            }
            fixture.stage++;
        }
    }
    Timer { interval: 7000; running: true; onTriggered: { console.error("QOL_FAIL fixture timed out"); Qt.quit(); } }
}
'''


@unittest.skipUnless(shutil.which('quickshell'), 'offscreen fixture requires Quickshell')
class NothingSurfaceQol(unittest.TestCase):
    def test_actual_qml_stages_loading_and_reports_confirmed_settings(self):
        with tempfile.TemporaryDirectory(prefix='foundation-qol-qml-') as directory:
            base = Path(directory)
            (base / 'app').symlink_to(ROOT / 'shell', target_is_directory=True)
            scripts = base / 'scripts'; scripts.mkdir()
            backend = scripts / 'nothing-backend'
            backend.write_text('#!' + sys.executable + '\nimport sys\nfor line in sys.stdin: pass\n')
            backend.chmod(0o755)
            native = base / 'native/foundation/target/release'; native.mkdir(parents=True)
            command = native / 'desktop-foundationctl'
            command.write_text('#!' + sys.executable + '\nimport json\nprint(json.dumps({"success":True,"codecs":["sbc","sbc_xq"]}))\n')
            command.chmod(0o755)
            (base / 'shell.qml').write_text(QML)
            env = dict(os.environ)
            for name in ('WAYLAND_DISPLAY', 'DISPLAY', 'NIRI_SOCKET', 'HYPRLAND_INSTANCE_SIGNATURE',
                         'DBUS_SESSION_BUS_ADDRESS', 'DBUS_SYSTEM_BUS_ADDRESS', 'QS_CONFIG_PATH', 'QS_CONFIG_NAME'):
                env.pop(name, None)
            for name, leaf in (('HOME', 'home'), ('XDG_RUNTIME_DIR', 'runtime'),
                               ('XDG_CONFIG_HOME', 'config'), ('XDG_STATE_HOME', 'state'),
                               ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data')):
                private = base / leaf; private.mkdir(mode=0o700); env[name] = str(private)
            env.update(QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                       DF_FOUNDATION_ROOT=str(base), XDG_DATA_DIRS=str(base / 'data'))
            if env.get('DF_QOL_VISUAL_DIR'):
                Path(env['DF_QOL_VISUAL_DIR']).mkdir(parents=True, exist_ok=True)
            result = subprocess.run(['quickshell', '--path', str(base)], env=env,
                                    capture_output=True, text=True, timeout=12)
            log = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, log)
            self.assertNotIn('QOL_FAIL', log)
            self.assertIn('QOL_RESULT {"failures":0}', log)
            self.assertNotIn('TypeError', log)
            self.assertNotIn('ReferenceError', log)
            self.assertNotIn('Failed to load configuration', log)
            if env.get('DF_QOL_VISUAL_DIR'):
                for name in ('loading', 'ready'):
                    self.assertTrue((Path(env['DF_QOL_VISUAL_DIR']) / f'nothing-{name}.png').is_file(), log)
