import importlib.machinery
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

path = Path(__file__).resolve().parents[1] / 'terminal/fastfetch/packages'
loader = importlib.machinery.SourceFileLoader('packages', str(path))
spec = importlib.util.spec_from_loader(loader.name, loader)
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)


class ForeignCache(unittest.TestCase):
    def test_reuse_and_local_or_repository_invalidation(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            (base / 'local').mkdir()
            (base / 'sync').mkdir()
            repo = base / 'sync/core.db'
            repo.write_text('initial')
            cache = base / 'cache/count.json'
            with patch.object(module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'foreign\n', '')) as query:
                self.assertEqual(module.foreign_count(base, ['core'], cache), 1)
                self.assertEqual(module.foreign_count(base, ['core'], cache), 1)
                self.assertEqual(query.call_count, 1)
                (base / 'local/new-package').mkdir()
                module.foreign_count(base, ['core'], cache)
                self.assertEqual(query.call_count, 2)
                repo.write_text('repository update')
                module.foreign_count(base, ['core'], cache)
                self.assertEqual(query.call_count, 3)
                module.foreign_count(base, ['core', 'extra'], cache)
                self.assertEqual(query.call_count, 4)

    def test_zero_transaction_and_error(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            (base / 'local').mkdir()
            cache = base / 'cache/count.json'
            with patch.object(module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1, '', '')) as query:
                self.assertEqual(module.foreign_count(base, [], cache), 0)
                self.assertEqual(module.foreign_count(base, [], cache), 0)
                self.assertEqual(query.call_count, 1)
                (base / 'db.lck').touch()
                module.foreign_count(base, [], cache)
                module.foreign_count(base, [], cache)
                self.assertEqual(query.call_count, 3)
            with patch.object(module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1, '', 'database error')):
                with self.assertRaises(RuntimeError):
                    module.foreign_count(base, [], cache)
