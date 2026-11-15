"""Production lock presentation and real PAM in isolated windows/compositors.

Portable offscreen checks run whenever Quickshell is installed. The explicit
nested test additionally needs Weston/Niri/swaylock and never uses the host's
Wayland socket, PAM stack, or account password.
"""
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]


def environment(base):
    env = {k: v for k, v in os.environ.items() if k not in
           ('DISPLAY', 'WAYLAND_DISPLAY', 'NIRI_SOCKET', 'NOTIFY_SOCKET', 'DBUS_SESSION_BUS_ADDRESS')}
    env.update(HOME=str(base), XDG_CONFIG_HOME=str(base/'config'),
               XDG_STATE_HOME=str(base/'state'), XDG_RUNTIME_DIR=str(base),
               DF_FOUNDATION_ROOT=str(ROOT), QT_QPA_PLATFORMTHEME='basic',
               LIBGL_ALWAYS_SOFTWARE='1',
               __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    return env


def imports(base):
    (base/'components').symlink_to(ROOT/'shell/components', target_is_directory=True)
    (base/'theme').symlink_to(ROOT/'shell/theme', target_is_directory=True)


class LockPresentation(unittest.TestCase):
    def test_masking_keyboard_clear_and_shared_gallery_component(self):
        if not shutil.which('quickshell'):
            if os.environ.get('DF_REQUIRE_QML_TESTS'):
                self.fail('Quickshell is required for this CI job')
            self.skipTest('Quickshell is not installed')
        with tempfile.TemporaryDirectory(prefix='df-lock-ui-') as tmp:
            base = Path(tmp)
            imports(base)
            (base/'shell.qml').write_text('''import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "components"
ShellRoot {
 Window { id: window; visible:true; width:640; height:480
  property int step:0
  LockContent { id: content; anchors.fill:parent; accountName:"Test account"
   onSubmitted: { takeResponse(); errorText="Incorrect password"; }
  }
  TestCase { id: keyboard; name:"LockKeyboard"; optional:true; when:false }
  Timer { interval:150; repeat:true; running:true
   onTriggered: {
    const field=keyboard.findChild(content,"lockPassword");
    if (!field) throw new Error("Missing password field");
    function check(value, message) { if (!value) throw new Error(message); }
    if (window.step===0) { content.focusPassword(); keyboard.keyClick(Qt.Key_A); }
    if (window.step===1) {
     check(field.text.length===1,"Keyboard input was not delivered");
     check(field.echoMode===TextInput.Password && field.passwordMaskDelay===0,"Unmasked password");
     keyboard.keyClick(Qt.Key_Return);
    }
    if (window.step===2) {
     check(content.errorText==="Incorrect password" && !content.hasResponse,"Submit/clear failed");
     keyboard.keyClick(Qt.Key_B); keyboard.keyClick(Qt.Key_Escape);
    }
    if (window.step===3) { check(!content.hasResponse,"Escape retained input"); keyboard.keyClick(Qt.Key_C); content.clearEpoch++; }
    if (window.step===4) { check(!content.hasResponse,"Global clear failed"); console.log("DF_LOCK_UI_PASS"); Qt.quit(); }
    window.step++;
   }
  }
 }
}''')
            env = environment(base)
            env.update(QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software')
            result = subprocess.run(['quickshell', '--no-color', '--path', str(base)],
                                    env=env, text=True, capture_output=True, timeout=12)
            self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
            self.assertIn('DF_LOCK_UI_PASS', result.stdout)
            self.assertNotIn('Error:', result.stdout)
            self.assertIn('LockContent {', (ROOT/'shell/gallery.qml').read_text())


@unittest.skipUnless(os.environ.get('DF_RUN_NESTED_LOCK_TESTS') == '1',
                     'explicit private compositor integration test')
class NestedLock(unittest.TestCase):
    def test_real_lock_pam_failure_success_and_crash_replacement(self):
        for tool in ('weston', 'niri', 'swaylock', 'quickshell'):
            self.assertIsNotNone(shutil.which(tool), tool)
        with tempfile.TemporaryDirectory(prefix='df-auth-wayland-') as tmp:
            base = Path(tmp)
            imports(base)
            env = environment(base)
            env['QT_QPA_PLATFORM'] = 'wayland'
            children = []
            streams = []

            def spawn(command, child_env, name):
                path = base/(name+'.log')
                stream = path.open('w')
                streams.append(stream)
                child = subprocess.Popen(command, env=child_env, stdout=stream,
                                         stderr=subprocess.STDOUT, start_new_session=True)
                children.append(child)
                return child, path

            def wait_until(condition, timeout=8):
                deadline = time.monotonic()+timeout
                while not condition():
                    if time.monotonic()>deadline:
                        self.fail('Private compositor timeout: '+ '\n'.join(p.read_text() for p in base.glob('*.log')))
                    time.sleep(.025)

            try:
                spawn(['weston', '--backend=headless', '--renderer=pixman', '--shell=kiosk-shell.so',
                       '--no-config', '--socket=outer', '--width=1280', '--height=800'], env, 'weston')
                wait_until(lambda: (base/'outer').exists())
                (base/'niri.kdl').write_text('hotkey-overlay { skip-at-startup; }\n')
                spawn(['niri', '--config', str(base/'niri.kdl')], dict(env, WAYLAND_DISPLAY='outer'), 'niri')
                wait_until(lambda: any(p for p in base.glob('wayland-*') if not p.name.endswith('.lock')))
                socket = next(p for p in base.glob('wayland-*') if not p.name.endswith('.lock'))
                child_env = dict(env, WAYLAND_DISPLAY=socket.name)
                lock, log = spawn(['quickshell', '--no-color', '--path', str(ROOT/'shell/lock.qml')], child_env, 'crash')
                wait_until(lambda: 'DF_LOCK_SECURE' in log.read_text() or lock.poll() is not None)
                self.assertIn('DF_LOCK_SECURE', log.read_text())
                ensure = subprocess.run(['quickshell','ipc','--path',str(ROOT/'shell/lock.qml'),'call','sessionLock','ensureLocked'],env=child_env,capture_output=True,text=True,timeout=3)
                self.assertEqual(ensure.returncode,0,ensure.stdout+ensure.stderr)
                self.assertEqual(ensure.stdout.strip(),'secure')
                lock.kill(); lock.wait()
                fallback, fallback_log = spawn(['swaylock', '--color', '101014', '--ready-fd', '1'], child_env, 'fallback')
                wait_until(lambda: fallback_log.stat().st_size>0 or fallback.poll() is not None)
                self.assertIn(b'\n', fallback_log.read_bytes())
                self.assertIsNone(fallback.poll())
                fallback.kill(); fallback.wait()
                for module, expected in [('pam_deny.so', 'DF_PAM_DENIED'), ('pam_permit.so', 'DF_LOCK_AUTHENTICATED')]:
                    pam = base/'pam'
                    pam.mkdir(exist_ok=True)
                    (pam/'fixture').write_text('auth required '+module+'\naccount required pam_permit.so\n')
                    source = (ROOT/'shell/lock.qml').read_text().replace('import QtQuick\n', 'import QtQuick\nimport QtTest\n', 1)
                    source = source.replace('config: "desktop-foundation-lock"', 'config: "fixture"').replace('configDirectory: "/etc/pam.d"', 'configDirectory: "'+str(pam)+'"')
                    source = source.replace('onSubmitted: root.submit(content)', '''onSubmitted: root.submit(content)
                Timer { interval:200; running:sessionLock.secure
                    onTriggered: root.submit(content)
                }''')
                    if module=='pam_deny.so':
                        source = source.replace('root.failure = result ===', 'if (!sessionLock.secure || root.authenticated) throw new Error("Failed authentication unlocked session");\n                console.log("DF_PAM_DENIED"); Qt.quit();\n                root.failure = result ===')
                    path = base/('fixture-'+module+'.qml')
                    path.write_text(source)
                    proc, output = spawn(['quickshell', '--no-color', '--path', str(path)], child_env, module)
                    wait_until(lambda: proc.poll() is not None)
                    if module == "pam_permit.so":
                        self.assertEqual(proc.returncode, 0, output.read_text())
                    self.assertIn(expected, output.read_text())
                    if module=='pam_deny.so': self.assertNotIn('DF_LOCK_AUTHENTICATED',output.read_text())
                # Exercise the actual Rust supervisor, not only manual recovery.
                tools=base/'bin'; tools.mkdir()
                notify=base/'notified'
                notifier=tools/'systemd-notify'
                notifier.write_text('#!/bin/sh\nprintf "ready\\n" >> "$DF_TEST_NOTIFY"\n')
                notifier.chmod(0o755)
                supervisor_env=dict(child_env, PATH=str(tools)+os.pathsep+os.environ['PATH'],DF_TEST_NOTIFY=str(notify))
                binary=ROOT/'native/foundation/target/debug/desktop-foundationctl'
                self.assertTrue(binary.is_file(), 'Build the debug backend for nested tests')
                supervisor, supervisor_log=spawn([str(binary),'--root',str(ROOT),'lock-screen','serve'],supervisor_env,'supervisor')
                wait_until(lambda: notify.exists() and notify.read_text().count('ready')>=1 or supervisor.poll() is not None)
                self.assertIsNone(supervisor.poll(),supervisor_log.read_text())
                ids=(Path('/proc')/str(supervisor.pid)/'task'/str(supervisor.pid)/'children').read_text().split()
                quickshell=next(int(pid) for pid in ids if (Path('/proc')/pid/'comm').read_text().strip()=='quickshell')
                os.kill(quickshell,signal.SIGKILL)
                wait_until(lambda: notify.read_text().count('ready')>=2 or supervisor.poll() is not None)
                self.assertIsNone(supervisor.poll(),supervisor_log.read_text())
                os.killpg(supervisor.pid,signal.SIGTERM);supervisor.wait()
            finally:
                for child in reversed(children):
                    if child.poll() is None:
                        os.killpg(child.pid, signal.SIGTERM)
                        try: child.wait(timeout=3)
                        except subprocess.TimeoutExpired:
                            os.killpg(child.pid, signal.SIGKILL); child.wait()
                for stream in streams: stream.close()
