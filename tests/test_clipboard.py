from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('DF_NATIVE_TEST_BINARY', ROOT / 'native/foundation/target/debug/desktop-foundationctl'))


class History(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not BINARY.is_file():
            raise RuntimeError('Build native/foundation or run scripts/test before these tests')

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.state = Path(self.temporary.name) / 'clipboard'
        self.env = dict(os.environ, CLIPBOARD_STATE='data')

    def invoke(self, *args, payload=None, check=True):
        return subprocess.run([str(BINARY), 'clipboard', '--state', str(self.state), *args],
                              input=payload, capture_output=True, timeout=10, check=check, env=self.env)

    def store(self, kind, payload):
        self.invoke('store', kind, payload=payload)

    def entries(self):
        return json.loads((self.state / 'index.json').read_text())

    def test_exact_multiline_unicode_roundtrip_and_dedup(self):
        payload = 'ä ö å €\nline two\n'.encode()
        self.store('text', payload)
        self.store('text', payload)
        self.assertEqual(len(self.entries()), 1)
        with closing(sqlite3.connect(self.state / 'history.sqlite')) as db:
            self.assertEqual(db.execute('SELECT payload FROM clips').fetchone()[0], payload)
        self.assertEqual(self.state.stat().st_mode & 0o777, 0o700)
        self.assertEqual((self.state / 'index.json').stat().st_mode & 0o777, 0o600)

    def test_sensitive_and_invalid_data_are_skipped(self):
        self.env['CLIPBOARD_STATE'] = 'sensitive'
        self.store('text', b'password')
        self.env['CLIPBOARD_STATE'] = 'data'
        self.store('text', b'\xff')
        self.store('image', b'unsupported')
        self.assertFalse(self.state.exists())

    def test_image_preview_removed_with_history(self):
        self.store('image', b'\x89PNG\r\n\x1a\nfixture')
        image = next(self.state.glob('*.png'))
        self.assertTrue(image.exists())
        self.invoke('clear')
        self.assertFalse(image.exists())
        self.assertEqual(self.entries(), [])

    def test_bounded_history_keeps_newest(self):
        for item in range(105):
            self.store('text', str(item).encode())
        entries = self.entries()
        self.assertEqual(len(entries), 100)
        self.assertEqual([e['preview'] for e in entries[:3]], ['104', '103', '102'])
        self.assertEqual(entries[-1]['preview'], '5')

    def test_limit_rejects_large_clipboard(self):
        self.store('text', b'x' * (8 * 1024 * 1024 + 1))
        self.assertFalse(self.state.exists())

    def test_sqlite_error_does_not_replace_index(self):
        self.store('text', b'kept')
        before = (self.state / 'index.json').read_bytes()
        with closing(sqlite3.connect(self.state / 'history.sqlite')) as db:
            db.execute("CREATE TRIGGER fail_insert BEFORE INSERT ON clips BEGIN SELECT RAISE(ABORT, 'fixture failure'); END")
            db.commit()
        result = self.invoke('store', 'text', payload=b'blocked', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'history was not reset', result.stderr)
        self.assertEqual((self.state / 'index.json').read_bytes(), before)
        with closing(sqlite3.connect(self.state / 'history.sqlite')) as db:
            self.assertEqual(db.execute('SELECT count(*) FROM clips').fetchone()[0], 1)

    def test_previews_preserve_nul_unicode_and_character_boundary(self):
        for text in ['before\0after', 'a' + '🙂' * 3000, 'äöå€' * 1200]:
            self.store('text', text.encode())
            self.assertEqual(self.entries()[0]['preview'], text[:2048])
            self.assertEqual(self.entries()[0]['size'], len(text.encode()))

    def test_export_recreates_missing_image_without_changing_history(self):
        payload = b'\x89PNG\r\n\x1a\nfixture'
        self.store('image', payload)
        image = next(self.state.glob('*.png'))
        expected = self.entries()
        image.unlink()
        self.invoke('init')
        self.assertEqual(image.read_bytes(), payload)
        self.assertEqual(self.entries(), expected)

    def test_copy_preserves_index_and_original_payload(self):
        payload = 'ä\0🙂\nsynthetic selection'.encode()
        self.store('text', payload)
        identity = self.entries()[0]['id']
        index = self.state / 'index.json'
        before = (index.read_bytes(), index.stat().st_ino, index.stat().st_mtime_ns)
        fake = Path(self.temporary.name) / 'wl-copy'
        fake.write_text('#!/usr/bin/env python3\nimport json,os,pathlib,sys\np = pathlib.Path(os.environ["DF_COPY_TEST"])\np.write_bytes(sys.stdin.buffer.read())\np.with_suffix(".json").write_text(json.dumps(sys.argv[1:]))\n')
        fake.chmod(0o700)
        output = Path(self.temporary.name) / 'copied'
        self.env.update(PATH=str(fake.parent) + os.pathsep + self.env['PATH'], DF_COPY_TEST=str(output))
        self.invoke('copy', identity)
        self.assertEqual((index.read_bytes(), index.stat().st_ino, index.stat().st_mtime_ns), before)
        self.assertEqual(output.read_bytes(), payload)
        self.assertEqual(json.loads(output.with_suffix('.json').read_text()), ['--type', 'text/plain;charset=utf-8'])

    def test_invalid_id_and_corrupt_database_are_reported_without_reset(self):
        self.store('text', b'kept')
        index = (self.state / 'index.json').read_bytes()
        self.assertNotEqual(self.invoke('copy', '../escape', check=False).returncode, 0)
        (self.state / 'history.sqlite').write_bytes(b'not a database')
        result = self.invoke('init', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'history was not reset', result.stderr)
        self.assertEqual((self.state / 'index.json').read_bytes(), index)
