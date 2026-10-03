import contextlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('deploy', ROOT / 'scripts/deploy.py')
deploy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(deploy)


class DeploymentPrerequisites(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'checkout'
        self.root.mkdir()
        self.state = self.base / 'state'
        self.binaries = [
            self.root / 'native/foundation/target/release/desktop-foundationctl',
            self.root / 'native/nothing/target/release/foundation-nothing',
        ]
        paths = patch.multiple(deploy, ROOT=self.root, STATE=self.state,
                               CONFIG=self.base / 'config', DATA=self.base / 'data')
        paths.start()
        self.addCleanup(paths.stop)

    def fixture_binary(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('private executable fixture; never run\n')
        path.chmod(0o755)

    def assert_rejected_without_mutation(self, build):
        before = sorted(self.base.rglob('*'))
        with patch.object(sys, 'argv', ['deploy', 'install']), patch.object(deploy.subprocess, 'run') as run:
            with self.assertRaisesRegex(RuntimeError, build):
                deploy.main()
        run.assert_not_called()
        self.assertEqual(sorted(self.base.rglob('*')), before)
        self.assertFalse(self.state.exists())

    def test_missing_each_backend_stops_before_state_or_deployment_writes(self):
        for binary in self.binaries:
            self.fixture_binary(binary)
        for binary, build in zip(self.binaries, ['scripts/build-backend', 'scripts/build-nothing']):
            with self.subTest(binary=binary.name):
                binary.unlink()
                self.assert_rejected_without_mutation(build)
                self.fixture_binary(binary)

    def test_nonexecutable_files_and_directories_are_not_accepted_as_backends(self):
        for binary in self.binaries:
            self.fixture_binary(binary)
        for binary, build in zip(self.binaries, ['scripts/build-backend', 'scripts/build-nothing']):
            with self.subTest(binary=binary.name):
                binary.chmod(0o644)
                self.assert_rejected_without_mutation(build)
                binary.unlink()
                binary.mkdir()
                self.assert_rejected_without_mutation(build)
                binary.rmdir()
                self.fixture_binary(binary)

    def test_complete_build_prerequisites_are_read_only(self):
        for binary in self.binaries:
            self.fixture_binary(binary)
        before = sorted(self.base.rglob('*'))
        deploy.require_native_backends()
        self.assertEqual(sorted(self.base.rglob('*')), before)
        self.assertFalse(self.state.exists())

    def test_missing_backends_do_not_block_restore_or_dry_run(self):
        with patch.object(deploy, 'require_native_backends', side_effect=AssertionError('build guard must not run')), contextlib.redirect_stdout(io.StringIO()):
            with patch.object(sys, 'argv', ['deploy', 'install', '--dry-run']):
                deploy.main()
            self.assertFalse(self.state.exists())
            with patch.object(sys, 'argv', ['deploy', 'restore']), patch.object(deploy.subprocess, 'run') as run:
                deploy.main()
            run.assert_not_called()


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

    def test_pending_owned_link_migration_is_recoverable(self):
        self.path.rename(self.backup)
        self.path.symlink_to('/example/new-runtime')
        self.manifest['entries'][0]['pending_source']='/example/new-runtime'
        deploy.restore(self.manifest)
        self.assertEqual(self.path.read_text(),'original')

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
