"""CLI integration with fake executables; never acts on the real desktop."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import application_roles
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'


class Backend(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'checkout'
        self.bin = self.base / 'bin'
        self.data = self.base / 'data'
        self.state = self.base / 'state'
        self.runtime = self.base / 'runtime'
        for path in [self.root / 'config', self.root / 'scripts', self.bin, self.data / 'applications', self.state, self.runtime]:
            path.mkdir(parents=True)
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ['PATH'],
                        HOME=str(self.base), XDG_DATA_HOME=str(self.data), XDG_DATA_DIRS='',
                        XDG_STATE_HOME=str(self.state), XDG_RUNTIME_DIR=str(self.runtime),
                        DF_FAKE_LOG=str(self.base / 'commands.jsonl'))
        self.roles = {}
        for name in ['terminal', 'browser', 'files', 'pdf', 'image', 'text', 'personalBrowser']:
            executable = 'df-test-' + name
            desktop = executable + '.desktop'
            self.fake(executable)
            (self.data / 'applications' / desktop).write_text('[Desktop Entry]\nType=Application\nName[fi]=Testi\nExec="' + str(self.bin / executable) + '" %U\n')
            self.roles[name] = dict(package='test-package', executable=executable, desktop=desktop, mimes=[])
        (self.root / 'config/application-roles.json').write_text(json.dumps(self.roles))
        self.fake('launch', directory=self.root / 'scripts')
        self.fake('session-exit', directory=self.root / 'scripts')

    def fake(self, name, body='pass', directory=None):
        file = (directory or self.bin) / name
        file.write_text('#!/usr/bin/env python3\nimport json,os,sys\nwith open(os.environ["DF_FAKE_LOG"], "a") as f: f.write(json.dumps(sys.argv) + "\\n")\n' + body + '\n')
        file.chmod(0o755)
        return file

    def invoke(self, *args, check=True):
        return subprocess.run([str(BINARY), '--root', str(self.root), *args], env=self.env,
                              capture_output=True, text=True, check=check, timeout=12)

    def calls(self):
        file = self.base / 'commands.jsonl'
        return [json.loads(line) for line in file.read_text().splitlines()] if file.exists() else []

    def test_role_launch_matches_existing_validation_and_profile_selection(self):
        for personal in [False, True]:
            journal = self.state / 'desktop-foundation/application-roles.json'
            journal.parent.mkdir(exist_ok=True)
            journal.write_text(json.dumps({'profile': 'personal' if personal else 'core', 'entries': {}}))
            for name in ['terminal', 'browser', 'files']:
                with patch.dict(os.environ, self.env), patch.object(application_roles, 'ROOT', self.root):
                    role = application_roles.roles(personal)[name]
                    application_roles.validate_role(name, role)
                    expected = [str(self.root / 'scripts/launch'), role['executable']]
                    expected += {'browser': ['about:blank'], 'files': [str(self.base)]}.get(name, [])
                self.assertEqual(json.loads(self.invoke('apps', 'launch', name, '--check').stdout), expected)
        self.assertEqual(self.calls(), [])

    def test_invalid_role_metadata_and_corrupt_profile_are_reported_before_execution(self):
        desktop = self.data / 'applications' / self.roles['browser']['desktop']
        desktop.write_text('[Desktop Entry]\nType=Application\nExec=sh -c browser\n')
        self.assertIn('differs', self.invoke('apps', 'launch', 'browser', check=False).stderr)
        journal = self.state / 'desktop-foundation/application-roles.json'
        journal.parent.mkdir(); journal.write_text('invalid')
        self.assertIn('Invalid state JSON', self.invoke('apps', 'launch', 'terminal', check=False).stderr)
        self.assertEqual(self.calls(), [])

    def test_registry_is_typed_and_power_plans_never_execute(self):
        self.fake('wpctl'); self.fake('systemctl')
        catalog = json.loads(self.invoke('actions', 'list').stdout)
        self.assertEqual(len(catalog), 9)
        self.assertTrue(all(action['available'] for action in catalog))
        self.assertEqual(next(a for a in catalog if a['id'] == 'power.reboot')['invocation_policy'], 'session-change')
        for action in ['suspend', 'reboot', 'poweroff', 'logout']:
            expected = [str(self.root / 'scripts/session-exit')] if action == 'logout' else ['systemctl', action]
            self.assertEqual(json.loads(self.invoke('power', '--check', action).stdout), expected)
            self.assertEqual(json.loads(self.invoke('actions', 'plan', 'power.' + action).stdout), expected)
        self.assertNotEqual(self.invoke('actions', 'invoke', 'power.reboot;touch', check=False).returncode, 0)
        self.assertEqual(self.calls(), [])

    def test_volume_native_osd_and_fallback_keep_existing_arguments(self):
        self.fake('wpctl', 'if sys.argv[1] == "get-volume": print("Volume: 0.325 [MUTED]")')
        self.fake('quickshell')
        self.fake('notify-send')
        self.invoke('volume', 'up')
        calls = self.calls()
        self.assertEqual(calls[0][1:], ['set-volume', '-l', '1', '@DEFAULT_AUDIO_SINK@', '3%+'])
        self.assertEqual(calls[1][1:], ['get-volume', '@DEFAULT_AUDIO_SINK@'])
        self.assertEqual(calls[2][1:], ['ipc', '--path', str(self.root / 'shell'), 'call', 'foundation', 'showVolume', '32', 'true'])
        self.fake('quickshell', 'sys.exit(1)')
        self.invoke('volume', 'down')
        calls = self.calls()
        self.assertEqual(calls[-4][1:], ['set-volume', '-l', '1', '@DEFAULT_AUDIO_SINK@', '3%-'])
        self.assertEqual(calls[-1][1:], ['--app-name', 'desktop-foundation-volume', '--hint',
                                     'string:x-canonical-private-synchronous:desktop-foundation-volume',
                                     '--expire-time', '1500', 'Muted · 32%'])

    def test_volume_failure_does_not_show_stale_osd(self):
        self.fake('wpctl', 'print("sink unavailable", file=sys.stderr); sys.exit(1)')
        self.fake('quickshell'); self.fake('notify-send')
        result = self.invoke('volume', 'up', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('sink unavailable', result.stderr)
        self.assertEqual(len(self.calls()), 1)

    def test_role_launch_hands_off_process_ownership_and_preserves_argv(self):
        self.invoke('apps', 'launch', 'browser')
        self.assertEqual(self.calls(), [[str(self.root / 'scripts/launch'), 'df-test-browser', 'about:blank']])
