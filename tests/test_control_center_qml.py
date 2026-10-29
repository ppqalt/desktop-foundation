"""Production controls and Hermite motion in a private offscreen Qt window."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
ROOT=Path(__file__).resolve().parents[1]

QML=r'''
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
    property bool enabled: true
    property bool concurrentSent: false
    property int count: 0
    property real velocity: 0
    function check(ok, why) { if (!ok) { failures++; console.error("CONTROL_FAIL "+why); } }
    function find(item, name) {
        if (item.objectName === name) return item;
        for (const child of item.children || []) { const found=find(child,name); if (found) return found; }
        return null;
    }
    readonly property var panel: loader.item
    LazyLoader {
        id: loader
        active: fixture.enabled
        ControlCenterFixture {
            width: 720; height: 1000
            sidebarStyle: ({width:720,padding:12,radius:14,opacity:0.75,openingMs:240,closingMs:180})
            onClosed: fixture.enabled=false
            TestCase {id: keys; name:"ControlKeys"; optional:true; when:false}
            function key(key, modifiers) { keys.keyClick(key, modifiers || Qt.NoModifier); }
        }
    }
    Timer {
        id: test
        interval: 30; running:true; repeat:true
        onTriggered: {
            if (fixture.stage===0) {
                if (panel.snapshot().progress>0 && panel.snapshot().progress<1) {
                    fixture.check(panel.width===720 && panel.height===1000,"sidebar dimensions changed");
                    fixture.stage=1;
                }
            } else if (fixture.stage===1) {
                if (panel.snapshot().moving || !panel.stateData.audio) return;
                panel.requestActivate();
                panel.navigate("audio");
                find(panel.contentItem,"outputChoice").forceActiveFocus();
                fixture.check(find(panel.contentItem,"outputChoice").activeFocus,"output choice not focusable");
                const card=panel.contentItem.children.find(c=>c.topRightRadius===14);
                fixture.check(card && card.bottomRightRadius===14 && card.topLeftRadius===0 && Math.abs(card.color.a-.75)<.01,"mirrored glass/corners differ");
                panel.key(Qt.Key_Tab);
                fixture.check(panel.activeFocusItem.Accessible.name.indexOf("Volume")>=0,"Tab misses output mute");
                panel.key(Qt.Key_Tab);
                const slider=find(panel.contentItem,"outputVolume");
                fixture.check(slider.activeFocus,"Tab misses output volume");
                panel.key(Qt.Key_Right);
                fixture.check(slider.value===51,"volume does not update immediately");
                panel.key(Qt.Key_A);
                fixture.check(slider.value===50,"A does not decrease volume immediately");
                panel.key(Qt.Key_D);
                fixture.check(slider.value===51,"D does not increase volume immediately");
                panel.key(Qt.Key_W);
                fixture.check(find(panel.contentItem,"outputChoice").activeFocus,"W does not navigate upward from slider");
                panel.key(Qt.Key_S);
                fixture.check(slider.activeFocus,"S does not navigate down to slider");
                fixture.stage=2;
            } else if (fixture.stage===2) {
                if (panel.busy || panel.queue.length) return;
                if (!fixture.concurrentSent) {
                    fixture.concurrentSent=true;
                    panel.enqueue(["wifi","on"]);
                    panel.enqueue(["volume","output","52"]);
                    return;
                }
                panel.dismiss(); fixture.stage=3;
            } else if (fixture.stage===3) {
                if (!panel || panel.snapshot().progress===0) { fixture.check(false,"exit finished too early");fixture.stage=7;return; }
                fixture.check(panel.closing && panel.snapshot().moving,"exit has no animation");
                const motion=panel.contentItem; // Reverse the real production controller.
                panel.present(); fixture.stage=4;
            } else if (fixture.stage===4) {
                if (panel.snapshot().moving) return;
                fixture.check(!panel.closing && panel.snapshot().progress===1,"reversal did not settle open");
                panel.navigate("overview");
                fixture.stage=8;
                captureDelay.restart();
            } else if (fixture.stage===9) {
                fixture.stage=5;
                const details=find(panel.contentItem,"audioDetails");
                details.forceActiveFocus();
                panel.key(Qt.Key_D);
                fixture.check(find(panel.contentItem,"inputDetails").activeFocus,"D does not navigate across audio cards");
                panel.key(Qt.Key_A);
                fixture.check(details.activeFocus,"A does not navigate back across audio cards");
                panel.key(Qt.Key_Return);
                fixture.check(panel.page==="audio","Enter does not open audio details");
                panel.key(Qt.Key_Escape);
                fixture.check(panel.page==="overview" && details.activeFocus,"Back does not restore initiating focus");
                // Walk the overview focus chain, including both connectivity modules.
                let sawNetwork=false, sawMic=false;
                for(let i=0;i<30;i++) {
                    panel.key(Qt.Key_Tab);
                    const label=panel.activeFocusItem?.Accessible.name || "";
                    if (!sawNetwork && label.indexOf("Networking")>=0) panel.key(Qt.Key_Return);
                    sawNetwork=sawNetwork || label.indexOf("Networking")>=0;
                    sawMic=sawMic || label.indexOf("Microphone")>=0;
                }
                fixture.check(sawNetwork && sawMic,"keyboard cannot reach microphone/network controls");
                panel.navigate("audio");
                const choice=find(panel.contentItem,"outputChoice");
                choice.forceActiveFocus();
                panel.key(Qt.Key_Space);
                fixture.check(choice.popup.visible,"keyboard cannot open device dropdown");
                panel.key(Qt.Key_Escape);
                fixture.check(!choice.popup.visible && !panel.closing,"Escape does not close dropdown first");
                panel.key(Qt.Key_Space);
                panel.key(Qt.Key_S);
                panel.key(Qt.Key_Space);
                fixture.check(!choice.popup.visible,"Enter does not accept the device choice");
                panel.key(Qt.Key_Escape);
                fixture.check(panel.page==="overview" && !panel.closing,"Escape does not return to overview");
                panel.key(Qt.Key_Escape);
            } else if (fixture.stage===5) {
                if (panel) return;
                fixture.stage=6;
                idle.restart();
                test.stop();
            }
        }
    }
    Timer {
        id: captureDelay
        interval:80
        onTriggered: {
            const directory=Quickshell.env("DF_CONTROL_CAPTURE");
            if (directory) panel.contentItem.grabToImage(result=>{result.saveToFile(directory);fixture.stage=9;});
            else fixture.stage=9;
        }
    }
    Timer {
        id: idle
        interval:2500
        onTriggered: {
            fixture.check(!fixture.panel,"closed surface remained resident");
            console.log("CONTROL_RESULT "+JSON.stringify({failures:fixture.failures}));
            Qt.quit();
        }
    }
    Timer {interval:6500;running:true;onTriggered:{console.error("CONTROL_TIMEOUT stage="+fixture.stage);Qt.quit();}}
}
'''

@unittest.skipUnless(shutil.which('quickshell'),'requires Quickshell')
class ControlCenterQml(unittest.TestCase):
    def test_keyboard_motion_and_lazy_unload(self):
        with tempfile.TemporaryDirectory(prefix='foundation-control-') as directory:
            base=Path(directory);surfaces=base/'shell/surfaces';surfaces.mkdir(parents=True)
            source=(ROOT/'shell/surfaces/ControlCenter.qml').read_text()
            source=source.replace('PanelWindow {','Window {',1)
            source=re.sub(r'    anchors \{\n        top: true\n        bottom: true\n        left: true\n    \}\n','',source,count=1)
            source='\n'.join(line for line in source.splitlines() if not line.startswith(('    screen:','    implicitWidth:','    margins.left:','    exclusionMode:','    WlrLayershell.')))+'\n'
            (surfaces/'ControlCenterFixture.qml').write_text(source)
            for name in ['components','theme']:(base/'shell'/name).symlink_to(ROOT/'shell'/name,target_is_directory=True)
            (base/'shell.qml').write_text(QML)
            (base/'theme').symlink_to(ROOT/'theme',target_is_directory=True)
            worker=base/'native/foundation/target/release/desktop-foundationctl';worker.parent.mkdir(parents=True)
            data={'audio':{'available':True,'output':'speakers','input':'mic','outputs':[{'name':'speakers','description':'Speakers','volume':50,'muted':False},{'name':'headphones','description':'Headphones','volume':40,'muted':False}], 'inputs':[{'name':'mic','description':'Microphone','volume':33,'muted':False}]},'network':{'available':True,'enabled':True,'wifi':True,'state':'connected','devices':[{'name':'enp4s0','kind':'ethernet','connection':'Office','state':'connected'}],'connections':[]}}
            (base/'snapshot.json').write_text(json.dumps(data))
            worker.write_text('#!'+sys.executable+'\nimport json,sys,pathlib,time\nroot=pathlib.Path(__file__).resolve().parents[4]\nargs=sys.argv[4:]\nwith (root/"calls").open("a") as f:f.write(json.dumps(args)+"\\n")\nif args==["wifi","on"]:\n    time.sleep(.6)\n    with (root/"calls").open("a") as f:f.write(json.dumps(["done",*args])+"\\n")\nprint((root/"snapshot.json").read_text() if args==["snapshot"] else "{\\"success\\":true}")\n')
            worker.chmod(0o700)
            env=dict(os.environ)
            for name in ['WAYLAND_DISPLAY','DISPLAY','NIRI_SOCKET','HYPRLAND_INSTANCE_SIGNATURE','DBUS_SESSION_BUS_ADDRESS','DBUS_SYSTEM_BUS_ADDRESS','QS_CONFIG_PATH','QS_CONFIG_NAME']:env.pop(name,None)
            for name,leaf in [('HOME','home'),('XDG_RUNTIME_DIR','runtime'),('XDG_CONFIG_HOME','config'),('XDG_STATE_HOME','state'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data')]:
                path=base/leaf;path.mkdir(mode=0o700);env[name]=str(path)
            if os.environ.get('DF_CONTROL_PALETTE'):
                palette_path=base/'state/desktop-foundation/theme/current/semantic.json'
                palette_path.parent.mkdir(parents=True)
                palette_path.write_text(Path(os.environ['DF_CONTROL_PALETTE']).read_text())
            env.update(QT_QPA_PLATFORM='offscreen',QT_QUICK_BACKEND='software',DF_FOUNDATION_ROOT=str(base),XDG_DATA_DIRS=str(base/'data'))
            r=subprocess.run(['quickshell','--path',str(base)],env=env,capture_output=True,text=True,timeout=9)
            log=r.stdout+r.stderr
            self.assertEqual(r.returncode,0,log)
            self.assertIn('CONTROL_RESULT {"failures":0}',log)
            self.assertNotIn('CONTROL_FAIL',log);self.assertNotIn('TypeError',log);self.assertNotIn('ReferenceError',log)
            calls=[json.loads(line) for line in (base/'calls').read_text().splitlines()]
            self.assertIn(['volume','output','51'],calls)
            self.assertIn(['networking','off'],calls)
            self.assertIn(['select','output','headphones'],calls)
            self.assertLess(calls.index(['volume','output','52']),calls.index(['done','wifi','on']),'network request blocked volume')
            self.assertLessEqual(sum(call==['snapshot'] for call in calls),3,'worker kept polling after close')

if __name__=='__main__':unittest.main()
