from contextlib import closing
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('clipboard', Path(__file__).resolve().parents[1] / 'scripts/clipboard.py')
clipboard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(clipboard)


class History(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        clipboard.STATE = Path(self.temporary.name) / 'clipboard'

    def entries(self):
        return json.loads((clipboard.STATE / 'index.json').read_text())

    def test_exact_multiline_unicode_roundtrip_and_dedup(self):
        payload = 'ä ö å €\nline two\n'.encode()
        clipboard.store('text', payload)
        clipboard.store('text', payload)
        self.assertEqual(len(self.entries()), 1)
        with closing(sqlite3.connect(clipboard.STATE / 'history.sqlite')) as db:
            self.assertEqual(db.execute('SELECT payload FROM clips').fetchone()[0], payload)
        self.assertEqual(clipboard.STATE.stat().st_mode & 0o777, 0o700)
        self.assertEqual((clipboard.STATE / 'index.json').stat().st_mode & 0o777, 0o600)

    def test_sensitive_and_invalid_data_are_skipped(self):
        with patch.dict(os.environ, {'CLIPBOARD_STATE': 'sensitive'}):
            clipboard.store('text', b'password')
        clipboard.store('text', b'\xff')
        clipboard.store('image', b'unsupported')
        self.assertFalse(clipboard.STATE.exists())

    def test_image_preview_removed_with_history(self):
        clipboard.store('image', b'\x89PNG\r\n\x1a\nfixture')
        image = next(clipboard.STATE.glob('*.png'))
        self.assertTrue(image.exists())
        with clipboard.database() as db:
            db.execute('DELETE FROM clips')
        self.assertFalse(image.exists())
        self.assertEqual(self.entries(), [])

    def test_bounded_history_keeps_newest(self):
        with patch.object(clipboard, 'MAX_ITEMS', 3):
            for item in range(5):
                clipboard.store('text', str(item).encode())
        self.assertEqual([e['preview'] for e in self.entries()], ['4', '3', '2'])

    def test_limit_rejects_large_clipboard(self):
        with patch.object(clipboard, 'MAX_BYTES', 4):
            clipboard.store('text', b'too large')
        self.assertFalse(clipboard.STATE.exists())

    def test_sqlite_error_does_not_replace_index(self):
        clipboard.store('text', b'kept')
        before = (clipboard.STATE / 'index.json').read_bytes()
        with self.assertRaises(RuntimeError):
            with clipboard.database() as db:
                db.execute('DELETE FROM clips')
                raise RuntimeError('interrupted transaction')
        self.assertEqual((clipboard.STATE / 'index.json').read_bytes(), before)
        with closing(sqlite3.connect(clipboard.STATE / 'history.sqlite')) as db:
            self.assertEqual(db.execute('SELECT count(*) FROM clips').fetchone()[0], 1)

    def test_previews_preserve_nul_unicode_and_character_boundary(self):
        for text in ['before\0after', 'a' + '🙂' * 3000, 'äöå€' * 1200]:
            clipboard.store('text', text.encode())
            self.assertEqual(self.entries()[0]['preview'], text[:2048])
            self.assertEqual(self.entries()[0]['size'], len(text.encode()))

    def test_export_recreates_missing_image_without_changing_history(self):
        payload = b'\x89PNG\r\n\x1a\nfixture'
        clipboard.store('image', payload)
        image = next(clipboard.STATE.glob('*.png'))
        expected = self.entries()
        image.unlink()
        with clipboard.database():
            pass
        self.assertEqual(image.read_bytes(), payload)
        self.assertEqual(self.entries(), expected)

    def test_copy_preserves_index_and_original_payload(self):
        payload = 'ä\0🙂\nsynthetic selection'.encode()
        clipboard.store('text', payload)
        identity = self.entries()[0]['id']
        index = clipboard.STATE / 'index.json'
        before = (index.read_bytes(), index.stat().st_ino, index.stat().st_mtime_ns)
        with patch('sys.argv', ['clipboard.py', '--state', str(clipboard.STATE), 'copy', identity]), patch.object(clipboard.subprocess, 'run') as copy:
            clipboard.main()
        self.assertEqual((index.read_bytes(), index.stat().st_ino, index.stat().st_mtime_ns), before)
        copy.assert_called_once_with(['wl-copy', '--type', 'text/plain;charset=utf-8'], input=payload, check=True, timeout=5)
