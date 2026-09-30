from pathlib import Path
import shutil
import subprocess
import unittest


class Search(unittest.TestCase):
    @unittest.skipUnless(shutil.which('node'), 'Node required for JS search verification')
    def test_ranking_and_filtering(self):
        subprocess.run(['node', str(Path(__file__).with_name('search.mjs'))], check=True)
