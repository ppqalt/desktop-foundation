import importlib.util
from pathlib import Path
import unittest
import subprocess
import os
import sys
import time
from unittest.mock import patch, MagicMock

SPEC = importlib.util.spec_from_file_location('bluetooth_action', Path(__file__).resolve().parents[1] / 'scripts/bluetooth-action.py')
bluetooth = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bluetooth)


class BluetoothAudio(unittest.TestCase):
    def test_prefers_ldac_then_aac_then_available_priority(self):
        card = {'profiles': {
            'off': {'priority': 10000},
            'headset-head-unit': {'priority': 2000},
            'a2dp-sink-sbc': {'priority': 10, 'available': True},
            'a2dp-sink-aptx_hd': {'priority': 50, 'available': True},
            'a2dp-sink-aac': {'priority': 20, 'available': True},
            'a2dp-sink': {'description': 'High Fidelity Playback (A2DP Sink, codec LDAC)', 'priority': 1, 'available': True},
            'a2dp-sink-unavailable': {'priority': 100, 'available': False},
        }}
        self.assertEqual(bluetooth.profiles(card), ['a2dp-sink', 'a2dp-sink-aac', 'a2dp-sink-aptx_hd', 'a2dp-sink-sbc'])

    def test_matches_discovered_address_or_path_without_node_ids(self):
        address = 'AB:CD:EF:01:02:03'
        self.assertEqual(bluetooth.address_of({'properties': {'api.bluez5.address': address.lower()}}), address)
        self.assertEqual(bluetooth.address_of({'properties': {'api.bluez5.path': '/org/bluez/hci7/dev_AB_CD_EF_01_02_03'}}), address)
        self.assertEqual(bluetooth.address_of({'properties': {'device.string': 'hw:0'}}), '')

    def test_failed_preferred_codec_falls_back_and_changes_only_playback(self):
        card = {'name': 'bluez_card.dynamic', 'active_profile': 'a2dp-sink-aac',
                'properties': {'api.bluez5.address': 'AB:CD:EF:01:02:03'},
                'profiles': {'a2dp-sink': {'description': 'codec LDAC', 'priority': 10},
                             'a2dp-sink-aac': {'description': 'codec AAC', 'priority': 9}}}
        sink = {'name': 'bluez_output.dynamic', 'properties': {
            'api.bluez5.address': 'AB:CD:EF:01:02:03',
            'api.bluez5.profile': 'a2dp-sink', 'api.bluez5.codec': 'aac'}}
        calls = []
        def request(*args):
            calls.append(args)
            if args[-1] == 'a2dp-sink':
                raise subprocess.CalledProcessError(1, args)
            return ''
        with patch.object(bluetooth, 'snapshot', side_effect=[[card], [sink]]), \
                patch.object(bluetooth, 'command', side_effect=request), \
                patch.object(bluetooth.subprocess, 'Popen', return_value=MagicMock()), \
                patch.object(bluetooth.selectors, 'DefaultSelector', return_value=MagicMock()):
            result = bluetooth.configure_audio('AB:CD:EF:01:02:03')
        self.assertEqual(result, {'codec': 'aac', 'routed': True})
        self.assertEqual(calls, [('pactl', 'set-card-profile', 'bluez_card.dynamic', 'a2dp-sink'),
                                 ('pactl', 'set-card-profile', 'bluez_card.dynamic', 'a2dp-sink-aac'),
                                 ('pactl', 'set-default-sink', 'bluez_output.dynamic')])

    def test_own_client_events_do_not_cause_repeated_snapshots(self):
        card = {'name': 'discovered', 'active_profile': 'a2dp-sink-aac',
                'properties': {'api.bluez5.address': 'AB:CD:EF:01:02:03'},
                'profiles': {'a2dp-sink-aac': {'priority': 1}}}
        sink = {'name': 'discovered-sink', 'properties': {'api.bluez5.address': 'AB:CD:EF:01:02:03',
                'api.bluez5.profile': 'a2dp-sink', 'api.bluez5.codec': 'aac'}}
        with patch.object(bluetooth, 'snapshot', side_effect=[[], [], [card], [sink]]) as snapshots, \
                patch.object(bluetooth, 'command', return_value=''), \
                patch.object(bluetooth.subprocess, 'Popen', return_value=MagicMock()), \
                patch.object(bluetooth.selectors, 'DefaultSelector', return_value=MagicMock()), \
                patch.object(bluetooth.os, 'read', side_effect=[b"Event 'new' on client #90\n", b"Event 'new' on card #12\n"]) as reads:
            self.assertTrue(bluetooth.configure_audio('AB:CD:EF:01:02:03')['routed'])
        self.assertEqual(snapshots.call_count, 4)
        self.assertEqual(reads.call_count, 2)

    @unittest.skipUnless(sys.platform == 'linux', 'Linux process cleanup')
    def test_children_exit_even_if_helper_is_killed(self):
        script = str(Path(bluetooth.__file__))
        source = f"import importlib.util,subprocess,time; spec=importlib.util.spec_from_file_location('bt',{script!r}); m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m); child=subprocess.Popen(['sleep','30'],preexec_fn=m.parent_death); print(child.pid,flush=True); time.sleep(30)"
        parent = subprocess.Popen([sys.executable, '-c', source], stdout=subprocess.PIPE, text=True)
        child = None
        try:
            child = int(parent.stdout.readline())
            parent.kill()
            parent.wait(timeout=2)
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline:
                status = Path(f'/proc/{child}/status')
                if not status.exists() or 'State:\tZ' in status.read_text():
                    break
                time.sleep(.02)
            else:
                self.fail('Child survived destroyed helper')
        finally:
            if parent.poll() is None:
                parent.kill(); parent.wait()
            parent.stdout.close()
            if child:
                try: os.kill(child, 9)
                except ProcessLookupError: pass

    def test_failure_message_hides_dbus_details(self):
        self.assertIn('off', bluetooth.friendly_error('org.bluez.Error.NotReady'))
        self.assertNotIn('org.bluez', bluetooth.friendly_error('org.bluez.Error.Failed br-connection-page-timeout'))
