"""Actual instant bar updates and visibility fades in private offscreen Qt.

Calls only the QML presenter. No audio command, real shell or device is used.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]

QML=r'''
import QtQuick
import QtQuick.Window
import Quickshell
import "shell/surfaces"
ShellRoot {
    id: fixture
    property int stage: 0
    property int failures: 0
    property int swaps: 0
    property real wanted: -1
    function check(value,detail) { if(!value) { failures++;console.error("VOLUME_FAIL "+detail); } }
    function find(item,name) {
        if(item.objectName===name)return item;
        for(const child of item.children || []) {const result=find(child,name);if(result)return result;}
        return null;
    }
    readonly property var card: find(volume.contentItem,"volume-card")
    readonly property var bar: find(volume.contentItem,"volume-progress")
    Window {width:100;height:100;visible:true;color:"#171b22"}
    VolumeFixture {id:volume}
    Connections {
        target: volume
        function onFrameSwapped() {
            if(fixture.swaps===0 && volume.visible) {
                fixture.check(fixture.bar.width===Math.round(fixture.bar.parent.width*.8),"First bar value animated from zero");
                fixture.check(fixture.card.opacity===0,"Visibility fade completed before its first frame");
            }
            if(volume.visible)fixture.swaps++;
        }
    }
    Timer {
        interval:100;running:true;repeat:true
        onTriggered: {
            if(fixture.stage===0) {
                fixture.check(!volume.visible && !volume.shown,"Widget starts visible");
                volume.present(80,false);
                fixture.check(volume.awaitingFrame && !volume.shown,"Fade starts before a frame");
            } else if(fixture.stage===1) {
                fixture.check(fixture.swaps>0 && volume.shown && !volume.awaitingFrame,"Volume never presents");
                fixture.wanted=Math.round(fixture.bar.parent.width*.5);
                volume.present(50,false);
                fixture.check(volume.level===50,"Percentage does not update immediately");
                fixture.check(fixture.bar.width===fixture.wanted,"Bar does not update immediately");
            } else if(fixture.stage===2) {
                fixture.check(fixture.card.opacity===1,"Visibility fade does not finish");
                fixture.check(fixture.bar.width===fixture.wanted,"Bar target differs from readout");
                fixture.wanted=-1;volume.present(45,true);
                fixture.check(fixture.bar.width===0,"Mute bar does not update immediately");
            } else if(fixture.stage===3) {
                fixture.check(volume.muted && volume.level===45 && fixture.bar.width===0,"Muted readout/bar is wrong");
                volume.present(999,false);fixture.check(volume.level===100,"Display cap was lost");
            } else if(fixture.stage===4) {
                fixture.check(fixture.bar.width===fixture.bar.parent.width,"Full level bar does not finish");
            } else if(fixture.stage===20) {
                fixture.check(!volume.visible && !volume.shown && !volume.awaitingFrame,"Expiry does not hide widget");
                volume.present(25,false);
                fixture.check(fixture.bar.width===Math.round(fixture.bar.parent.width*.25),"Reopened bar starts with stale width");
                fixture.check(volume.awaitingFrame && !volume.shown,"Reopened widget lacks visibility fade gate");
                // Simulate a host kept unmapped until its notification expires.
                volume.visible=false;
            } else if(fixture.stage===37) {
                fixture.check(!volume.visible && !volume.shown && !volume.awaitingFrame,"Unmapped expired widget reappears");
                console.log("VOLUME_RESULT "+JSON.stringify({failures:fixture.failures}));
                Qt.quit();
            }
            fixture.stage++;
        }
    }
    Timer {interval:6000;running:true;onTriggered:{console.error("VOLUME_FAIL timeout");Qt.quit();}}
}
'''


@unittest.skipUnless(shutil.which('quickshell'),'volume fixture requires Quickshell')
class VolumeFeedback(unittest.TestCase):
    def test_first_value_rapid_update_mute_and_expiry(self):
        with tempfile.TemporaryDirectory(prefix='foundation-volume-feedback-') as directory:
            base=Path(directory);surfaces=base/'shell/surfaces';surfaces.mkdir(parents=True)
            source=(ROOT/'shell/surfaces/Volume.qml').read_text().replace('PanelWindow {','Window {',1)
            source='\n'.join(line for line in source.splitlines() if not line.startswith(('    screen:','    anchors.top:','    margins.top:','    exclusionMode:','    WlrLayershell.','    mask:')))+'\n'
            source=source.replace('    implicitWidth:','    width:').replace('    implicitHeight:','    height:')
            (surfaces/'VolumeFixture.qml').write_text(source)
            (base/'shell/theme').symlink_to(ROOT/'shell/theme',target_is_directory=True)
            (base/'shell.qml').write_text(QML)
            env=dict(os.environ)
            for name in ('WAYLAND_DISPLAY','DISPLAY','NIRI_SOCKET','HYPRLAND_INSTANCE_SIGNATURE','QS_CONFIG_PATH','QS_CONFIG_NAME'):
                env.pop(name,None)
            for name,leaf in [('HOME','home'),('XDG_RUNTIME_DIR','runtime'),('XDG_STATE_HOME','state'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data'),('XDG_CONFIG_HOME','config')]:
                path=base/leaf;path.mkdir(mode=0o700);env[name]=str(path)
            env.update(QT_QPA_PLATFORM='offscreen',QT_QUICK_BACKEND='software',
                       DBUS_SESSION_BUS_ADDRESS='unix:path='+str(base/'absent-user-bus'),
                       DBUS_SYSTEM_BUS_ADDRESS='unix:path='+str(base/'absent-system-bus'))
            result=subprocess.run(['quickshell','--path',str(base)],env=env,capture_output=True,text=True,timeout=9)
            log=result.stdout+result.stderr
            self.assertEqual(result.returncode,0,log)
            self.assertNotIn('VOLUME_FAIL',log)
            self.assertIn('VOLUME_RESULT {"failures":0',log)
            self.assertNotIn('TypeError',log)
            self.assertNotIn('ReferenceError',log)
            self.assertNotIn('Failed to load configuration',log)
