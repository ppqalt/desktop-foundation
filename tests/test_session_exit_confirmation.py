"""Never invokes Niri: verify the logout helper with an isolated socket and spy."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

class SessionExitConfirmation(unittest.TestCase):
    def test_confirmed_logout_does_not_request_a_second_compositor_prompt(self):
        with tempfile.TemporaryDirectory(prefix='foundation-logout-') as directory:
            base = Path(directory)
            executable = base / 'niri'
            executable.write_text('#!/usr/bin/env python3\nimport json,os,sys\nopen(os.environ["DF_LOGOUT_TEST"],"w").write(json.dumps(sys.argv[1:]))\n')
            executable.chmod(0o700)
            with socket.socket(socket.AF_UNIX) as peer:
                peer.bind(str(base / 'socket'))
                subprocess.run([str(ROOT / 'scripts/session-exit')], check=True,
                    env={**os.environ, 'NIRI_SOCKET': str(base / 'socket'),
                         'PATH': str(base) + os.pathsep + os.environ['PATH'],
                         'DF_LOGOUT_TEST': str(base / 'args')})
            self.assertEqual(json.loads((base / 'args').read_text()),
                             ['msg', 'action', 'quit', '--skip-confirmation'])
