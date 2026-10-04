"""Screenshot actions against private command fixtures, never a real desktop."""
import fcntl
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile
import time
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'


def png(extra=0):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
            + (chunk(b'tEXt', b'Fixture\0' + b'x' * extra) if extra else b'')
            + chunk(b'IDAT', zlib.compress(b'\0\x11\x22\x33\xff'))
            + chunk(b'IEND', b''))


FAKE = r'''
import json, os, pathlib, sys, time
name = pathlib.Path(sys.argv[0]).name
base = pathlib.Path(os.environ['DF_SCREENSHOT_FIXTURE'])
config = json.loads((base / 'config.json').read_text())
call = {'program': name, 'args': sys.argv[1:], 'pid': os.getpid()}
if name == 'slurp':
    call['stdin_hex'] = sys.stdin.buffer.read().hex()
with (base / 'calls.jsonl').open('a') as log:
    log.write(json.dumps(call) + '\n')
if name == 'slurp':
    time.sleep(config.get('selection_delay', 0))
    sys.stdout.write(config.get('geometry', '-20,10 80x60\n'))
    sys.stderr.write(config.get('selection_error', ''))
    sys.exit(config.get('selection_status', 0))
elif name == 'grim':
    if config.get('capture_error'):
        print('capture fixture failed', file=sys.stderr); sys.exit(1)
    destination = sys.argv[-1]
    if destination != '-' and config.get('capture_fifo'):
        os.mkfifo(destination)
    elif destination != '-' and config.get('capture_sparse_size'):
        with open(destination, 'wb') as out:
            out.truncate(config['capture_sparse_size'])
    else:
        image = bytes.fromhex(config['capture_hex']) if 'capture_hex' in config else (base / 'image.png').read_bytes()
        if destination == '-':
            sys.stdout.buffer.write(image)
        else:
            pathlib.Path(destination).write_bytes(image)
elif name == 'wl-copy':
    image = sys.stdin.buffer.read()
    if config.get('copy_error'):
        print('clipboard fixture failed', file=sys.stderr); sys.exit(1)
    (base / 'clipboard').write_bytes(image)
elif name == 'hyprctl':
    resource = sys.argv[-1]
    if config.get('snapshot_error'):
        print('snapshot fixture failed', file=sys.stderr); sys.exit(1)
    if config.get('malformed_snapshot'):
        print('invalid json'); sys.exit(0)
    print(json.dumps(config[resource]))
elif name == 'niri':
    sys.exit(config.get('native_status', 0))
'''


class ScreenshotBackend(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        self.runtime = self.base / 'runtime'
        self.runtime.mkdir(mode=0o700)
        self.checkout = self.base / 'checkout'
        (self.checkout / 'config').mkdir(parents=True)
        (self.checkout / 'compositor/niri').mkdir(parents=True)
        (self.checkout / 'compositor/niri/screenshots.toml').write_bytes(
            (ROOT / 'compositor/niri/screenshots.toml').read_bytes())
        self.screenshot_config()
        self.image = png()
        (self.base / 'image.png').write_bytes(self.image)
        (self.base / 'clipboard').write_bytes(b'previous-clipboard')
        self.env = dict(os.environ, PATH=str(self.bin), HOME=str(self.base),
                        XDG_RUNTIME_DIR=str(self.runtime),
                        XDG_STATE_HOME=str(self.base / 'state'),
                        DF_SCREENSHOT_FIXTURE=str(self.base),
                        DF_COMPOSITOR='niri')
        # PATH contains only these fakes. Removing a fake cannot expose any
        # real compositor/capture command, even on a machine running a desktop.
        for key in ('WAYLAND_DISPLAY', 'DISPLAY', 'NIRI_SOCKET',
                    'HYPRLAND_INSTANCE_SIGNATURE', 'DBUS_SESSION_BUS_ADDRESS'):
            self.env.pop(key, None)
        for name in ('slurp', 'grim', 'wl-copy', 'hyprctl', 'niri'):
            path = self.bin / name
            path.write_text('#!' + sys.executable + '\n' + FAKE)
            path.chmod(0o755)
        self.configure()

    def screenshot_config(self, directory='~/Pictures/Screenshots', filename='Screenshot_fixed.png'):
        (self.checkout / 'config/screenshots.toml').write_text(
            'directory = ' + json.dumps(directory) + '\nfilename = ' + json.dumps(filename) + '\n')

    def configure(self, **config):
        defaults = {
            'monitors': [{'id': 1, 'name': 'DP-1', 'focused': True, 'disabled': False,
                          'width': 1920, 'height': 1080, 'scale': 1, 'transform': 0,
                          'x': 0, 'y': 0}],
            'activewindow': {'address': '0xfixture', 'mapped': True, 'hidden': False,
                             'monitor': 1, 'at': [-20, 10], 'size': [300, 200]},
        }
        defaults.update(config)
        (self.base / 'config.json').write_text(json.dumps(defaults))

    def invoke(self, *args, timeout=12, env=None):
        return subprocess.run([str(BINARY), '--root', str(self.checkout), 'screenshot', *args],
                              env=env or self.env, capture_output=True, text=True, timeout=timeout)

    def hyprland(self, action, **kwargs):
        env = dict(self.env, HYPRLAND_INSTANCE_SIGNATURE='isolated-fixture')
        return self.invoke('--backend', 'hyprland', action, env=env, **kwargs)

    def calls(self):
        path = self.base / 'calls.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def clear_calls(self):
        (self.base / 'calls.jsonl').unlink(missing_ok=True)

    def programs(self):
        return [call['program'] for call in self.calls()]

    def assert_failure(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertTrue(result.stderr)
        self.assertNotIn('panicked', result.stderr)

    def assert_clipboard_untouched(self):
        self.assertEqual((self.base / 'clipboard').read_bytes(), b'previous-clipboard')

    def saved(self):
        return self.base / 'Pictures/Screenshots/Screenshot_fixed.png'

    def assert_capture_temporaries_removed(self):
        self.assertFalse(any(path.name.startswith('foundation-capture-') for path in self.runtime.iterdir()))
        folder = self.base / 'Pictures/Screenshots'
        if folder.exists():
            self.assertFalse(any(path.name.startswith('.capture-') for path in folder.iterdir()))

    def test_niri_cancel_variants_preserve_clipboard_without_creating_captures(self):
        for status, error in ((1, ''), (1, 'selection cancelled\n'),
                              (1, 'Selection Canceled\n'), (0, '')):
            with self.subTest(status=status, error=error):
                self.configure(selection_status=status, selection_error=error, geometry='')
                self.clear_calls()
                result = self.invoke('region')
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.programs(), ['slurp'])
                self.assertEqual(self.calls()[0]['stdin_hex'], '')
                self.assert_clipboard_untouched()
                self.assertEqual([p.suffix for p in self.runtime.iterdir()], ['.lock'])

    def test_niri_selector_uses_config_and_immediately_captures_negative_geometry(self):
        result = self.invoke('--backend', 'niri', 'region')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(self.programs(), ['slurp', 'grim', 'wl-copy'])
        self.assertEqual(calls[0]['args'], ['-d', '-b', '#080b1070', '-c', '#79b8d4b0',
                         '-s', '#00000000', '-B', '#171b22ee', '-F', 'Google Sans', '-w', '1'])
        self.assertEqual(calls[0]['stdin_hex'], '')
        self.assertEqual(calls[1]['args'], ['-g', '-20,10 80x60', '-'])
        self.assertEqual(calls[2]['args'], ['--type', 'image/png'])
        self.assertEqual((self.base / 'clipboard').read_bytes(), self.image)
        self.assertEqual([p.suffix for p in self.runtime.iterdir()], ['.lock'])
        self.assertFalse((self.base / 'Pictures').exists())
        self.assertTrue(all(self.stopped(call['pid']) for call in calls),
                        'Screenshot command left a fixture child running')

    def test_region_waits_for_user_selection_beyond_native_action_five_second_budget(self):
        self.configure(selection_delay=5.2)
        result = self.invoke('region', timeout=9)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.base / 'clipboard').read_bytes(), self.image)

    def test_niri_png_larger_than_text_command_limit_is_copied_exactly(self):
        self.image = png(extra=100 * 1024)
        self.assertGreater(len(self.image), 64 * 1024)
        (self.base / 'image.png').write_bytes(self.image)
        result = self.invoke('region')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.base / 'clipboard').read_bytes(), self.image)

    def test_selector_display_errors_and_invalid_geometry_do_not_capture(self):
        failures = [dict(selection_status=1, selection_error='failed to connect to display'),
                    dict(selection_status=2, selection_error='selection cancelled'),
                    dict(geometry='10,20 0x5'), dict(geometry='10,20 5x0'),
                    dict(geometry='10,20 1x2; touch anything'), dict(geometry='invalid'),
                    dict(geometry='10,20 01x2'), dict(geometry='+10,20 1x2')]
        for config in failures:
            with self.subTest(config=config):
                self.configure(**config)
                self.clear_calls()
                self.assert_failure(self.invoke('region'))
                self.assertEqual(self.programs(), ['slurp'])
                self.assert_clipboard_untouched()

    def test_bad_or_failed_capture_never_publishes_clipboard(self):
        for config in ({'capture_hex': b'not png'.hex()}, {'capture_hex': self.image[:23].hex()},
                       {'capture_hex': (self.image[:12] + b'WRNG' + self.image[16:]).hex()},
                       {'capture_error': True}):
            with self.subTest(config=config):
                self.configure(**config)
                self.clear_calls()
                self.assert_failure(self.invoke('region'))
                self.assertEqual(self.programs(), ['slurp', 'grim'])
                self.assert_clipboard_untouched()

    def test_niri_clipboard_failure_does_not_save_an_image(self):
        self.configure(copy_error=True)
        self.assert_failure(self.invoke('region'))
        self.assert_clipboard_untouched()
        self.assertFalse((self.base / 'Pictures').exists())
        self.assertEqual([p.suffix for p in self.runtime.iterdir()], ['.lock'])

    def test_niri_window_and_output_use_clipboard_only_native_actions(self):
        for action, native in (('window', 'screenshot-window'), ('output', 'screenshot-screen')):
            with self.subTest(action=action):
                self.clear_calls()
                result = self.invoke('--backend', 'niri', action)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.programs(), ['niri'])
                self.assertEqual(self.calls()[0]['args'], ['msg', 'action', native,
                                 '--show-pointer', 'false', '--write-to-disk', 'false'])
                self.assertEqual(list(self.runtime.iterdir()), [])
        self.configure(native_status=1)
        self.assert_failure(self.invoke('window'))

    def test_niri_native_actions_need_no_selector_config_or_runtime_directory(self):
        (self.checkout / 'compositor/niri/screenshots.toml').unlink()
        env = dict(self.env)
        env.pop('XDG_RUNTIME_DIR')
        result = self.invoke('output', env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.programs(), ['niri'])

    def test_compatibility_entrypoints_forward_to_same_native_screenshot_action(self):
        scripts = self.checkout / 'scripts'
        scripts.mkdir()
        for name in ('foundation', 'screenshot', 'screenshot.py'):
            source = ROOT / 'scripts' / name
            copied = scripts / name
            copied.write_bytes(source.read_bytes())
            copied.chmod(source.stat().st_mode & 0o777)
        binary = self.checkout / 'native/foundation/target/release/desktop-foundationctl'
        binary.parent.mkdir(parents=True)
        binary.symlink_to(BINARY)
        # Only wrapper necessities are added to PATH; desktop commands still
        # resolve exclusively to the fixture executables above.
        (self.bin / 'bash').symlink_to('/usr/bin/bash')
        (self.bin / 'dirname').symlink_to('/usr/bin/dirname')
        for entrypoint in ([str(scripts / 'screenshot')],
                           [sys.executable, str(scripts / 'screenshot.py')]):
            with self.subTest(entrypoint=entrypoint):
                self.clear_calls()
                result = subprocess.run([*entrypoint, '--backend', 'niri', 'output'],
                                        env=self.env, capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.programs(), ['niri'])
                self.assertEqual(self.calls()[0]['args'], ['msg', 'action', 'screenshot-screen',
                                 '--show-pointer', 'false', '--write-to-disk', 'false'])
                self.assertEqual(list(self.runtime.iterdir()), [])
                self.assert_clipboard_untouched()

    def test_nonblocking_capture_locks_keep_duplicate_requests_noop(self):
        for backend, name in (('niri', 'desktop-foundation-region.lock'),
                              ('hyprland', 'desktop-foundation-screenshot.lock')):
            with self.subTest(backend=backend):
                self.clear_calls()
                with (self.runtime / name).open('a') as lock:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    result = self.invoke('region') if backend == 'niri' else self.hyprland('region')
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.calls(), [])
                self.assert_clipboard_untouched()

    def test_selector_lock_symlink_is_rejected_without_touching_target(self):
        target = self.base / 'unrelated-file'
        target.write_bytes(b'unchanged')
        (self.runtime / 'desktop-foundation-region.lock').symlink_to(target)
        self.assert_failure(self.invoke('region'))
        self.assertEqual(self.calls(), [])
        self.assertEqual(target.read_bytes(), b'unchanged')
        self.assert_clipboard_untouched()

    def test_hyprland_region_preserves_selector_and_private_atomic_save(self):
        result = self.hyprland('region')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(self.programs(), ['slurp', 'grim', 'wl-copy'])
        self.assertEqual(calls[0]['args'], ['-b', '#080b1080', '-c', '#b8ceeebb',
                         '-s', '#b8ceee18', '-w', '1'])
        self.assertEqual(calls[1]['args'][:-1], ['-t', 'png', '-g', '-20,10 80x60'])
        capture = Path(calls[1]['args'][-1])
        self.assertEqual(capture.parent.parent, self.runtime)
        self.assertEqual(capture.name, 'capture.png')
        self.assertEqual(self.saved().read_bytes(), self.image)
        self.assertEqual(self.saved().stat().st_mode & 0o777, 0o600)
        self.assertEqual(Path(result.stdout.strip()), self.saved())
        self.assertEqual((self.base / 'clipboard').read_bytes(), self.image)
        self.assert_capture_temporaries_removed()

    def test_hyprland_cancel_is_distinguished_from_display_failure_and_empty_region(self):
        self.configure(selection_status=1, selection_error='selection cancelled\n')
        result = self.hyprland('region')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.programs(), ['slurp'])
        self.assert_capture_temporaries_removed()
        for config in ({'selection_status': 1, 'selection_error': ''},
                       {'selection_status': 1, 'selection_error': 'display unavailable'},
                       {'geometry': ''}):
            with self.subTest(config=config):
                self.configure(**config)
                self.clear_calls()
                self.assert_failure(self.hyprland('region'))
                self.assertEqual(self.programs(), ['slurp'])
                self.assert_clipboard_untouched()
                self.assert_capture_temporaries_removed()

    def test_hyprland_output_selects_only_focused_enabled_monitor(self):
        self.configure(monitors=[{'focused': True, 'disabled': True, 'name': 'Disabled'},
                                 {'focused': False, 'name': 'Other'},
                                 {'focused': True, 'disabled': False, 'name': 'DP-3'}])
        result = self.hyprland('output')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.programs(), ['hyprctl', 'grim', 'wl-copy'])
        self.assertEqual(self.calls()[0]['args'], ['-j', 'monitors'])
        self.assertEqual(self.calls()[1]['args'][:-1], ['-t', 'png', '-o', 'DP-3'])

    def test_hyprland_window_clips_to_visible_monitor_bounds(self):
        result = self.hyprland('window')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.programs(), ['hyprctl', 'hyprctl', 'grim', 'wl-copy'])
        self.assertEqual([call['args'] for call in self.calls()[:2]],
                         [['-j', 'activewindow'], ['-j', 'monitors']])
        self.assertEqual(self.calls()[2]['args'][:-1], ['-t', 'png', '-g', '0,10 280x200'])

    def test_hyprland_transformed_scaled_window_rounds_and_clips_visible_pixels(self):
        self.configure(monitors=[{'id': 1, 'name': 'Rotated', 'width': 1920, 'height': 1080,
                                 'scale': 1.5, 'transform': 1, 'x': -500, 'y': 100}],
                       activewindow={'address': '0xfixture', 'mapped': True, 'hidden': False,
                                     'monitor': 1, 'at': [-550.4, 110.4], 'size': [900.9, 1500.5]})
        result = self.hyprland('window')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls()[2]['args'][:-1], ['-t', 'png', '-g', '-500,111 720x1269'])

    def test_hyprland_absent_or_hidden_window_and_missing_output_do_not_capture(self):
        fixtures = [('window', {'activewindow': {}}),
                    ('window', {'activewindow': {'address': '0xfixture', 'mapped': False}}),
                    ('window', {'activewindow': {'address': '0xfixture', 'mapped': True, 'hidden': True}}),
                    ('window', {'monitors': []}), ('output', {'monitors': []}),
                    ('output', {'monitors': [{'name': 'DP-1', 'focused': False}]}),
                    ('window', {'activewindow': {'address': '0xfixture', 'mapped': True, 'monitor': 1,
                                                 'at': [-500, 0], 'size': [50, 50]}})]
        for action, config in fixtures:
            with self.subTest(action=action, config=config):
                self.configure(**config)
                self.clear_calls()
                self.assert_failure(self.hyprland(action))
                self.assertNotIn('grim', self.programs())
                self.assert_clipboard_untouched()
                self.assert_capture_temporaries_removed()

    def test_hyprland_bad_snapshot_and_failed_or_bad_capture_do_not_save_or_copy(self):
        for config in ({'snapshot_error': True}, {'malformed_snapshot': True},
                       {'capture_error': True}, {'capture_hex': b'not png'.hex()},
                       {'capture_fifo': True},
                       {'capture_sparse_size': 128 * 1024 * 1024 + 1}):
            with self.subTest(config=config):
                self.configure(**config)
                self.clear_calls()
                self.assert_failure(self.hyprland('output'))
                self.assertNotIn('wl-copy', self.programs())
                self.assertFalse(self.saved().exists())
                self.assert_capture_temporaries_removed()

    def test_hyprland_filename_collision_never_overwrites_or_copies_new_capture(self):
        self.saved().parent.mkdir(parents=True)
        self.saved().write_bytes(b'existing-image')
        self.assert_failure(self.hyprland('output'))
        self.assertEqual(self.saved().read_bytes(), b'existing-image')
        self.assertNotIn('wl-copy', self.programs())
        self.assert_clipboard_untouched()
        self.assert_capture_temporaries_removed()

    def test_hyprland_copy_failure_keeps_completed_private_capture_and_reports_path(self):
        self.configure(copy_error=True)
        result = self.hyprland('output')
        self.assert_failure(result)
        self.assertIn(str(self.saved()), result.stderr)
        self.assertIn('saved', result.stderr.lower())
        self.assertEqual(self.saved().read_bytes(), self.image)
        self.assert_clipboard_untouched()
        self.assert_capture_temporaries_removed()

    def test_storage_config_rejects_relative_directory_and_filename_escape(self):
        for directory, filename in (('relative', 'safe.png'), ('~/Pictures/Screenshots', '../escape.png'),
                                    ('~/Pictures/Screenshots', '/absolute.png'),
                                    ('~/Pictures/Screenshots', 'not-png.jpg')):
            with self.subTest(directory=directory, filename=filename):
                self.screenshot_config(directory, filename)
                self.clear_calls()
                self.assert_failure(self.hyprland('output'))
                self.assertNotIn('wl-copy', self.programs())
                self.assert_clipboard_untouched()
                self.assertFalse((self.base / 'Pictures/escape.png').exists())
                self.assert_capture_temporaries_removed()

    def test_microsecond_filename_format_and_escaped_percent_are_preserved(self):
        self.screenshot_config(filename='Capture_%%f_%f.png')
        result = self.hyprland('output')
        self.assertEqual(result.returncode, 0, result.stderr)
        path = Path(result.stdout.strip())
        self.assertRegex(path.name, r'^Capture_%f_\d{6}\.png$')
        self.assertEqual(path.read_bytes(), self.image)

    def test_invalid_cli_and_absent_hyprland_session_do_not_spawn_commands(self):
        for args in ((), ('--backend', 'invalid', 'region'), ('unknown',),
                     ('region', 'extra'), ('--backend',), ('--backend', 'niri')):
            with self.subTest(args=args):
                self.assert_failure(self.invoke(*args))
                self.assertEqual(self.calls(), [])
        self.assert_failure(self.invoke('--backend', 'hyprland', 'output'))
        self.assertEqual(self.calls(), [])

    def test_default_backend_honors_compositor_and_explicit_backend_overrides_it(self):
        env = dict(self.env, DF_COMPOSITOR='hyprland', HYPRLAND_INSTANCE_SIGNATURE='isolated-fixture')
        result = self.invoke('output', env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.programs(), ['hyprctl', 'grim', 'wl-copy'])
        self.clear_calls()
        result = self.invoke('--backend', 'niri', 'window', env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.programs(), ['niri'])

    def test_malformed_toml_is_reported_before_selector_or_capture(self):
        for backend, file in (('niri', self.checkout / 'compositor/niri/screenshots.toml'),
                              ('hyprland', self.checkout / 'config/screenshots.toml')):
            with self.subTest(backend=backend):
                file.write_text('broken = [this is not toml\n')
                result = self.invoke('region') if backend == 'niri' else self.hyprland('output')
                self.assert_failure(result)
                self.assertEqual(self.calls(), [])
                self.assert_clipboard_untouched()

    def test_missing_capture_executable_does_not_fall_through_to_real_desktop(self):
        (self.bin / 'grim').unlink()
        self.assert_failure(self.invoke('region'))
        self.assertEqual(self.programs(), ['slurp'])
        self.assert_clipboard_untouched()

    @staticmethod
    def stopped(pid):
        try:
            return 'State:\tZ' in (Path('/proc') / str(pid) / 'status').read_text()
        except (FileNotFoundError, ProcessLookupError):
            return True

    def stop_owned(self, pid):
        if not self.stopped(pid):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def test_cancelled_backend_stops_stalled_selector_owned_child(self):
        self.configure(selection_delay=30)
        backend = subprocess.Popen([str(BINARY), '--root', str(self.checkout), 'screenshot', 'region'],
                                   env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(lambda: backend.poll() is None and backend.kill())
        deadline = time.monotonic() + 3
        while not self.calls() and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertTrue(self.calls(), 'Selector fixture did not start')
        pid = self.calls()[0]['pid']
        self.addCleanup(lambda: self.stop_owned(pid))
        backend.kill()
        backend.wait(timeout=3)
        deadline = time.monotonic() + 2
        while not self.stopped(pid) and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertTrue(self.stopped(pid), 'Owned selector survived backend cancellation')
        self.assert_clipboard_untouched()


if __name__ == '__main__':
    unittest.main()
