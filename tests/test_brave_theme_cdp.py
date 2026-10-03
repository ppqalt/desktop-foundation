"""Exercise restricted native DevTools client without accessing a browser."""
from pathlib import Path
import shutil
import subprocess
import unittest

class BraveThemeCdp(unittest.TestCase):
    @unittest.skipUnless(shutil.which('node'), 'optional personal Node dependency unavailable')
    def test_restricted_lifecycle_client(self):
        root=Path(__file__).resolve().parents[1]
        result=subprocess.run(['node','--test',str(root/'tests/brave_theme_cdp.test.mjs')],capture_output=True,text=True)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)
