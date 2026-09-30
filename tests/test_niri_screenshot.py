"""Cancellation/failure must not replace clipboard contents or create captures."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('niri_capture', Path(__file__).resolve().parents[1] / 'scripts/screenshot_backends/niri.py')
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)


class RegionSafety(unittest.TestCase):
    def test_cancel_preserves_clipboard_and_creates_no_capture(self):
        with tempfile.TemporaryDirectory() as runtime:
            with patch.dict(os.environ, {'XDG_RUNTIME_DIR': runtime}), patch.object(capture.subprocess, 'run') as run:
                run.return_value = subprocess.CompletedProcess(['slurp'], 1, b'', b'')
                self.assertEqual(capture.region(), 0)
                self.assertEqual(run.call_count, 1)
                self.assertEqual(run.call_args.kwargs['input'], b'')
                self.assertEqual([p.suffix for p in Path(runtime).iterdir()], ['.lock'])

    def test_capture_failure_does_not_publish_clipboard(self):
        with tempfile.TemporaryDirectory() as runtime:
            with patch.dict(os.environ, {'XDG_RUNTIME_DIR': runtime}), patch.object(capture.subprocess, 'run') as run:
                run.side_effect = [subprocess.CompletedProcess(['slurp'], 0, b'10,20 80x60\n', b''),
                                   subprocess.CalledProcessError(1, ['grim'])]
                with self.assertRaises(subprocess.CalledProcessError):
                    capture.region()
                self.assertFalse(any(call.args[0][0] == 'wl-copy' for call in run.call_args_list))
                self.assertEqual([p.suffix for p in Path(runtime).iterdir()], ['.lock'])
