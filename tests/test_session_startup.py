import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import session_units
import startup_report


class SessionStartup(unittest.TestCase):
    def test_units_idempotent_and_shell_independent(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            paths = session_units.targets(ROOT, base / 'config', base / 'state', base / 'data')
            generated = base / 'state/session-units'
            times = {p: p.stat().st_mtime_ns for p in generated.iterdir()}
            self.assertEqual(paths, session_units.targets(ROOT, base / 'config', base / 'state', base / 'data'))
            self.assertEqual(times, {p: p.stat().st_mtime_ns for p in generated.iterdir()})
            shell = (generated / 'desktop-foundation-shell.service').read_text()
            self.assertNotIn('After=', shell)
            self.assertNotIn('Requires=', shell)
            watchers = (generated / 'desktop-foundation-clipboard@.service').read_text()
            self.assertIn('Requires=desktop-foundation-clipboard-init.service', watchers)
            self.assertIn('After=desktop-foundation-clipboard-init.service', watchers)
            target = (generated / 'desktop-foundation-session.target').read_text()
            self.assertNotIn('After=', target)
            self.assertIn('Wants=desktop-foundation-shell.service', target)
            aliases = dict(paths)
            self.assertEqual(aliases[base / 'config/systemd/user/mako.service'],
                             base / 'config/systemd/user/desktop-foundation-notifications.service')

    def test_changed_definitions_are_replaced(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            session_units.targets(ROOT, base / 'config', base / 'state', base / 'data')
            file = base / 'state/session-units/desktop-foundation-shell.service'
            file.write_text('obsolete')
            session_units.targets(ROOT, base / 'config', base / 'state', base / 'data')
            self.assertIn('ExecStart=', file.read_text())
            self.assertNotIn('obsolete', file.read_text())

    def test_hyprland_still_gets_shared_units(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            paths = session_units.targets(ROOT, base / 'config', base / 'state', base / 'data', niri=False)
            names = {p.name for p, _ in paths}
            self.assertIn('desktop-foundation-shell.service', names)
            self.assertNotIn('desktop-foundation-wallpaper.service', names)
            self.assertNotIn('mako.service', names)

    def test_login_path_has_no_generation_or_reload(self):
        for name in ['session-start', 'shell-start', 'clipboard-start']:
            text = (ROOT / 'scripts' / name).read_text()
            self.assertNotIn('daemon-reload', text)
            self.assertNotIn('printf', text)
        self.assertNotIn('write_text', (ROOT / 'scripts/niri_notifications.py').read_text())
        self.assertNotIn('scripts/dark-mode', (ROOT / 'scripts/session-start').read_text())

    def test_journal_binary_messages_and_missing_metrics(self):
        message = '\x1b[32mConfiguration Loaded\x1b[0m'
        self.assertEqual(startup_report.decode_message(list(message.encode())), 'Configuration Loaded')
        self.assertIsNone(startup_report.delta({}, 'launch', 'frame'))
        self.assertEqual(startup_report.delta({'a': 1, 'b': 1.125}, 'a', 'b'), 125)

    def test_tray_override_keeps_other_desktops(self):
        text = (ROOT / 'session/autostart/arch-update-tray.desktop').read_text()
        self.assertIn('NotShowIn=niri;', text)
        self.assertNotIn('Hidden=true', text)
        self.assertIn('arch-update --tray', text)

    def test_early_shortcut_waits_and_toggles_only_once(self):
        import os
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            fake = base / 'quickshell'
            fake.write_text('''#!/usr/bin/env python3
import os, pathlib, sys
p = pathlib.Path(os.environ['DF_TEST_LOG'])
lines = p.read_text().splitlines() if p.exists() else []
method = sys.argv[-1]
p.write_text('\\n'.join(lines + [method]) + '\\n')
if method == 'status' and lines.count('status') < 2:
    sys.exit(1)
''')
            fake.chmod(0o755)
            log = base / 'calls'
            result = subprocess.run([str(ROOT / 'scripts/shell-ipc'), 'toggleLauncher'],
                                    env={**os.environ, 'PATH': str(base) + ':' + os.environ['PATH'], 'DF_TEST_LOG': str(log)},
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(log.read_text().splitlines(), ['status', 'status', 'status', 'toggleLauncher'])
