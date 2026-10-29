"""Control-center requests against inert PipeWire/NetworkManager executables."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'

class ControlCenter(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        tools = self.base / 'bin'; tools.mkdir()
        runtime = self.base / 'runtime'; runtime.mkdir()
        self.log = self.base / 'calls.jsonl'
        self.env = {**os.environ, 'HOME': str(self.base), 'XDG_RUNTIME_DIR': str(runtime),
                    'PATH': str(tools) + ':' + os.environ['PATH'], 'DF_CONTROL_LOG': str(self.log)}
        source = '''#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
args=sys.argv[1:]
name=Path(sys.argv[0]).name
with open(os.environ['DF_CONTROL_LOG'],'a') as f:f.write(json.dumps([name,*args])+'\\n')
if os.environ.get('DF_FAIL_CONTROL')==name:sys.exit(8)
if name=='pactl':
    if args==['--format=json','info']:print(json.dumps({'default_sink_name':'speakers','default_source_name':'mic'}))
    elif args[:2]==['--format=json','list']:
        kind=args[-1]
        if kind in ['sink-inputs','source-outputs']:print('[{"index":42}]')
        else:
            endpoint={'index':1,'name':'speakers' if kind=='sinks' else 'mic','description':'Speakers' if kind=='sinks' else 'Microphone','mute':False,'volume':{'mono':{'value':32768}}}
            print(json.dumps([endpoint]+([dict(endpoint,name='speakers.monitor')] if kind=='sources' else [])))
elif name=='nmcli':
    if args[-1]=='general':print('enabled:enabled:connected:full')
    elif args[-2:]==['device','status']:print('enp4s0:ethernet:connected:Office\\\\:LAN\\nwlan0:wifi:disconnected:')
    elif args[-2:]==['connection','show']:print('Office\\\\:LAN:12345678-abcd-1234-abcd-123456789abc:802-3-ethernet:enp4s0')
'''
        for name in ['pactl', 'nmcli', 'quickshell']:
            p=tools/name; p.write_text(source); p.chmod(0o755)

    def invoke(self,*args,**env):
        return subprocess.run([str(BINARY),'--root',str(ROOT),'control-center',*args],env={**self.env,**env},text=True,capture_output=True,timeout=8)
    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_snapshot_has_audio_and_escaped_network_names(self):
        r=self.invoke('snapshot'); self.assertEqual(r.returncode,0,r.stderr); data=json.loads(r.stdout)
        self.assertEqual(data['audio']['outputs'][0]['volume'],50)
        self.assertEqual(len(data['audio']['inputs']),1)
        self.assertEqual(data['network']['connections'][0]['name'],'Office:LAN')
        self.assertTrue(data['network']['connections'][0]['active'])
        self.assertEqual(data['network']['devices'][0]['connection'],'Office:LAN')
        self.assertFalse(any('password' in word for call in self.calls() for word in call))

    def test_service_failure_isolated_from_other_controls(self):
        for name,available,unavailable in [('pactl','network','audio'),('nmcli','audio','network')]:
            r=self.invoke('snapshot',DF_FAIL_CONTROL=name)
            self.assertEqual(r.returncode,0,r.stderr); data=json.loads(r.stdout)
            self.assertTrue(data[available]['available']);self.assertFalse(data[unavailable]['available'])
            self.assertIn('error',data[unavailable])

    def test_volume_and_microphone_mute_are_bounded_and_separate(self):
        for args,expected in [(('volume','output','72'),['pactl','set-sink-volume','@DEFAULT_SINK@','72%']),
                              (('volume','input','22'),['pactl','set-source-volume','@DEFAULT_SOURCE@','22%']),
                              (('mute','input','on'),['pactl','set-source-mute','@DEFAULT_SOURCE@','1'])]:
            r=self.invoke(*args);self.assertEqual(r.returncode,0,r.stderr);self.assertIn(expected,self.calls())
        before=self.calls()
        for args in [('volume','output','101'),('volume','output','-1'),('volume','output','nan'),('mute','input','toggle')]:
            self.assertNotEqual(self.invoke(*args).returncode,0)
        self.assertEqual(self.calls(),before)

    def test_selection_moves_existing_streams(self):
        for kind,name,default,move in [('output','speakers','set-default-sink','move-sink-input'),('input','mic','set-default-source','move-source-output')]:
            r=self.invoke('select',kind,name);self.assertEqual(r.returncode,0,r.stderr)
            self.assertIn(['pactl',default,name],self.calls());self.assertIn(['pactl',move,'42',name],self.calls())
        before=self.calls(); r=self.invoke('select','output','--bogus'); self.assertNotEqual(r.returncode,0)
        self.assertFalse(any(call[1] in ['set-default-sink','move-sink-input'] for call in self.calls()[len(before):]))

    def test_network_changes_use_separate_arguments_and_validated_ids(self):
        r=self.invoke('connection','up','12345678-abcd-1234-abcd-123456789abc');self.assertEqual(r.returncode,0,r.stderr)
        self.assertIn(['nmcli','--wait','10','connection','up','uuid','12345678-abcd-1234-abcd-123456789abc'],self.calls())
        for args in [('wifi','off'),('networking','off'),('device','disconnect','enp4s0')]:
            r=self.invoke(*args);self.assertEqual(r.returncode,0,r.stderr)
        before=self.calls()
        for args in [('connection','up','--bogus'),('device','connect','--bogus'),('device','connect','eth0;reboot'),('networking','invalid')]:
            self.assertNotEqual(self.invoke(*args).returncode,0)
        self.assertEqual(before,self.calls())

    def test_toggle_passes_shared_style_without_starting_monitor(self):
        r=self.invoke('toggle');self.assertEqual(r.returncode,0,r.stderr)
        call=self.calls()[-1];self.assertEqual(call[-2],'toggleControlCenter')
        style=json.loads(call[-1]);self.assertEqual(style,dict(width=720,padding=12,radius=14,opacity=.75,openingMs=240,closingMs=180))
        self.assertTrue(all(call[0]=='quickshell' for call in self.calls()))

if __name__=='__main__':unittest.main()
