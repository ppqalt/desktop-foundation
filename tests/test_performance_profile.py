"""Isolated transaction tests: no writes to /etc, /boot or real systemd."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('performance_profile', Path(__file__).resolve().parents[1] / 'scripts/performance_profile.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class PerformanceProfileTests(unittest.TestCase):
    def fixture(self, root):
        self.patches = [patch.object(m, 'STATE', root/'state'), patch.object(m, 'WINE', root/'etc/binfmt.d/wine.conf'), patch.object(m, 'CONDITION', root/'etc/systemd/service.d/condition.conf'), patch.object(m, 'VENDOR', root/'vendor.conf'), patch.object(m, 'hardware', return_value={'root': 'fixture'}), patch.object(m, 'run', return_value=''), patch.object(m, 'safe_parent', side_effect=lambda p: p.parent.mkdir(parents=True, exist_ok=True))]
        for p in self.patches:
            p.start()
            self.addCleanup(p.stop)
        m.VENDOR.write_text(':DOSWin:M::MZ::/usr/bin/wine:\n')

    def test_apply_idempotence_and_restore_preserve_unrelated_rules(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            unrelated = root/'etc/binfmt.d/python.conf'
            unrelated.parent.mkdir(parents=True)
            unrelated.write_text('unrelated')
            m.transaction('apply')
            journal = (m.STATE/'journal.json').read_bytes()
            m.transaction('apply')
            self.assertEqual(journal, (m.STATE/'journal.json').read_bytes())
            self.assertEqual(m.WINE.readlink(), Path('/dev/null'))
            self.assertNotIn('grub', m.CONDITION.read_text().lower())
            self.assertNotIn('/proc/sys/fs/binfmt_misc', m.CONDITION.read_text())
            m.transaction('restore')
            self.assertFalse(m.WINE.is_symlink())
            self.assertFalse(m.CONDITION.exists())
            self.assertEqual(unrelated.read_text(), 'unrelated')

    def test_foreign_file_and_changed_vendor_stop_without_overwriting(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            m.WINE.parent.mkdir(parents=True)
            m.WINE.write_text('owner policy')
            with self.assertRaisesRegex(ValueError, 'Existing administrator'):
                m.transaction('apply')
            self.assertEqual(m.WINE.read_text(), 'owner policy')
            m.WINE.unlink()
            m.transaction('apply')
            m.CONDITION.write_text('owner modification')
            with self.assertRaisesRegex(ValueError, 'Externally modified'):
                m.transaction('restore')
            m.VENDOR.write_text('new registration')
            with self.assertRaisesRegex(ValueError, 'changed'):
                m.binfmt_changes()

    def test_interrupted_apply_can_be_restored_without_touching_live_handlers(self):
        with tempfile.TemporaryDirectory() as directory:
            self.fixture(Path(directory))
            with patch.object(m, 'run', side_effect=RuntimeError('verify failed')):
                with self.assertRaisesRegex(RuntimeError, 'verify failed'):
                    m.transaction('apply')
            m.transaction('restore')
            self.assertFalse(m.WINE.is_symlink())
            self.assertFalse(m.CONDITION.exists())

    def test_condition_keeps_other_formats_and_fails_open_on_query_errors(self):
        import subprocess
        text = m.CONDITION_TEXT.split("/bin/sh -c '", 1)[1].rsplit("'", 1)[0].replace('$$', '$').replace('%%', '%')
        for output, query_exit, expected in [('# heading\n\n', 0, 1), (':other:M::AB::/usr/bin/other:\n', 0, 0), ('', 1, 0)]:
            with tempfile.TemporaryDirectory() as directory:
                tool = Path(directory)/'binfmt'
                tool.write_text('#!/bin/sh\ncat <<\'DATA\'\n'+output+'\nDATA\nexit '+str(query_exit)+'\n')
                tool.chmod(0o700)
                command = text.replace(m.BINFMT, str(tool))
                self.assertEqual(subprocess.run(['/bin/sh', '-c', command], check=False).returncode, expected)


if __name__ == '__main__':
    unittest.main()
