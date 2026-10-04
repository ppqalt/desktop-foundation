#!/usr/bin/env python3
"""Finite BlueZ action + event-driven PipeWire-Pulse playback setup; no daemon."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
import time

ENV = dict(os.environ, LC_ALL='C')
PARENT_PID = os.getpid()


def parent_death():
    # Quickshell may kill a destroyed Process immediately. Its children must also
    # stop even if Python's finally blocks cannot run (Linux/Arch target).
    if ctypes.CDLL(None).prctl(1, signal.SIGTERM, 0, 0, 0) != 0:
        os._exit(1)
    if os.getppid() != PARENT_PID:
        os._exit(1)


def command(*args, timeout=8):
    process = subprocess.Popen(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=ENV, preexec_fn=parent_death)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
        if process.returncode:
            raise subprocess.CalledProcessError(process.returncode, args, stdout, stderr)
        return stdout.strip()
    except BaseException:
        if process.poll() is None:
            process.kill()
        process.wait()
        raise


def property_value(path, name):
    return json.loads(command('busctl', '--system', '--json=short', 'get-property',
                              'org.bluez', path, 'org.bluez.Device1', name))['data']


def snapshot(kind):
    return json.loads(command('pactl', '--format=json', 'list', kind))


def address_of(item):
    props = item.get('properties', {})
    address = props.get('api.bluez5.address') or props.get('device.string', '')
    if re.fullmatch(r'(?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}', address):
        return address.upper()
    # Older PipeWire versions provide the dynamically discovered BlueZ path.
    path = props.get('api.bluez5.path', '')
    match = re.search(r'/dev_((?:[0-9A-Fa-f]{2}_){5}[0-9A-Fa-f]{2})$', path)
    return match[1].replace('_', ':').upper() if match else ''


def profile_codec(name, info):
    hint = (name + ' ' + info.get('description', '')).lower()
    if 'ldac' in hint:
        return 'ldac'
    if 'aac' in hint:
        return 'aac'
    if any(marker in hint for marker in ['sbc_xq', 'sbc-xq', 'sbc xq']):
        return 'sbc_xq'
    return 'sbc' if 'sbc' in hint else ''


def profiles(card, requested_codec=None):
    candidates = []
    for name, info in card.get('profiles', {}).items():
        if not name.startswith('a2dp-sink') or info.get('available') in (False, 'no'):
            continue
        codec = profile_codec(name, info)
        if requested_codec and codec != requested_codec:
            continue
        rank = 0 if codec == 'ldac' else 1 if codec == 'aac' else 2
        candidates.append((rank, -info.get('priority', 0), name))
    return [name for _, _, name in sorted(candidates)]


def codec_of(sink):
    return sink.get('properties', {}).get('api.bluez5.codec', '')


def configure_audio(address, requested_codec=None):
    # Subscribe before the first snapshot. Wait on events, never repeated timers.
    events = subprocess.Popen(['pactl', 'subscribe'], stdout=subprocess.PIPE,
                              stderr=subprocess.DEVNULL, env=ENV, preexec_fn=parent_death)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(events.stdout, selectors.EVENT_READ)
            deadline = time.monotonic() + 12
            attempted = set()
            selected_profile = ''
            expected_codec = ''
            buffered = b''
            while True:
                cards = [c for c in snapshot('cards') if address_of(c) == address]
                if cards and not selected_profile:
                    card = cards[0]
                    candidates = profiles(card, requested_codec)
                    if requested_codec and not candidates:
                        return {'routed': False, 'error': 'This device does not currently offer ' + requested_codec.upper() + ' playback.'}
                    for profile in candidates:
                        if profile in attempted:
                            continue
                        attempted.add(profile)
                        try:
                            command('pactl', 'set-card-profile', card['name'], profile)
                            selected_profile = profile
                            hint = (profile + ' ' + card['profiles'][profile].get('description', '')).lower()
                            expected_codec = requested_codec or ('ldac' if 'ldac' in hint else 'aac' if 'aac' in hint else '')
                            break
                        except (subprocess.SubprocessError, OSError):
                            continue  # Codec preference must never prevent connection.
                sinks = [s for s in snapshot('sinks') if address_of(s) == address]
                # Do not route a stale headset sink while the A2DP node is replacing it.
                sinks = [s for s in sinks if not selected_profile or
                         (s.get('properties', {}).get('api.bluez5.profile') == 'a2dp-sink' and
                          (not cards or cards[0].get('active_profile') == selected_profile) and
                          (not expected_codec or codec_of(s).lower() == expected_codec))]
                if sinks:
                    command('pactl', 'set-default-sink', sinks[0]['name'])
                    return {'codec': codec_of(sinks[0]), 'routed': True}
                while True:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0 or not selector.select(remaining):
                        return {'routed': False, 'warning': 'Connected; audio output is not ready yet.'}
                    event = os.read(events.stdout.fileno(), 65536)
                    if not event:
                        return {'routed': False, 'warning': 'Connected; audio service is unavailable.'}
                    buffered += event
                    lines = buffered.split(b'\n')
                    buffered = lines.pop()
                    # Snapshot queries create client events themselves. Ignore those,
                    # streams and captures: only real card/sink/server changes matter.
                    if any(re.search(rb' on (?:card|sink|server) #', line) for line in lines):
                        break
    finally:
        events.terminate()
        try:
            events.wait(timeout=2)
        except subprocess.TimeoutExpired:
            events.kill()
            events.wait()


def friendly_error(error, action="connect"):
    text = str(error).lower()
    if 'notready' in text or 'not ready' in text:
        return 'Bluetooth is turned off or unavailable.'
    if 'blocked' in text:
        return 'This device is blocked. Check Bluetooth Manager.'
    if 'inprogress' in text:
        return 'A connection is already in progress. Try again shortly.'
    if 'authentication' in text or 'notauthorized' in text:
        return 'Connection was refused. Check the device in Bluetooth Manager.'
    if action in ('codec', 'codecs'):
        return 'Could not change the playback codec. Check the device and audio service.'
    if action == 'disconnect':
        return 'Could not disconnect. Try again or open Bluetooth Manager.'
    return 'Could not connect. Make sure the device is on and nearby.'


def main():
    # Discovery is finite Rust code; connection/audio routing stays event-driven.
    if len(sys.argv) > 1 and sys.argv[1] == 'codecs':
        wrapper = str(Path(__file__).resolve().with_name('foundation'))
        os.execv(wrapper, [wrapper, 'bluetooth', *sys.argv[1:]])
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(SystemExit(0)))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['connect', 'disconnect', 'codec', 'reconnect'])
    parser.add_argument('path')
    parser.add_argument('--codec', choices=['sbc'])
    args = parser.parse_args()
    if not re.fullmatch(r'/org/bluez/[^/]+/dev_(?:[0-9A-Fa-f]{2}_){5}[0-9A-Fa-f]{2}', args.path):
        parser.error('Invalid BlueZ device path')
    try:
        if not (property_value(args.path, 'Paired') or property_value(args.path, 'Bonded')):
            raise RuntimeError('Device is not paired')
        if args.action == 'codec':
            address = property_value(args.path, 'Address').upper()
            if not args.codec:
                raise RuntimeError('No codec selected')
            if not property_value(args.path, 'Connected'):
                raise RuntimeError('Device disconnected')
            audio = configure_audio(address, args.codec)
            result = {'success': bool(audio.get('routed')), **audio}
            if not result['success']:
                result['error'] = audio.get('error') or audio.get('warning') or 'Playback codec could not be confirmed.'
        elif args.action in ('connect', 'reconnect'):
            if not json.loads(command('busctl', '--system', '--json=short', 'get-property', 'org.bluez', args.path.rsplit('/', 1)[0], 'org.bluez.Adapter1', 'Powered'))['data']:
                raise RuntimeError('Bluetooth not ready')
            if property_value(args.path, 'Blocked'):
                raise RuntimeError('Device blocked')
            uuids = property_value(args.path, 'UUIDs')
            address = property_value(args.path, 'Address').upper()
            if args.action == 'reconnect':
                # Codec preference changes reboot the earbuds. Give that restart
                # time to finish, then make a bounded set of connection attempts.
                time.sleep(5)
                deadline = time.monotonic() + 30
                while True:
                    try:
                        command('busctl', '--system', '--timeout=10s', 'call', 'org.bluez', args.path,
                                'org.bluez.Device1', 'Connect', timeout=12)
                        if property_value(args.path, 'Connected'):
                            break
                        raise RuntimeError('Device has not reconnected yet')
                    except (subprocess.SubprocessError, RuntimeError):
                        if time.monotonic() >= deadline:
                            raise
                        time.sleep(2)
            else:
                command('busctl', '--system', '--timeout=40s', 'call', 'org.bluez', args.path,
                        'org.bluez.Device1', 'Connect', timeout=45)
            if not property_value(args.path, 'Connected'):
                raise RuntimeError('BlueZ did not confirm connection')
            result = {'success': True, 'connected': True}
            if '0000110b-0000-1000-8000-00805f9b34fb' in uuids:
                try:
                    result.update(configure_audio(address))
                except (OSError, subprocess.SubprocessError, ValueError):
                    result['warning'] = 'Connected; audio could not be switched automatically.'
            if not property_value(args.path, 'Connected'):
                raise RuntimeError('Device disconnected while connecting')
        else:
            command('busctl', '--system', '--timeout=15s', 'call', 'org.bluez', args.path,
                    'org.bluez.Device1', 'Disconnect', timeout=20)
            if property_value(args.path, 'Connected'):
                raise RuntimeError('BlueZ did not confirm disconnection')
            result = {'success': True, 'connected': False}
        print(json.dumps(result), flush=True)
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        raw = error.stderr if isinstance(error, subprocess.CalledProcessError) else str(error)
        print(json.dumps({'success': False, 'error': friendly_error(raw, args.action)}), flush=True)
        raise SystemExit(1)


if __name__ == '__main__':
    main()
