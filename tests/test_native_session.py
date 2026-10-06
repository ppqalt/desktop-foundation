"""Native startup decisions with private configuration and inert executables.

No Wayland, D-Bus, service manager, notification provider or wallpaper process
is contacted. Final exec ownership is checked using fake executable PIDs.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'

SPY = '''import json, os, pathlib, sys, time
name=pathlib.Path(sys.argv[0]).name
with pathlib.Path(os.environ['DF_SESSION_LOG']).open('a') as file:
    file.write(json.dumps({'name':name,'args':sys.argv[1:],'pid':os.getpid()})+'\\n')
if name == 'busctl':
    mode=os.environ.get('DF_OWNER_RESULT','free')
    if mode == 'stall': time.sleep(20)
    elif mode == 'fail': print('synthetic bus failure',file=sys.stderr);sys.exit(3)
    elif mode == 'large': print('x'*80000)
    else: print({'free':'b false','owned':'b true','bad':'garbage','typed':'s "false"'}.get(mode,mode))
if name == 'python3' and os.environ.get('DF_LEGACY_FAIL'): sys.exit(4)
if name in ('swaybg','fallback-swaybg','systemctl'):
    print(json.dumps({'name':name,'args':sys.argv[1:],'pid':os.getpid()}))
'''


class NativeSession(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='foundation-session-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base/'checkout'
        (self.root/'compositor/niri').mkdir(parents=True)
        self.bin = self.base/'bin'; self.bin.mkdir()
        self.state = self.base/'state/desktop-foundation'
        self.current = self.state/'theme/current'; self.current.mkdir(parents=True)
        self.image = self.base/'wall with spaces.png'; self.image.write_bytes(b'fixture image')
        self.log = self.base/'calls.jsonl'
        self.env = dict(PATH=str(self.bin), HOME=str(self.base),
                        XDG_STATE_HOME=str(self.base/'state'), XDG_CACHE_HOME=str(self.base/'cache'),
                        XDG_CONFIG_HOME=str(self.base/'config'), XDG_DATA_HOME=str(self.base/'data'),
                        XDG_RUNTIME_DIR=str(self.base/'runtime'), NIRI_SOCKET='isolated-presence-guard',
                        DF_SESSION_LOG=str(self.log), LANG='C', LC_ALL='C')
        for command in ('swaybg','systemctl','busctl','python3'):
            self.spy(self.bin/command)
        self.config(self.root/'compositor/niri/wallpaper.toml', self.image)
        self.config(self.current/'wallpaper.toml', self.image)

    def spy(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('#!'+sys.executable+'\n'+SPY); path.chmod(0o755)

    def config(self, path, image, mode=None):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('image = '+json.dumps(str(image))+'\n'+
                        (('mode = '+json.dumps(mode)+'\n') if mode is not None else ''))

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def invoke(self, *args, environment=None, check=True):
        process = subprocess.Popen([str(BINARY),'--root',str(self.root),*args],
                                   env=environment or self.env, cwd=self.base,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            stdout, stderr = process.communicate(timeout=7)
        except subprocess.TimeoutExpired:
            process.kill(); process.wait(); raise
        result = subprocess.CompletedProcess(process.args,process.returncode,stdout,stderr)
        if check: self.assertEqual(result.returncode,0,result.stderr)
        return result, process.pid

    def test_current_bundle_execs_packaged_wallpaper_without_python(self):
        for mode in ('fill','fit','center','tile'):
            with self.subTest(mode=mode):
                self.config(self.current/'wallpaper.toml', self.image, mode)
                result,pid = self.invoke('wallpaper','start')
                output=json.loads(result.stdout)
                self.assertEqual(output['pid'],pid)
                self.assertEqual(output['args'],['--image',str(self.image),'--mode',mode])
        self.assertEqual([call['name'] for call in self.calls()],['swaybg']*4)
        self.assertFalse((self.base/'cache').exists())

    def test_active_runtime_path_wins_and_missing_runtime_image_fails(self):
        runtime = self.base/'runtime-image.png'; runtime.write_bytes(b'runtime')
        self.config(self.current/'wallpaper.toml', runtime)
        result,_ = self.invoke('wallpaper','start')
        self.assertEqual(json.loads(result.stdout)['args'][1],str(runtime))
        runtime.unlink(); self.log.unlink()
        result,_ = self.invoke('wallpaper','start',check=False)
        self.assertNotEqual(result.returncode,0)
        self.assertIn('unavailable',result.stderr)
        self.assertEqual(self.calls(),[])

    def test_current_symlink_and_repository_only_basename_recovery(self):
        revision=self.state/'theme/revisions/fixture'; revision.mkdir(parents=True)
        (self.current/'wallpaper.toml').unlink(); self.current.rmdir()
        self.current.symlink_to(revision,target_is_directory=True)
        image=self.root/'wallpapers/shipped.png'; image.parent.mkdir(); image.write_bytes(b'shipped')
        self.config(self.root/'compositor/niri/wallpaper.toml','~/missing/shipped.png')
        result,_=self.invoke('wallpaper','start')
        self.assertEqual(json.loads(result.stdout)['args'][1],str(image))
        self.assertEqual([call['name'] for call in self.calls()],['swaybg'])
        self.config(revision/'wallpaper.toml',self.image,'fit')
        result,_=self.invoke('wallpaper','start')
        self.assertEqual(json.loads(result.stdout)['args'][1],str(self.image))

    def test_home_expansion_and_shell_characters_stay_literal(self):
        image=self.base/'wall $(touch OWNED); `x`.png'; image.write_bytes(b'private')
        self.config(self.current/'wallpaper.toml','~/'+image.name)
        result,_=self.invoke('wallpaper','start')
        self.assertEqual(json.loads(result.stdout)['args'][1],str(image))
        self.assertFalse((self.base/'OWNED').exists())

    def test_state_local_executable_is_fallback_and_path_package_wins(self):
        fallback=self.state/'bin/swaybg'; self.spy(fallback)
        result,_=self.invoke('wallpaper','start')
        self.assertEqual(self.calls()[-1]['pid'],json.loads(result.stdout)['pid'])
        # The spy reports identical basenames, so argv[0] selection is observed
        # by giving only the fallback a distinct output signature.
        fallback.write_text(fallback.read_text().replace("name=pathlib.Path(sys.argv[0]).name","name='fallback-swaybg'"))
        result,_=self.invoke('wallpaper','start'); self.assertEqual(json.loads(result.stdout)['name'],'swaybg')
        (self.bin/'swaybg').unlink()
        result,_=self.invoke('wallpaper','start'); self.assertEqual(json.loads(result.stdout)['name'],'fallback-swaybg')
        fallback.chmod(0o600)
        result,_=self.invoke('wallpaper','start',check=False)
        self.assertNotEqual(result.returncode,0)
        self.assertIn('swaybg unavailable',result.stderr)

    def test_legacy_missing_or_broken_bundle_prepares_exactly_once_then_execs(self):
        shutil.rmtree(self.current)
        for broken in (False,True):
            with self.subTest(broken=broken):
                if broken: self.current.symlink_to(self.base/'absent')
                self.log.unlink(missing_ok=True)
                result,pid=self.invoke('wallpaper','start')
                calls=self.calls()
                self.assertEqual([call['name'] for call in calls],['python3','swaybg'])
                self.assertEqual(calls[0]['args'],[str(self.root/'scripts/niri_wallpaper.py'),'--prepare-backdrop',str(self.image),'fill'])
                self.assertEqual(calls[1]['pid'],pid)
                self.assertEqual(json.loads(result.stdout)['pid'],pid)

    def test_failed_legacy_preparation_never_starts_wallpaper(self):
        shutil.rmtree(self.current); self.env['DF_LEGACY_FAIL']='1'
        result,_=self.invoke('wallpaper','start',check=False)
        self.assertNotEqual(result.returncode,0)
        self.assertEqual([call['name'] for call in self.calls()],['python3'])

    def test_invalid_config_and_images_fail_before_side_effects(self):
        config=self.current/'wallpaper.toml'
        inputs=['image = 12','image = ""','image = "x"\nmode = "stretch"',
                'image = "x"\nmode = 2','image = "x"\n#'+'x'*70000]
        for content in inputs:
            with self.subTest(content=content[:50]):
                config.write_text(content)
                result,_=self.invoke('wallpaper','start',check=False)
                self.assertNotEqual(result.returncode,0)
                self.assertEqual(self.calls(),[])
        fifo=self.base/'fifo'; os.mkfifo(fifo)
        for image in (self.base,fifo,self.base/'absent'):
            self.config(config,image)
            result,_=self.invoke('wallpaper','start',check=False)
            self.assertNotEqual(result.returncode,0)
            self.assertEqual(self.calls(),[])

    def test_session_guards_run_before_config_and_commands(self):
        for socket in (None,''):
            env=dict(self.env)
            if socket is None: env.pop('NIRI_SOCKET')
            else: env['NIRI_SOCKET']=socket
            for args in (('wallpaper','start'),('notifications','start')):
                result,_=self.invoke(*args,environment=env,check=False)
                self.assertNotEqual(result.returncode,0)
                self.assertIn('Niri session',result.stderr)
                self.assertEqual(self.calls(),[])

    def test_notification_start_hands_pid_to_existing_unit_start(self):
        result,pid=self.invoke('notifications','start')
        output=json.loads(result.stdout)
        self.assertEqual(output['pid'],pid)
        self.assertEqual(output['args'],['--user','start','desktop-foundation-notifications.service'])
        self.assertEqual([call['name'] for call in self.calls()],['systemctl'])

    def test_owner_probe_accepts_only_explicit_free_bus_and_never_starts_service(self):
        for answer,success in (('free',True),('owned',False),('bad',False),('typed',False),('fail',False),('large',False)):
            with self.subTest(answer=answer):
                self.env['DF_OWNER_RESULT']=answer
                result,_=self.invoke('notifications','check-owner',check=False)
                self.assertEqual(result.returncode==0,success)
        for call in self.calls():
            self.assertEqual(call['name'],'busctl')
            self.assertEqual(call['args'],['--user','call','org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner','s','org.freedesktop.Notifications'])

    def test_owner_probe_timeout_is_bounded(self):
        self.env['DF_OWNER_RESULT']='stall'; start=time.monotonic()
        result,_=self.invoke('notifications','check-owner',check=False)
        self.assertNotEqual(result.returncode,0)
        self.assertIn('timed out',result.stderr)
        self.assertLess(time.monotonic()-start,3.5)

    def test_python_compatibility_files_forward_to_native_without_other_side_effects(self):
        # Copy the wrappers into a private checkout whose native executable is
        # only an argv spy. Imported blur helpers remain inert on this route.
        scripts=self.root/'scripts'; scripts.mkdir()
        fake=self.root/'native/foundation/target/release/desktop-foundationctl'; self.spy(fake)
        fake.write_text(fake.read_text()+"\nprint(json.dumps({'args':sys.argv[1:]}))\n")
        for script,command in (('niri_wallpaper.py',['wallpaper','start']),('niri_notifications.py',['notifications','start'])):
            shutil.copy2(ROOT/'scripts'/script,scripts/script)
            result=subprocess.run([sys.executable,str(scripts/script)],env=self.env,cwd=self.base,capture_output=True,text=True,timeout=5)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertEqual(json.loads(result.stdout)['args'],['--root',str(self.root),*command])
        self.assertEqual(len(self.calls()),2)
