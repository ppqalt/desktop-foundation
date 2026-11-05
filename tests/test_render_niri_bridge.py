"""Legacy Python callers use the same native renderer and semantic palette."""
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
import render_niri


class NativeRendererBridge(unittest.TestCase):
    def test_palette_is_passed_to_native_renderer_without_lua(self):
        palette = json.loads((ROOT / 'theme/fallback/semantic.json').read_text())
        palette['windowActive'] = '#123456'
        with tempfile.TemporaryDirectory() as temporary:
            env = {'HOME': temporary, 'XDG_CONFIG_HOME': temporary + '/config',
                   'XDG_STATE_HOME': temporary + '/state', 'PATH': temporary}
            with patch.dict(os.environ, env, clear=True):
                output = render_niri.render(ROOT, 'default', palette)
                self.assertIn('active-color "#123456"', output)
                palette['windowActive'] = 'invalid'
                with self.assertRaises(subprocess.CalledProcessError):
                    render_niri.render(ROOT, 'default', palette)
