"""Public helper directories must not inherit the installer's private umask."""
import importlib.machinery
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
loader = importlib.machinery.SourceFileLoader('browser_setup_fixture',str(ROOT/'scripts/browser-policy-setup'))
spec = importlib.util.spec_from_loader(loader.name,loader)
setup = importlib.util.module_from_spec(spec)
loader.exec_module(setup)


class PublicDirectories(unittest.TestCase):
    def test_restrictive_umask_and_owned_directory_repair(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'public';journal={'created_directories':[]}
            previous=os.umask(0o077)
            try:
                with patch.object(setup,'parents'),patch.object(setup,'save') as save:
                    setup.ensure_dirs(journal,[path],'policy.json')
                    self.assertEqual(path.stat().st_mode&0o777,0o755)
                    self.assertIn(str(path),journal['created_directories'])
                    self.assertEqual(save.call_args.args[0],setup.STATE/'policy.json')
                    path.chmod(0o700)
                    setup.ensure_dirs(journal,[path],'policy.json')
                    self.assertEqual(path.stat().st_mode&0o777,0o755)
            finally:os.umask(previous)

    def test_existing_unowned_directory_permissions_are_preserved(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'existing';path.mkdir(mode=0o700)
            with patch.object(setup,'parents'),patch.object(setup,'save') as save:
                setup.ensure_dirs({'created_directories':[]},[path])
            self.assertEqual(path.stat().st_mode&0o777,0o700)
            save.assert_not_called()
