"""BlueZ/playback actions through command fixtures, never real desktop services."""
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
A2DP = '0000110b-0000-1000-8000-00805f9b34fb'

FAKE = r'''
import json, os, pathlib, sys, time
name = pathlib.Path(sys.argv[0]).name
base = pathlib.Path(os.environ['DF_BT_ACTION_FIXTURE'])
config = json.loads((base / 'config.json').read_text())
args = sys.argv[1:]

def read(path, default):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return default

def write(path, value):
    temporary = path.with_name(path.name + '.' + str(os.getpid()))
    temporary.write_text(json.dumps(value))
    temporary.replace(path)

def snapshots():
    return sum(read(base / (kind + '_count.json'), 0) for kind in ('cards', 'sinks'))

# A pactl subscription has no readiness acknowledgement. Lists wait only in
# this fake until the already-spawned subscription executable starts, avoiding
# interpreter-startup scheduling races in the recorded call ordering.
if name == 'pactl' and 'list' in args and config.get('require_subscription', True):
    deadline = time.monotonic() + 2
    while not (base / 'subscribed').exists() and time.monotonic() < deadline:
        time.sleep(.002)
    if not (base / 'subscribed').exists():
        print('snapshot issued without subscription', file=sys.stderr); sys.exit(1)
parts = base / 'event_parts.jsonl'
call = {'program': name, 'args': args, 'pid': os.getpid(),
        'event_parts': len(parts.read_text().splitlines()) if parts.exists() else 0}
with (base / 'calls.jsonl').open('a') as log:
    log.write(json.dumps(call) + '\n')
if name == 'busctl':
    state_path = base / 'bus_state.json'
    state = read(state_path, {})
    if 'get-property' in args:
        property_name = args[-1]
        if config.get('stall_property') == property_name:
            time.sleep(60)
        if config.get('error_property') == property_name:
            print(config.get('bus_error', 'org.bluez.Error.Failed'), file=sys.stderr); sys.exit(1)
        if config.get('malformed_property') == property_name:
            print(config.get('property_output', 'invalid json')); sys.exit(0)
        queries = state.get('queries', {})
        index = queries.get(property_name, 0)
        queries[property_name] = index + 1
        state['queries'] = queries
        write(state_path, state)
        defaults = {'Paired': True, 'Bonded': False, 'Powered': True, 'Blocked': False,
                    'Connected': state.get('connected', False),
                    'Address': 'AB:CD:EF:01:02:03', 'UUIDs': []}
        if property_name in config.get('property_sequences', {}):
            sequence = config['property_sequences'][property_name]
            value = sequence[min(index, len(sequence) - 1)]
        else:
            value = config.get(property_name, defaults[property_name])
        print(json.dumps({'data': value}))
    elif 'call' in args:
        method = args[-1]
        attempts = state.get('attempts', {})
        attempts[method] = attempts.get(method, 0) + 1
        state['attempts'] = attempts
        write(state_path, state)
        if config.get('call_error') or attempts[method] <= config.get('transient_connect_failures', 0):
            print(config.get('bus_error', 'org.bluez.Error.Failed br-connection-page-timeout'), file=sys.stderr); sys.exit(1)
        state['connected'] = method == 'Connect'
        write(state_path, state)
elif name == 'pactl':
    if args == ['subscribe']:
        (base / 'subscribed').write_text('ready')
        for index, item in enumerate(config.get('events', [])):
            deadline = time.monotonic() + 20
            while snapshots() < item.get('after_snapshots', 2) and time.monotonic() < deadline:
                time.sleep(.002)
            time.sleep(item.get('delay', 0))
            with parts.open('a') as log:
                log.write(json.dumps({'part': index, 'chunk': item['chunk']}) + '\n')
            sys.stdout.write(item['chunk']); sys.stdout.flush()
        if config.get('subscribe_eof'):
            sys.exit(config.get('subscribe_status', 0))
        time.sleep(60)
    elif 'list' in args:
        kind = args[-1]
        count_path = base / (kind + '_count.json')
        index = read(count_path, 0)
        write(count_path, index + 1)
        if config.get('stall_snapshot') == kind:
            time.sleep(60)
        if config.get('snapshot_error') == kind:
            print('audio fixture unavailable', file=sys.stderr); sys.exit(1)
        if kind + '_output' in config:
            print(config[kind + '_output']); sys.exit(0)
        fixtures = config.get(kind + '_snapshots', [config.get(kind, [])])
        print(json.dumps(fixtures[min(index, len(fixtures) - 1)]))
    elif args[0] == 'set-card-profile':
        if args[-1] in config.get('profile_errors', []):
            print('profile fixture rejected', file=sys.stderr); sys.exit(1)
    elif args[0] == 'set-default-sink':
        if config.get('route_error'):
            print('route fixture rejected', file=sys.stderr); sys.exit(1)
'''


def card(profile='a2dp-sink-aac', profiles=None, properties=None):
    return {'name': 'bluez_card.dynamic', 'active_profile': profile,
            'properties': properties or {'api.bluez5.address': ADDRESS.lower()},
            'profiles': profiles or {profile: {'description': 'codec AAC', 'priority': 10}}}


def sink(codec='aac', profile='a2dp-sink', name='bluez_output.dynamic', properties=None):
    return {'name': name, 'properties': properties or {
        'api.bluez5.address': ADDRESS, 'api.bluez5.profile': profile, 'api.bluez5.codec': codec}}


class BluetoothActions(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        self.env = dict(os.environ, PATH=str(self.bin), HOME=str(self.base),
                        XDG_RUNTIME_DIR=str(self.base / 'runtime'),
                        XDG_STATE_HOME=str(self.base / 'state'),
                        DF_BT_ACTION_FIXTURE=str(self.base), LC_ALL='C')
        for key in ('WAYLAND_DISPLAY', 'DISPLAY', 'NIRI_SOCKET',
                    'HYPRLAND_INSTANCE_SIGNATURE', 'DBUS_SESSION_BUS_ADDRESS'):
            self.env.pop(key, None)
        # Failure/missing executable cannot fall through to real busctl/pactl.
        for name in ('busctl', 'pactl'):
            path = self.bin / name
            path.write_text('#!' + sys.executable + '\n' + FAKE)
            path.chmod(0o755)
        self.configure()

    def configure(self, **config):
        (self.base / 'config.json').write_text(json.dumps(config))

    def invoke(self, *args, timeout=18):
        return subprocess.run([str(BINARY), '--root', str(ROOT), 'bluetooth', *args],
                              env=self.env, capture_output=True, text=True, timeout=timeout)

    def action(self, action='connect', **kwargs):
        return self.invoke(action, DEVICE, **kwargs)

    def calls(self):
        path = self.base / 'calls.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def clear_fixture_state(self):
        for name in ('calls.jsonl', 'bus_state.json', 'cards_count.json', 'sinks_count.json',
                     'subscribed', 'event_parts.jsonl'):
            (self.base / name).unlink(missing_ok=True)

    def payload(self, result):
        self.assertNotIn('panicked', result.stderr)
        return json.loads(result.stdout)

    def assert_success(self, result):
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = self.payload(result)
        self.assertTrue(payload['success'])
        return payload

    def assert_failure(self, result):
        self.assertNotEqual(result.returncode, 0)
        return self.assert_unsuccessful(result)

    def assert_unsuccessful(self, result):
        payload = self.payload(result)
        self.assertFalse(payload['success'])
        self.assertTrue(payload['error'])
        self.assertNotIn('org.bluez', payload['error'])
        return payload

    def bus_calls(self):
        return [c for c in self.calls() if c['program'] == 'busctl' and 'call' in c['args']]

    def audio_calls(self):
        return [c for c in self.calls() if c['program'] == 'pactl']

    def snapshots(self):
        return [c for c in self.audio_calls() if 'list' in c['args']]

    def assert_subscription_stopped(self):
        subscriptions = [c for c in self.audio_calls() if c['args'] == ['subscribe']]
        self.assertEqual(len(subscriptions), 1)
        self.assertTrue(self.stopped(subscriptions[0]['pid']), 'Audio subscription survived action completion')

    def test_generic_device_connect_confirms_twice_without_touching_audio(self):
        payload = self.assert_success(self.action())
        self.assertEqual(payload, {'success': True, 'connected': True})
        self.assertEqual(len(self.bus_calls()), 1)
        self.assertEqual(self.bus_calls()[0]['args'], ['--system', '--timeout=40s', 'call',
                         'org.bluez', DEVICE, 'org.bluez.Device1', 'Connect'])
        properties = [c['args'][-1] for c in self.calls() if 'get-property' in c['args']]
        self.assertEqual(properties.count('Connected'), 2)
        self.assertNotIn('Bonded', properties)
        self.assertEqual(self.audio_calls(), [])

    def test_disconnect_confirms_native_state_and_never_touches_playback(self):
        payload = self.assert_success(self.action('disconnect'))
        self.assertEqual(payload, {'success': True, 'connected': False})
        self.assertEqual(self.bus_calls()[0]['args'], ['--system', '--timeout=15s', 'call',
                         'org.bluez', DEVICE, 'org.bluez.Device1', 'Disconnect'])
        self.assertEqual(self.audio_calls(), [])

    def test_bonded_device_is_accepted_and_unpaired_device_never_connects(self):
        self.configure(Paired=False, Bonded=True)
        self.assert_success(self.action())
        self.clear_fixture_state()
        self.configure(Paired=False, Bonded=False)
        self.assert_failure(self.action())
        self.assertEqual(self.bus_calls(), [])
        self.assertEqual(self.audio_calls(), [])

    def test_off_adapter_and_blocked_device_stop_before_connect_with_friendly_errors(self):
        for configuration, expected in (({'Powered': False}, 'off'), ({'Blocked': True}, 'blocked')):
            with self.subTest(configuration=configuration):
                self.configure(**configuration)
                self.clear_fixture_state()
                payload = self.assert_failure(self.action())
                self.assertIn(expected, payload['error'].lower())
                self.assertEqual(self.bus_calls(), [])
                self.assertEqual(self.audio_calls(), [])

    def test_connect_and_disconnect_require_confirmed_native_state(self):
        for action, sequence in (('connect', [False]), ('connect', [True, False]),
                                 ('disconnect', [True])):
            with self.subTest(action=action, sequence=sequence):
                self.configure(property_sequences={'Connected': sequence})
                self.clear_fixture_state()
                self.assert_failure(self.action(action))
                self.assertEqual(len(self.bus_calls()), 1)

    def test_native_errors_map_to_friendly_ui_messages(self):
        fixtures = [('org.bluez.Error.NotReady', 'off'), ('org.bluez.Error.Blocked', 'blocked'),
                    ('org.bluez.Error.InProgress', 'progress'),
                    ('org.bluez.Error.AuthenticationFailed', 'refused'),
                    ('org.bluez.Error.NotAuthorized', 'refused'),
                    ('org.bluez.Error.Failed br-connection-page-timeout', 'nearby')]
        for error, expected in fixtures:
            with self.subTest(error=error):
                self.configure(call_error=True, bus_error=error)
                self.clear_fixture_state()
                payload = self.assert_failure(self.action())
                self.assertIn(expected, payload['error'].lower())
        self.configure(call_error=True)
        self.clear_fixture_state()
        self.assertIn('disconnect', self.assert_failure(self.action('disconnect'))['error'].lower())

    def test_malformed_properties_and_address_refuse_connect_before_mutation(self):
        fixtures = [{'malformed_property': 'Paired'}, {'Paired': 'true'},
                    {'Address': 'not-an-address'}, {'UUIDs': 'not-a-list'}]
        for fixture in fixtures:
            with self.subTest(fixture=fixture):
                self.configure(**fixture)
                self.clear_fixture_state()
                self.assert_failure(self.action())
                self.assertEqual(self.bus_calls(), [])

    def test_a2dp_connect_subscribes_first_and_routes_only_playback(self):
        self.configure(UUIDs=[A2DP], cards=[card()], sinks=[sink()])
        payload = self.assert_success(self.action())
        self.assertEqual(payload, {'success': True, 'connected': True, 'routed': True, 'codec': 'aac'})
        calls = self.audio_calls()
        self.assertEqual(calls[0]['args'], ['subscribe'])
        self.assertEqual([c['args'] for c in calls if c['args'][0].startswith('set-')],
                         [['set-card-profile', 'bluez_card.dynamic', 'a2dp-sink-aac'],
                          ['set-default-sink', 'bluez_output.dynamic']])
        self.assert_subscription_stopped()

    def test_failed_ldac_profile_falls_back_to_aac_without_mutating_capture(self):
        profiles = {'off': {'priority': 9999}, 'headset-head-unit': {'priority': 2000},
                    'a2dp-sink-sbc': {'priority': 10},
                    'a2dp-sink-aptx_hd': {'priority': 50},
                    'a2dp-sink-aac': {'description': 'codec AAC', 'priority': 20},
                    'a2dp-sink': {'description': 'codec LDAC', 'priority': 1},
                    'a2dp-sink-unavailable': {'priority': 5000, 'available': False}}
        self.configure(UUIDs=[A2DP], cards=[card(profiles=profiles)], sinks=[sink()],
                       profile_errors=['a2dp-sink'])
        payload = self.assert_success(self.action())
        self.assertTrue(payload['routed'])
        self.assertEqual(payload['codec'], 'aac')
        setters = [c['args'] for c in self.audio_calls() if c['args'][0].startswith('set-')]
        self.assertEqual(setters, [['set-card-profile', 'bluez_card.dynamic', 'a2dp-sink'],
                                  ['set-card-profile', 'bluez_card.dynamic', 'a2dp-sink-aac'],
                                  ['set-default-sink', 'bluez_output.dynamic']])
        self.assertFalse(any('source' in ' '.join(args) for args in setters))
        self.assert_subscription_stopped()

    def test_dynamic_address_and_bluez_path_match_without_hardcoded_card_ids(self):
        cards = [card(properties={'device.string': 'hw:0'}),
                 card(properties={'api.bluez5.path': DEVICE})]
        cards[1]['name'] = 'discovered-card'
        target = sink(properties={'device.string': ADDRESS.lower(), 'api.bluez5.profile': 'a2dp-sink',
                                  'api.bluez5.codec': 'aac'})
        target['name'] = 'discovered-output'
        self.configure(UUIDs=[A2DP], cards=cards, sinks=[target])
        self.assertTrue(self.assert_success(self.action())['routed'])
        self.assertIn(['set-card-profile', 'discovered-card', 'a2dp-sink-aac'],
                      [c['args'] for c in self.audio_calls()])
        self.assertIn(['set-default-sink', 'discovered-output'], [c['args'] for c in self.audio_calls()])

    def test_client_and_stream_events_are_ignored_until_complete_relevant_event(self):
        events = [{'chunk': "Event 'new' on client #90\nEvent 'change' on sink-input #3\n"},
                  {'chunk': "Event 'new' on ca", 'delay': .04},
                  {'chunk': 'rd #12\n', 'delay': .04}]
        self.configure(UUIDs=[A2DP], cards_snapshots=[[], [card()]],
                       sinks_snapshots=[[], [sink()]], events=events)
        self.assertTrue(self.assert_success(self.action())['routed'])
        snapshots = self.snapshots()
        self.assertEqual([c['args'][-1] for c in snapshots], ['cards', 'sinks', 'cards', 'sinks'])
        self.assertEqual(snapshots[2]['event_parts'], 3)
        self.assert_subscription_stopped()

    def test_relevant_sink_and_server_events_trigger_snapshots_without_polling(self):
        for kind in ('sink', 'server'):
            with self.subTest(kind=kind):
                self.clear_fixture_state()
                self.configure(UUIDs=[A2DP], cards_snapshots=[[], [card()]],
                               sinks_snapshots=[[], [sink()]],
                               events=[{'chunk': "Event 'change' on " + kind + ' #4\n', 'delay': .08}])
                self.assertTrue(self.assert_success(self.action())['routed'])
                self.assertEqual(len(self.snapshots()), 4)
                self.assert_subscription_stopped()

    def test_subscription_eof_reports_not_ready_without_repeated_snapshots(self):
        self.configure(UUIDs=[A2DP], subscribe_eof=True)
        payload = self.assert_success(self.action())
        self.assertFalse(payload['routed'])
        self.assertIn('unavailable', payload['warning'])
        self.assertEqual(len(self.snapshots()), 2)
        self.assert_subscription_stopped()

    def test_audio_budget_expires_without_polling_and_subscription_is_reaped(self):
        self.configure(UUIDs=[A2DP])
        start = time.monotonic()
        payload = self.assert_success(self.action(timeout=16))
        elapsed = time.monotonic() - start
        self.assertFalse(payload['routed'])
        self.assertIn('not ready', payload['warning'])
        self.assertGreater(elapsed, 11.5)
        self.assertLess(elapsed, 15)
        self.assertEqual(len(self.snapshots()), 2)
        self.assert_subscription_stopped()

    def test_oversized_partial_event_line_is_bounded_and_subscription_is_reaped(self):
        self.configure(UUIDs=[A2DP], events=[{'chunk': 'x' * (64 * 1024 + 1)}])
        payload = self.assert_success(self.action(timeout=4))
        self.assertIn('audio', payload['warning'])
        self.assertEqual(len(self.snapshots()), 2)
        self.assert_subscription_stopped()

    def test_connected_device_keeps_success_when_audio_queries_or_route_fail(self):
        for fixture in ({'snapshot_error': 'cards'}, {'cards_output': 'invalid json'},
                        {'cards_output': '{}'}, {'cards': [card()], 'sinks': [sink()], 'route_error': True}):
            with self.subTest(fixture=fixture):
                self.clear_fixture_state()
                self.configure(UUIDs=[A2DP], **fixture)
                payload = self.assert_success(self.action())
                self.assertTrue(payload['connected'])
                self.assertIn('audio', payload['warning'])
                self.assert_subscription_stopped()

    def test_audio_snapshot_larger_than_default_command_limit_is_supported(self):
        unrelated = [{'name': 'unrelated', 'properties': {'device.string': 'hw:0'},
                      'description': 'x' * 512} for _ in range(180)]
        self.configure(UUIDs=[A2DP], cards=unrelated + [card()], sinks=[sink()])
        self.assertTrue(self.assert_success(self.action())['routed'])
        self.assert_subscription_stopped()

    def test_oversized_audio_snapshot_fails_finitely_and_cleans_subscription(self):
        self.configure(UUIDs=[A2DP], cards_output=' ' * (2 * 1024 * 1024 + 1) + '[]')
        payload = self.assert_success(self.action(timeout=4))
        self.assertIn('audio', payload['warning'])
        self.assert_subscription_stopped()

    def test_explicit_sbc_filters_profiles_and_waits_for_fresh_a2dp_sink(self):
        profiles = {'a2dp-sink': {'description': 'codec LDAC', 'priority': 999},
                    'a2dp-sink-aac': {'priority': 888},
                    'a2dp-sink-sbc_xq': {'priority': 777},
                    'a2dp-sink-sbc': {'description': 'codec SBC', 'priority': 1}}
        sbc_card = card(profile='a2dp-sink-sbc', profiles=profiles)
        stale = sink(codec='sbc', profile='headset-head-unit', name='stale-headset')
        self.configure(Connected=True, cards=[sbc_card], sinks_snapshots=[[stale], [sink(codec='sbc')]],
                       events=[{'chunk': "Event 'new' on sink #21\n"}])
        payload = self.assert_success(self.invoke('codec', DEVICE, '--codec', 'sbc'))
        self.assertEqual(payload, {'success': True, 'routed': True, 'codec': 'sbc'})
        setters = [c['args'] for c in self.audio_calls() if c['args'][0].startswith('set-')]
        self.assertEqual(setters, [['set-card-profile', 'bluez_card.dynamic', 'a2dp-sink-sbc'],
                                  ['set-default-sink', 'bluez_output.dynamic']])
        self.assertEqual(len(self.snapshots()), 4)
        self.assert_subscription_stopped()

    def test_stale_active_profile_and_wrong_codec_sink_are_not_routed(self):
        sbc_card = card(profile='a2dp-sink-sbc', profiles={'a2dp-sink-sbc': {'priority': 10}})
        stale_card = dict(sbc_card, active_profile='headset-head-unit')
        self.configure(Connected=True, cards_snapshots=[[stale_card], [sbc_card], [sbc_card]],
                       sinks_snapshots=[[sink(codec='sbc')], [sink(codec='aac')], [sink(codec='sbc')]],
                       events=[{'chunk': "Event 'change' on card #21\n", 'after_snapshots': 2},
                               {'chunk': "Event 'change' on sink #21\n", 'after_snapshots': 4}])
        self.assertTrue(self.assert_success(self.invoke('codec', DEVICE, '--codec', 'sbc'))['routed'])
        self.assertEqual(len(self.snapshots()), 6)
        routes = [c for c in self.audio_calls() if c['args'][0] == 'set-default-sink']
        self.assertEqual(len(routes), 1)
        self.assertEqual(routes[0]['event_parts'], 2)
        self.assert_subscription_stopped()

    def test_missing_matching_card_does_not_allow_stale_selected_profile_sink(self):
        sbc_card = card(profile='a2dp-sink-sbc', profiles={'a2dp-sink-sbc': {'priority': 10}})
        self.configure(Connected=True, cards_snapshots=[[sbc_card], []],
                       sinks_snapshots=[[], [sink(codec='sbc')]], subscribe_eof=True,
                       events=[{'chunk': "Event 'change' on card #21\n"}])
        result = self.invoke('codec', DEVICE, '--codec', 'sbc')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_unsuccessful(result)
        self.assertFalse(any(c['args'][0] == 'set-default-sink' for c in self.audio_calls()))
        self.assertEqual(len(self.snapshots()), 4)
        self.assert_subscription_stopped()

    def test_rejected_explicit_sbc_request_does_not_route_existing_ldac(self):
        self.configure(Connected=True,
                       cards=[card(profile='a2dp-sink', profiles={'a2dp-sink-sbc': {'priority': 10}})],
                       sinks=[sink(codec='ldac')], profile_errors=['a2dp-sink-sbc'], subscribe_eof=True)
        result = self.invoke('codec', DEVICE, '--codec', 'sbc')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_unsuccessful(result)
        self.assertFalse(any(c['args'][0] == 'set-default-sink' for c in self.audio_calls()))
        self.assert_subscription_stopped()

    def test_rejected_sbc_profile_request_can_route_already_matching_sbc_playback(self):
        self.configure(Connected=True,
                       cards=[card(profile='a2dp-sink-sbc', profiles={'a2dp-sink-sbc': {'priority': 10}})],
                       sinks=[sink(codec='sbc')], profile_errors=['a2dp-sink-sbc'])
        payload = self.assert_success(self.invoke('codec', DEVICE, '--codec', 'sbc'))
        self.assertEqual(payload['codec'], 'sbc')
        self.assertTrue(payload['routed'])
        self.assertIn(['set-default-sink', 'bluez_output.dynamic'], [c['args'] for c in self.audio_calls()])
        self.assert_subscription_stopped()

    def test_sbc_xq_canonical_alias_and_sink_spellings_select_only_xq_profile(self):
        profiles = {'a2dp-sink-sbc': {'description': 'codec SBC', 'priority': 9999},
                    'a2dp-sink-sbc-xq': {'description': 'codec SBC XQ', 'priority': 1}}
        for requested, reported in (('sbc_xq', 'sbc_xq'), ('sbc-xq', 'SBC-XQ'),
                                    ('sbc_xq', 'SBC XQ'), ('sbc-xq', 'SBC_XQ')):
            with self.subTest(requested=requested, reported=reported):
                self.clear_fixture_state()
                self.configure(Connected=True, cards=[card(profile='a2dp-sink-sbc-xq', profiles=profiles)],
                               sinks=[sink(codec=reported)])
                payload = self.assert_success(self.invoke('codec', DEVICE, '--codec', requested))
                self.assertEqual(payload, {'success': True, 'routed': True, 'codec': reported})
                setters = [c['args'] for c in self.audio_calls() if c['args'][0].startswith('set-')]
                self.assertEqual(setters, [['set-card-profile', 'bluez_card.dynamic', 'a2dp-sink-sbc-xq'],
                                          ['set-default-sink', 'bluez_output.dynamic']])
                self.assert_subscription_stopped()

    def test_rejected_sbc_xq_profile_does_not_route_existing_regular_sbc(self):
        self.configure(Connected=True,
                       cards=[card(profile='a2dp-sink-sbc', profiles={
                           'a2dp-sink-sbc': {'description': 'codec SBC', 'priority': 9999},
                           'a2dp-sink-sbc_xq': {'description': 'codec SBC XQ', 'priority': 1}})],
                       sinks=[sink(codec='sbc')], profile_errors=['a2dp-sink-sbc_xq'], subscribe_eof=True)
        result = self.invoke('codec', DEVICE, '--codec', 'sbc_xq')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_unsuccessful(result)
        self.assertEqual([c['args'] for c in self.audio_calls() if c['args'][0].startswith('set-')],
                         [['set-card-profile', 'bluez_card.dynamic', 'a2dp-sink-sbc_xq']])
        self.assert_subscription_stopped()

    def test_sbc_unavailable_and_disconnected_device_report_error_without_other_profile_changes(self):
        self.configure(Connected=True, cards=[card()])
        result = self.invoke('codec', DEVICE, '--codec', 'sbc')
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = self.assert_unsuccessful(result)
        self.assertIn('SBC', payload['error'])
        self.assertFalse(any(c['args'][0].startswith('set-') for c in self.audio_calls()))
        self.assert_subscription_stopped()
        self.clear_fixture_state()
        self.configure(Connected=False)
        self.assert_failure(self.invoke('codec', DEVICE, '--codec', 'sbc'))
        self.assertEqual(self.audio_calls(), [])

    def test_codec_eof_and_audio_failure_are_unsuccessful_actions(self):
        for fixture in ({'subscribe_eof': True}, {'cards_output': 'invalid json'}):
            with self.subTest(fixture=fixture):
                self.clear_fixture_state()
                self.configure(Connected=True, **fixture)
                result = self.invoke('codec', DEVICE, '--codec', 'sbc')
                if fixture.get('subscribe_eof'):
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assert_unsuccessful(result)
                else:
                    self.assert_failure(result)
                self.assert_subscription_stopped()

    def test_reconnect_waits_for_earbud_restart_then_uses_bounded_native_attempt(self):
        start = time.monotonic()
        payload = self.assert_success(self.action('reconnect', timeout=9))
        elapsed = time.monotonic() - start
        self.assertTrue(payload['connected'])
        self.assertGreater(elapsed, 4.9)
        self.assertLess(elapsed, 8)
        self.assertEqual(len(self.bus_calls()), 1)
        self.assertEqual(self.bus_calls()[0]['args'], ['--system', '--timeout=10s', 'call',
                         'org.bluez', DEVICE, 'org.bluez.Device1', 'Connect'])
        self.assertEqual(self.audio_calls(), [])

    def test_invalid_actions_paths_and_codec_arguments_have_no_native_side_effects(self):
        fixtures = [('connect',), ('connect', DEVICE, 'extra'), ('disconnect', DEVICE, '--codec', 'sbc'),
                    ('connect', '/org/bluez/custom/dev_AB_CD_EF_01_02_03'),
                    ('connect', DEVICE + ';sh'), ('codec', DEVICE),
                    ('codec', DEVICE, '--codec', 'ldac'), ('codec', DEVICE, '--codec', 'sbc', 'extra'),
                    ('unknown', DEVICE), ('reconnect', DEVICE, '--codec', 'sbc')]
        for args in fixtures:
            with self.subTest(args=args):
                result = self.invoke(*args)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])
                self.assertNotIn('panicked', result.stderr)

    def test_legacy_action_shim_forwards_all_actions_to_foundation_without_extra_commands(self):
        scripts = self.base / 'compatibility'
        scripts.mkdir()
        shim = scripts / 'bluetooth-action.py'
        shim.write_bytes((ROOT / 'scripts/bluetooth-action.py').read_bytes())
        foundation = scripts / 'foundation'
        foundation.write_text('#!' + sys.executable + '\n' + r'''
import json, os, pathlib, sys
base = pathlib.Path(os.environ['DF_BT_ACTION_FIXTURE'])
with (base / 'shim-calls.jsonl').open('a') as log:
    log.write(json.dumps(sys.argv[1:]) + '\n')
print(json.dumps({'success': True}))
''')
        foundation.chmod(0o755)
        actions = [('connect', DEVICE), ('disconnect', DEVICE), ('reconnect', DEVICE),
                   ('codec', DEVICE, '--codec', 'sbc'), ('codec', DEVICE, '--codec', 'sbc_xq'),
                   ('codecs', DEVICE)]
        for args in actions:
            with self.subTest(args=args):
                result = subprocess.run([sys.executable, str(shim), *args], env=self.env,
                                        capture_output=True, text=True, timeout=3)
                self.assert_success(result)
                self.assertEqual(self.calls(), [])
        forwarded = [json.loads(line) for line in (self.base / 'shim-calls.jsonl').read_text().splitlines()]
        self.assertEqual(forwarded, [['bluetooth', *args] for args in actions])

    @staticmethod
    def stopped(pid):
        try:
            return 'State:\tZ' in (Path('/proc') / str(pid) / 'status').read_text()
        except FileNotFoundError:
            return True

    def stop_owned(self, pid):
        if not self.stopped(pid):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def test_hard_killed_backend_stops_event_subscription(self):
        for stalled_snapshot in (None, 'sinks'):
            with self.subTest(stalled_snapshot=stalled_snapshot):
                self.clear_fixture_state()
                self.configure(UUIDs=[A2DP], stall_snapshot=stalled_snapshot)
                backend = subprocess.Popen([str(BINARY), '--root', str(ROOT), 'bluetooth', 'connect', DEVICE],
                                           env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                self.addCleanup(lambda backend=backend: backend.poll() is None and backend.kill())
                deadline = time.monotonic() + 5
                while len(self.snapshots()) < 2 and time.monotonic() < deadline:
                    time.sleep(.01)
                self.assertEqual(len(self.snapshots()), 2)
                subscriptions = [c for c in self.audio_calls() if c['args'] == ['subscribe']]
                self.assertEqual(len(subscriptions), 1)
                pids = [c['pid'] for c in self.calls()]
                for pid in pids:
                    self.addCleanup(lambda pid=pid: self.stop_owned(pid))
                backend.kill()
                backend.wait(timeout=3)
                deadline = time.monotonic() + 2
                while any(not self.stopped(pid) for pid in pids) and time.monotonic() < deadline:
                    time.sleep(.01)
                self.assertTrue(all(self.stopped(pid) for pid in pids),
                                'Owned audio subscription/query survived destroyed backend')


if __name__ == '__main__':
    unittest.main()
