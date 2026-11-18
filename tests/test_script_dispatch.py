"""Public command compatibility, using an inert backend and private checkout."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
PAYLOAD = ['two words', '', '--literal=$HOME;$(false)', 'ääkköset']
DIRECT = ('baseline boot-report deploy dns doctor launcher-bench lock-screen '
          'niri-package preflight session-acceptance shell-profile startup '
          'system-setup trace volume').split()
PREFIXES = {
    **{name: [name] for name in DIRECT},
    'applications': ['app-roles'], 'power-action': ['power'],
    'preferences': ['preferences-setup'], 'failure': ['failure-check'],
    'geode-setup': ['geode'], 'record': ['recording'],
    'shell-ipc': ['shell', 'call'],
    **{f'theme-{action}': ['theme-runtime', action]
       for action in ['apply', 'generate', 'promote', 'reset', 'rollback']},
    'wallpaper-set': ['theme-runtime', 'wallpaper'],
    **{name: [name] for name in ['control-center', 'monitor', 'transparency', 'local-ai']},
    'install-app-mods': ['app-mods'],
}
FIXED = {
    **{name: ['shell', 'call', method] for name, method in {
        'bluetooth-popup': 'toggleBluetooth', 'clipboard': 'toggleClipboard',
        'clock': 'toggleClock', 'launcher': 'toggleLauncher', 'power-menu': 'togglePower',
    }.items()},
    'clipboard-watch': ['clipboard', 'watch'],
    'niri-notifications-start': ['notifications', 'start'],
    'niri-wallpaper-start': ['wallpaper', 'start'],
    'notification-owner-check': ['notifications', 'check-owner'],
}
WORKFLOWS = ['bench', 'format', 'lint', 'reload', 'restore', 'test']


class ScriptDispatch(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='foundation dispatch ')
        self.root = Path(self.directory.name)
        self.scripts = self.root / 'scripts'
        self.scripts.mkdir()
        self.executable(self.scripts / 'foundation', (ROOT / 'scripts/foundation').read_text())
        self.backend = self.root / 'native/foundation/target/release/desktop-foundationctl'
        self.backend.parent.mkdir(parents=True)
        fake = '#!' + sys.executable + '\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n'
        self.executable(self.backend, fake)
        self.executable(self.scripts / 'dev', fake)
        for name in [*PREFIXES, *FIXED, *WORKFLOWS]:
            source = ROOT / 'scripts' / name
            self.assertTrue(source.is_symlink(), name)
            self.assertEqual(source.readlink(), Path('foundation'))
            (self.scripts / name).symlink_to('foundation')

    def tearDown(self):
        self.directory.cleanup()

    def executable(self, path, content):
        path.write_text(content)
        path.chmod(0o755)

    def run_command(self, name, arguments=(), **kwargs):
        return subprocess.run([str(self.scripts / name), *arguments],
                              text=True, capture_output=True, timeout=5, **kwargs)

    def assert_route(self, name, arguments, expected):
        result = self.run_command(name, arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), ['--root', str(self.root), *expected])

    def test_all_runtime_aliases_preserve_arguments_without_shell_interpretation(self):
        for name, prefix in PREFIXES.items():
            with self.subTest(name=name):
                self.assert_route(name, PAYLOAD, [*prefix, *PAYLOAD])
        for name, expected in FIXED.items():
            with self.subTest(name=name):
                self.assert_route(name, PAYLOAD, expected)
        self.assert_route('applications', ['launch', *PAYLOAD], ['apps', 'launch', *PAYLOAD])
        self.assert_route('foundation', ['config', 'desktop'], ['config', 'desktop'])

    def test_defaults_and_missing_ipc_method(self):
        for name in ['control-center', 'monitor', 'transparency']:
            self.assert_route(name, [], [name, 'toggle'])
        self.assert_route('local-ai', [], ['local-ai', 'start'])
        self.assert_route('install-app-mods', [], ['app-mods', 'install', '--skip-unavailable'])
        result = self.run_command('shell-ipc')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('IPC method required', result.stderr)

    def test_workflows_do_not_require_a_built_backend(self):
        self.backend.unlink()
        for name in WORKFLOWS:
            result = self.run_command(name, PAYLOAD)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), [name, *PAYLOAD])
        result = self.run_command('volume', ['up'])
        self.assertEqual(result.returncode, 1)
        self.assertIn('not built', result.stderr)

    def test_installed_link_chains_and_path_invocation_find_the_owner(self):
        bin_directory = self.root / 'home/.local/bin'
        bin_directory.mkdir(parents=True)
        shortcut = bin_directory / 'df-volume'
        shortcut.symlink_to('../../../scripts/volume')
        environment = dict(os.environ, PATH=str(bin_directory) + os.pathsep + os.environ['PATH'])
        result = subprocess.run(['df-volume', *PAYLOAD], env=environment, cwd='/',
                                text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), ['--root', str(self.root), 'volume', *PAYLOAD])

    def test_native_exit_status_and_stderr_are_preserved(self):
        self.executable(self.backend, '#!/bin/sh\nprintf "fixture failure\\n" >&2\nexit 23\n')
        result = self.run_command('record', ['status'])
        self.assertEqual(result.returncode, 23)
        self.assertEqual(result.stderr, 'fixture failure\n')

    def test_development_help_and_combined_build_use_current_checkout(self):
        self.executable(self.scripts / 'dev', (ROOT / 'scripts/dev').read_text())
        for name in ['build-backend', 'build-nothing']:
            self.executable(self.scripts / name,
                            '#!' + sys.executable + '\nimport json, sys\nprint(json.dumps([sys.argv[0], *sys.argv[1:]]))\n')
        result = self.run_command('dev', ['--help'])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('scripts/dev build', result.stdout)
        result = self.run_command('dev', ['build', 'all', '--offline'], cwd='/')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(calls, [[str(self.scripts / name), '--offline']
                                 for name in ['build-backend', 'build-nothing']])


if __name__ == '__main__':
    unittest.main()
