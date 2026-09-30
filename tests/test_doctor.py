from contextlib import redirect_stdout
import io
from pathlib import Path
import runpy
import unittest
from unittest.mock import patch

MODULE = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'scripts/doctor'))


class PortalDiagnosis(unittest.TestCase):
    def test_unavailable_portal_actionable(self):
        def run(*args):
            if args[:3] == ('systemctl', '--user', 'is-active'):
                return (3, 'inactive') if args[-1] == 'xdg-desktop-portal.service' else (0, 'active')
            if args[:3] == ('hyprctl', '-j', 'devices'):
                return 0, '{"keyboards":[{"active_keymap":"Finnish"}]}'
            return 0, ''
        output = io.StringIO()
        with patch.dict(MODULE['main'].__globals__, {'run': run}), patch.dict('os.environ', {v: 'test' for v in ['WAYLAND_DISPLAY', 'HYPRLAND_INSTANCE_SIGNATURE', 'XDG_CURRENT_DESKTOP', 'XDG_RUNTIME_DIR']}), redirect_stdout(output):
            self.assertTrue(MODULE['main']())
        self.assertIn('xdg-desktop-portal: inspect journalctl', output.getvalue())
