"""Actual shell policy and source reload in private, inert hosts only."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

from test_surface_keyboard import block

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT/'shell/shell.qml').read_text()


def function(name):
    signature=re.search(r'function '+name+r'\((.*?)\)\s*:\s*(\w+)\s*\{',SOURCE)
    if signature is None: raise AssertionError('Missing shell function '+name)
    return signature.group(1),signature.group(2),block(SOURCE,'function '+name+'(')


@unittest.skipUnless(shutil.which('node'),'policy fixture requires Node')
class MenuLifecycle(unittest.TestCase):
    def test_shortcut_and_cross_menu_closes_use_animation_and_keep_busy_action(self):
        names=['dismissPower','togglePower','toggleBluetooth','toggleClipboard','showClipboard','toggleLauncher','showLauncher','reloadConfiguration']
        functions='\n'.join('function '+name+'() {'+function(name)[2]+'}' for name in names)
        script=r'''
const vm=require('node:vm');const assert=require('node:assert/strict');
const events=[];const callbacks=[];
const context={powerEnabled:true,launcherEnabled:false,clipboardEnabled:false,bluetoothEnabled:false,
    powerLoader:{item:{busy:false,dismiss(){events.push('power-dismiss');}}},bluetoothLoader:{item:null},
    launcher:null,clipboard:null,Qt:{callLater(fn){callbacks.push(fn)}},Quickshell:{reload(hard){events.push(['reload',hard])}}};
context.root=context;vm.createContext(context);vm.runInContext(FUNCTIONS,context);
context.togglePower();assert.equal(context.powerEnabled,true);assert.deepEqual(events,['power-dismiss']);
context.powerEnabled=false;context.powerLoader.item=null;context.togglePower();assert.equal(context.powerEnabled,true);
for(const name of ['toggleBluetooth','toggleClipboard','showClipboard','toggleLauncher','showLauncher']){
    events.length=0;context.powerEnabled=true;context.powerLoader.item={busy:false,dismiss(){events.push('power-dismiss')}};
    context.launcherEnabled=false;context.launcher=null;context.clipboard=null;context.clipboardEnabled=false;context.bluetoothEnabled=false;
    context[name]();assert.equal(context.powerEnabled,true,name+' unloaded instead of dismissing');assert.equal(events[0],'power-dismiss');
    context.powerLoader.item.busy=true;events.length=0;context.launcherEnabled=false;context.clipboardEnabled=false;context.bluetoothEnabled=false;
    context[name]();assert.deepEqual(events,[],name+' affected pending power action');
    assert.equal(context.launcherEnabled||context.clipboardEnabled||context.bluetoothEnabled,false);
}
assert.equal(context.reloadConfiguration(),'busy');assert.equal(callbacks.length,0);
context.powerLoader.item.busy=false;assert.equal(context.reloadConfiguration(),'queued');assert.equal(callbacks.length,1);
assert.deepEqual(events,[]);callbacks.shift()();assert.deepEqual(events,[['reload',true]]);
'''.replace('FUNCTIONS',json.dumps(functions))
        result=subprocess.run(['node','-e',script],capture_output=True,text=True,timeout=8)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)


@unittest.skipUnless(shutil.which('quickshell'),'isolated reload requires Quickshell')
class SourceReload(unittest.TestCase):
    def test_final_reload_reads_updated_lazy_component_with_watch_disabled(self):
        with tempfile.TemporaryDirectory(prefix='foundation-reload-') as directory:
            base=Path(directory)
            env=dict(os.environ)
            for name in ('WAYLAND_DISPLAY','DISPLAY','NIRI_SOCKET','HYPRLAND_INSTANCE_SIGNATURE','QS_CONFIG_PATH','QS_CONFIG_NAME'):
                env.pop(name,None)
            for name,leaf in [('HOME','home'),('XDG_RUNTIME_DIR','runtime'),('XDG_STATE_HOME','state'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data'),('XDG_CONFIG_HOME','config')]:
                path=base/leaf;path.mkdir(mode=0o700);env[name]=str(path)
            env.update(QT_QPA_PLATFORM='offscreen',QT_QUICK_BACKEND='software',
                       QS_NO_RELOAD_POPUP='1',
                       DBUS_SESSION_BUS_ADDRESS='unix:path='+str(base/'absent-user-bus'),
                       DBUS_SYSTEM_BUS_ADDRESS='unix:path='+str(base/'absent-system-bus'))
            (base/'Version.qml').write_text('import QtQuick\nQtObject { property string value: "one" }\n')
            reload_body=function('reloadConfiguration')[2]
            (base/'shell.qml').write_text('''import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
ShellRoot {
    Component.onCompleted: Quickshell.watchFiles = false
    Window { width: 100; height: 100; visible: true }
    QtObject { id: powerLoader; property var item: null }
    LazyLoader { id: revision; active: true; Version {} }
    IpcHandler {
        target: "foundation"
        function version(): string { return revision.item.value; }
        function reloadConfiguration(): string {'''+reload_body+'''}
    }
}
''')
            log_path=base/'log'
            with log_path.open('w') as log:
                process=subprocess.Popen(['quickshell','--path',str(base)],env=env,stdout=log,stderr=subprocess.STDOUT)
                def call(method):
                    return subprocess.run(['quickshell','ipc','--pid',str(process.pid),'call','foundation',method],env=env,capture_output=True,text=True,timeout=3)
                try:
                    deadline=time.monotonic()+5
                    while time.monotonic()<deadline:
                        result=call('version')
                        if result.returncode==0: break
                        if process.poll() is not None: break
                        time.sleep(.05)
                    self.assertEqual(result.returncode,0,log_path.read_text()+result.stderr)
                    self.assertEqual(result.stdout.strip(),'one')
                    (base/'Version.qml').write_text('import QtQuick\nQtObject { property string value: "two" }\n')
                    self.assertEqual(call('version').stdout.strip(),'one')
                    result=call('reloadConfiguration');self.assertEqual(result.returncode,0,result.stderr)
                    self.assertEqual(result.stdout.strip(),'queued')
                    deadline=time.monotonic()+5
                    while time.monotonic()<deadline:
                        result=call('version')
                        if result.returncode==0 and result.stdout.strip()=='two':break
                        time.sleep(.05)
                    self.assertEqual(result.stdout.strip(),'two',log_path.read_text()+result.stderr)
                finally:
                    if process.poll() is None:process.terminate()
                    process.wait(timeout=3)

    def test_reload_wrapper_waits_for_acknowledgment_and_reports_busy(self):
        with tempfile.TemporaryDirectory(prefix='foundation-reload-wrapper-') as directory:
            base=Path(directory);fake=base/'quickshell';trace=base/'calls'
            fake.write_text('#!'+sys.executable+'\nimport os,pathlib,sys\n'
                'with pathlib.Path(os.environ["DF_RELOAD_LOG"]).open("a") as log:log.write(sys.argv[-1]+"\\n")\n'
                'if sys.argv[-1]=="reloadConfiguration":\n'
                ' reply=os.environ["DF_RELOAD_REPLY"]\n'
                ' count=pathlib.Path(os.environ["DF_RELOAD_LOG"]).read_text().count("reloadConfiguration")\n'
                ' print(("Not ready to accept queries yet." if count<3 else "queued") if reply=="warming" else reply)\n')
            fake.chmod(0o755)
            env=dict(os.environ,PATH=str(base)+':/usr/bin:/bin',DF_RELOAD_LOG=str(trace))
            for reply,code in [('queued',0),('busy',1),('warming',0)]:
                trace.unlink(missing_ok=True);env['DF_RELOAD_REPLY']=reply
                result=subprocess.run(['/bin/bash',str(ROOT/'scripts/shell-reload')],env=env,capture_output=True,text=True,timeout=5)
                self.assertEqual(result.returncode,code,result.stderr)
                self.assertEqual(trace.read_text().splitlines(),['status','reloadConfiguration']*(3 if reply=='warming' else 1))
