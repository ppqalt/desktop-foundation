"""Finite native Bluetooth requests against fake commands, never real services."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'
DEVICE = '/org/bluez/hci7/dev_AB_CD_EF_01_02_03'
ADDRESS = 'AB:CD:EF:01:02:03'

FAKE = r'''
import json, os, pathlib, subprocess, sys, time
name = pathlib.Path(sys.argv[0]).name
base = pathlib.Path(os.environ['DF_BT_FIXTURE'])
config = json.loads((base / 'config.json').read_text())
with (base / 'calls.jsonl').open('a') as log:
    log.write(json.dumps({'program': name, 'args': sys.argv[1:], 'pid': os.getpid()}) + '\n')
if name == 'rfkill':
    if config.get('unblock_error'):
        print('permission denied', file=sys.stderr); sys.exit(1)
elif name == 'busctl':
    if 'tree' in sys.argv:
        if config.get('tree_error'):
            print('org.bluez unavailable', file=sys.stderr); sys.exit(1)
        print(config.get('tree', '/org/bluez\n/org/bluez/hci7'))
    elif 'set-property' in sys.argv:
        path = sys.argv[sys.argv.index('set-property') + 2]
        state_path = base / 'state.json'
        state = json.loads(state_path.read_text()) if state_path.exists() else {}
        attempts = state.get('attempts', {})
        attempts[path] = attempts.get(path, 0) + 1
        state['attempts'] = attempts
        state_path.write_text(json.dumps(state))
        if config.get('set_error') or attempts[path] <= config.get('transient', 0):
            print('org.bluez.Error.Failed', file=sys.stderr); sys.exit(1)
        state[path] = sys.argv[-1] == 'true'
        state_path.write_text(json.dumps(state))
    elif 'get-property' in sys.argv:
        property_name = sys.argv[-1]
        if config.get('stall_property') == property_name:
            time.sleep(30)
        if config.get('malformed_property') == property_name:
            print(config.get('property_output', 'not-json')); sys.exit(0)
        if property_name == 'Powered':
            state_path = base / 'state.json'
            state = json.loads(state_path.read_text()) if state_path.exists() else {}
            path = sys.argv[sys.argv.index('get-property') + 2]
            value = state.get(path, False)
        else:
            value = config.get(property_name, {'Paired': True, 'Bonded': False, 'Address': 'AB:CD:EF:01:02:03'}[property_name])
        print(json.dumps({'data': value}))
elif name == 'pactl':
    if config.get('pactl_error'):
        print('audio service unavailable', file=sys.stderr); sys.exit(1)
    if config.get('stall_pactl'):
        time.sleep(30)
    if 'cards_output' in config:
        print(config['cards_output'])
    else:
        print(json.dumps(config.get('cards', [])))
elif name == 'wl-copy':
    sys.stdin.buffer.read()
    owner = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'],
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL, start_new_session=True)
    (base / 'owner.pid').write_text(str(owner.pid))
'''


class BluetoothBackend(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        # PATH contains only our fakes; no failure can fall through to busctl,
        # rfkill, pactl or wl-copy belonging to the actual desktop.
        self.env = dict(os.environ, PATH=str(self.bin), HOME=str(self.base),
                        XDG_STATE_HOME=str(self.base / 'state'),
                        XDG_RUNTIME_DIR=str(self.base / 'runtime'),
                        DF_BT_FIXTURE=str(self.base))
        self.env.pop('DF_CLIPBOARD_STATE', None)
        for name in ('busctl', 'rfkill', 'pactl', 'wl-copy'):
            path = self.bin / name
            path.write_text('#!' + sys.executable + '\n' + FAKE)
            path.chmod(0o755)
        self.configure()

    def configure(self, **config):
        (self.base / 'config.json').write_text(json.dumps(config))

    def invoke(self, *args, input=None, timeout=12):
        return subprocess.run([str(BINARY), '--root', str(ROOT), *args],
                              env=self.env, input=input, capture_output=True,
                              text=True, timeout=timeout)

    def calls(self):
        path = self.base / 'calls.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def assert_failure(self, result):
        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        self.assertFalse(payload['success'])
        self.assertTrue(payload['error'])
        self.assertNotIn('panicked', result.stderr)
        return payload

    def test_radio_discovers_filters_and_deduplicates_all_adapters(self):
        self.configure(tree='\n'.join(['/org/bluez', '/org/bluez/hci7',
                       '/org/bluez/hci0', '/org/bluez/hci7', DEVICE,
                       '/org/bluez/hci0/child', '/org/bluez/hcix',
                       '/org/bluez/hci', '/org/bluez/hci2extra']))
        result = self.invoke('bluetooth', 'power', 'on')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {'success': True, 'powered': True})
        calls = self.calls()
        self.assertEqual([c['args'] for c in calls if c['program'] == 'rfkill'], [['unblock', 'bluetooth']])
        sets = [c['args'] for c in calls if 'set-property' in c['args']]
        self.assertEqual(len(sets), 2)
        self.assertEqual({args[3] for args in sets}, {'/org/bluez/hci0', '/org/bluez/hci7'})
        self.assertTrue(all(args[-1] == 'true' for args in sets))

    def test_radio_off_verifies_without_unblocking(self):
        result = self.invoke('bluetooth', 'power', 'off')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {'success': True, 'powered': False})
        self.assertFalse(any(c['program'] == 'rfkill' for c in self.calls()))
        self.assertTrue(any('get-property' in c['args'] and c['args'][-1] == 'Powered' for c in self.calls()))

    def test_transient_radio_rejection_is_retried_and_verified(self):
        self.configure(transient=1)
        result = self.invoke('bluetooth', 'power', 'on')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(sum('set-property' in c['args'] for c in calls), 2)
        self.assertEqual(sum('get-property' in c['args'] for c in calls), 1)

    def test_absent_adapter_does_not_unblock_or_mutate(self):
        self.configure(tree='/org/bluez\n' + DEVICE)
        payload = self.assert_failure(self.invoke('bluetooth', 'power', 'on'))
        self.assertIn('No Bluetooth adapter', payload['error'])
        self.assertEqual(len(self.calls()), 1)

    def test_discovery_and_rfkill_errors_stop_before_power_write(self):
        for configuration in ({'tree_error': True}, {'unblock_error': True}):
            with self.subTest(configuration=configuration):
                self.configure(**configuration)
                (self.base / 'calls.jsonl').unlink(missing_ok=True)
                self.assert_failure(self.invoke('bluetooth', 'power', 'on'))
                self.assertFalse(any('set-property' in c['args'] for c in self.calls()))

    def test_malformed_radio_property_fails_without_unbounded_retry(self):
        for output in ('not-json', '{"data":"true"}', '{}'):
            with self.subTest(output=output):
                self.configure(malformed_property='Powered', property_output=output)
                start = time.monotonic()
                self.assert_failure(self.invoke('bluetooth', 'power', 'on'))
                self.assertLess(time.monotonic() - start, 2)

    def test_permanent_radio_failure_obeys_shared_five_second_budget(self):
        self.configure(set_error=True)
        start = time.monotonic()
        self.assert_failure(self.invoke('bluetooth', 'power', 'on'))
        elapsed = time.monotonic() - start
        self.assertGreater(elapsed, 4.5)
        self.assertLess(elapsed, 7)

    def test_codecs_match_dynamic_address_path_and_availability(self):
        self.configure(cards=[
            {'properties': {'api.bluez5.address': ADDRESS.lower()}, 'profiles': {
                'a2dp-sink': {'description': 'codec LDAC'},
                'a2dp-sink-aac': {'available': True},
                'a2dp-sink-sbc': {},
                'a2dp-sink-sbc-xq': {'available': 'no'},
                'headset-head-unit-sbc': {},
                'a2dp-sink-aptx': {},
            }},
            {'properties': {'api.bluez5.path': DEVICE}, 'profiles': {
                'a2dp-sink-aac': {}, 'a2dp-sink-sbc_xq': {'available': True},
            }},
            {'properties': {'device.string': ADDRESS}, 'profiles': {'a2dp-sink-sbc': {}}},
            {'properties': {'api.bluez5.address': '11:22:33:44:55:66'},
             'profiles': {'a2dp-sink-ldac': {}}},
        ])
        result = self.invoke('bluetooth', 'codecs', DEVICE)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {'success': True, 'codecs': ['aac', 'ldac', 'sbc', 'sbc_xq']})
        calls = self.calls()
        self.assertEqual([c['args'] for c in calls if c['program'] == 'pactl'], [['--format=json', 'list', 'cards']])
        self.assertFalse(any('set-property' in c['args'] or c['program'] == 'rfkill' for c in calls))

    def test_bonded_device_is_accepted_and_unpaired_device_stops_discovery(self):
        self.configure(Paired=False, Bonded=True)
        result = self.invoke('bluetooth', 'codecs', DEVICE)
        self.assertEqual(json.loads(result.stdout), {'success': True, 'codecs': []})
        self.configure(Paired=False, Bonded=False)
        (self.base / 'calls.jsonl').unlink(missing_ok=True)
        self.assert_failure(self.invoke('bluetooth', 'codecs', DEVICE))
        self.assertFalse(any(c['program'] == 'pactl' for c in self.calls()))

    def test_unavailable_profiles_are_not_offered_and_empty_snapshot_is_valid(self):
        self.configure(cards=[{'properties': {'api.bluez5.address': ADDRESS}, 'profiles': {
            'a2dp-sink-sbc': {'available': False},
            'a2dp-sink-sbc_xq': {'available': 'no'},
            'headset-head-unit-aac': {'available': True},
        }}])
        result = self.invoke('bluetooth', 'codecs', DEVICE)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {'success': True, 'codecs': []})

    def test_codec_query_errors_and_malformed_data_do_not_report_zero_success(self):
        configurations = [
            {'malformed_property': 'Paired'}, {'Address': 'not-an-address'},
            {'pactl_error': True}, {'cards_output': '{"not":"a list"}'},
            {'cards_output': 'broken-json'},
        ]
        for configuration in configurations:
            with self.subTest(configuration=configuration):
                self.configure(**configuration)
                self.assert_failure(self.invoke('bluetooth', 'codecs', DEVICE))

    def test_codecs_support_snapshot_larger_than_default_command_limit(self):
        cards = [{'properties': {'device.string': 'hw:0'}, 'description': 'x' * 512} for _ in range(180)]
        cards.append({'properties': {'api.bluez5.address': ADDRESS},
                      'profiles': {'a2dp-sink-sbc': {'available': True}}})
        self.assertGreater(len(json.dumps(cards)), 64 * 1024)
        self.configure(cards=cards)
        result = self.invoke('bluetooth', 'codecs', DEVICE)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['codecs'], ['sbc'])

    def test_oversized_codec_snapshot_is_bounded(self):
        self.configure(cards_output=' ' * (2 * 1024 * 1024 + 1) + '[]')
        self.assert_failure(self.invoke('bluetooth', 'codecs', DEVICE))

    def test_invalid_cli_and_device_paths_never_spawn_commands(self):
        for args in [('power', 'toggle'), ('power', 'on', 'extra'), ('codecs',),
                     ('codecs', '/org/bluez/hci/dev_AB_CD_EF_01_02_03'),
                     ('codecs', DEVICE + ';touch'), ('unknown-action', DEVICE)]:
            with self.subTest(args=args):
                result = self.invoke('bluetooth', *args)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])

    def test_property_query_timeout_is_two_seconds_and_stops_child(self):
        self.configure(stall_property='Paired')
        start = time.monotonic()
        self.assert_failure(self.invoke('bluetooth', 'codecs', DEVICE))
        self.assertLess(time.monotonic() - start, 4)
        self.assertGreater(time.monotonic() - start, 1.5)
        self.assertTrue(self.stopped(self.calls()[0]['pid']))

    @staticmethod
    def stopped(pid):
        path = Path('/proc') / str(pid) / 'status'
        try:
            return 'State:\tZ' in path.read_text()
        except (FileNotFoundError, ProcessLookupError):
            return True

    def stop_owned(self, pid):
        if not self.stopped(pid):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def test_cancelled_backend_stops_its_stalled_direct_child(self):
        self.configure(stall_property='Paired')
        backend = subprocess.Popen([str(BINARY), '--root', str(ROOT), 'bluetooth', 'codecs', DEVICE],
                                   env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(lambda: backend.poll() is None and backend.kill())
        child = None
        try:
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline and not self.calls():
                time.sleep(.01)
            self.assertTrue(self.calls(), 'Fake query did not start')
            child = self.calls()[0]['pid']
            backend.kill()
            backend.wait(timeout=2)
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline and not self.stopped(child):
                time.sleep(.01)
            self.assertTrue(self.stopped(child), 'Owned child survived killed backend')
        finally:
            if backend.poll() is None:
                backend.kill(); backend.wait(timeout=2)
            if child:
                self.stop_owned(child)

    def test_clipboard_owner_survives_successful_native_copy_and_backend_exit(self):
        history = self.base / 'clipboard'
        stored = self.invoke('clipboard', '--state', str(history), 'store', 'text', input='fixture clipboard text')
        self.assertEqual(stored.returncode, 0, stored.stderr)
        item = json.loads((history / 'index.json').read_text())[0]
        owner = None
        try:
            copied = self.invoke('clipboard', '--state', str(history), 'copy', item['id'])
            self.assertEqual(copied.returncode, 0, copied.stderr)
            owner = int((self.base / 'owner.pid').read_text())
            time.sleep(.05)
            self.assertFalse(self.stopped(owner), 'Detached clipboard owner stopped with backend')
        finally:
            if owner:
                self.stop_owned(owner)


if __name__ == '__main__':
    unittest.main()
