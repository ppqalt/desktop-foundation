from contextlib import redirect_stdout
import io
from pathlib import Path
import runpy
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
MODULE = runpy.run_path(str(ROOT / 'scripts/doctor'))


class PortalDiagnosis(unittest.TestCase):
    def test_empty_instance_list_is_distinct_from_invalid_data(self):
        count = MODULE['shell_instance_count']
        self.assertEqual(count(''), 0)
        self.assertEqual(count('[]'), 0)
        self.assertEqual(count('No running instances for "/checkout with spaces/shell/shell.qml"\nUse --all to list all instances.\n'), 0)
        self.assertEqual(count('[{"pid":1}]'), 1)
        for reply in ['null', '{}', 'invalid']:
            with self.assertRaises(ValueError): count(reply)

    def test_command_failures_preserve_native_stderr(self):
        import subprocess
        with patch.dict(MODULE['run'].__globals__, {'subprocess': __import__('subprocess')}), patch('subprocess.run', return_value=subprocess.CompletedProcess([], 1, '', 'fixture failure')):
            self.assertEqual(MODULE['run']('fixture'), (1, 'fixture failure'))

    def test_unavailable_portal_actionable(self):
        def run(*args):
            if args[:3] == ('systemctl', '--user', 'is-active'):
                return (3, 'inactive') if args[-1] == 'xdg-desktop-portal.service' else (0, 'active')
            if args[:3] == ('hyprctl', '-j', 'devices'):
                return 0, '{"keyboards":[{"active_keymap":"Finnish"}]}'
            return 0, ''
        output = io.StringIO()
        with patch.dict(MODULE['main'].__globals__, {'run': run}), patch.dict('os.environ', {v: 'test' for v in ['WAYLAND_DISPLAY', 'HYPRLAND_INSTANCE_SIGNATURE', 'XDG_CURRENT_DESKTOP', 'XDG_RUNTIME_DIR']}, clear=True), redirect_stdout(output):
            self.assertTrue(MODULE['main']())
        self.assertIn('xdg-desktop-portal: inspect journalctl', output.getvalue())


    def test_missing_greetd_pam_is_actionable_without_traceback(self):
        output = io.StringIO()
        with patch.dict(MODULE['session_main'].__globals__, {'run': lambda *args: (0, '')}), patch.dict('os.environ', {'NIRI_SOCKET': 'fixture'}, clear=True), patch('pathlib.Path.read_text', side_effect=FileNotFoundError('fixture missing')), redirect_stdout(output):
            self.assertTrue(MODULE['session_main']())
        self.assertIn('greetd PAM configuration unreadable', output.getvalue())
