"""Preflight separates optional runtime tools from deployment requirements."""
import contextlib
import io
from pathlib import Path
import runpy
import subprocess
import sys
import unittest
from unittest.mock import patch
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import render_niri

class UpdatePreflight(unittest.TestCase):
    def invoke(self, missing, update):
        output = io.StringIO()
        original_is_file = Path.is_file
        def exists(path):
            return path == ROOT / 'native/foundation/target/release/desktop-foundationctl' or original_is_file(path)
        with patch.object(Path, 'is_file', exists), patch('os.access', return_value=True), \
             patch.object(sys, 'argv', ['preflight', *(['--update'] if update else [])]), \
             patch('shutil.which', side_effect=lambda tool: None if tool in missing else '/fixture/' + tool), \
             patch.object(render_niri, 'render', return_value='// isolated configuration'), \
             patch('subprocess.run', return_value=subprocess.CompletedProcess([],0,stdout='',stderr='')) as run, \
             contextlib.redirect_stdout(output):
            runpy.run_path(str(ROOT / 'scripts/preflight.py'), run_name='__main__')
        return output.getvalue(), run.call_args_list

    def test_update_reports_missing_media_tool_without_package_install(self):
        message, calls = self.invoke({'playerctl'}, True)
        self.assertIn('playerctl', message); self.assertIn('PASS', message)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0].args[0][:2], ['niri', 'validate'])

    def test_installer_still_requires_all_runtime_tools(self):
        with self.assertRaisesRegex(SystemExit, 'Missing runtime tools: playerctl'):
            self.invoke({'playerctl'}, False)

    def test_update_stops_before_validation_when_renderer_dependency_missing(self):
        with self.assertRaisesRegex(SystemExit, 'Missing deployment tools: fish'):
            self.invoke({'fish'}, True)

if __name__ == '__main__': unittest.main()
