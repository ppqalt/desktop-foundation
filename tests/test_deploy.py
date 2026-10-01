import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('deploy', Path(__file__).resolve().parents[1] / 'scripts/deploy.py')
deploy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(deploy)


class Recovery(unittest.TestCase):
    def setUp(self):
        commands = patch.object(deploy.subprocess, 'run')
        commands.start()
        self.addCleanup(commands.stop)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        deploy.STATE = self.base / 'state'
        deploy.STATE.mkdir()
        self.path = self.base / 'config'
        self.path.write_text('original')
        self.backup = deploy.STATE / 'backup-0'
        self.manifest = {'root': str(deploy.ROOT), 'entries': [{'path': str(self.path), 'source': '/example/managed', 'backup': str(self.backup), 'original': deploy.fingerprint(self.path)}]}

    def test_interrupted_before_backup(self):
        deploy.save(self.manifest)
        deploy.restore(self.manifest)
        self.assertEqual(self.path.read_text(), 'original')

    def test_interrupted_after_backup(self):
        deploy.save(self.manifest)
        self.path.rename(self.backup)
        deploy.restore(self.manifest)
        self.assertEqual(self.path.read_text(), 'original')

    def test_interrupted_after_link(self):
        self.path.rename(self.backup)
        self.path.symlink_to('/example/managed')
        deploy.restore(self.manifest)
        self.assertEqual(self.path.read_text(), 'original')

    def test_foreign_replacement_refused(self):
        self.path.rename(self.backup)
        self.path.write_text('foreign')
        with self.assertRaises(RuntimeError):
            deploy.restore(self.manifest)
        self.assertEqual(self.backup.read_text(), 'original')

    def test_interrupted_restore_already_returned_backup(self):
        self.path.rename(self.backup)
        self.backup.rename(self.path)
        deploy.restore(self.manifest)
        self.assertEqual(self.path.read_text(), 'original')
