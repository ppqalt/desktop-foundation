"""Native preference ownership with private XDG state and inert native tools."""
import fcntl
import json
import os
from pathlib import Path
import subprocess
import unittest

from preference_fixture import BINARY, ROOT, PreferenceFixture


class NativePreferences(unittest.TestCase):
    def setUp(self):
        self.fixture = PreferenceFixture(self)

    def test_old_journal_format_repeated_update_and_restore(self):
        f = self.fixture; f.state.mkdir(parents=True)
        f.values(**{'accent-color': "'orange'"})
        f.journal.write_text(json.dumps([{'key': ['gsettings', 'accent-color'], 'original': "'blue'", 'managed': "'orange'", 'note': 'keep unknown fields'}]))
        f.invoke('set', 'gsettings', 'accent-color', "'purple'")
        saved = f.saved()[0]
        self.assertEqual(saved['original'], "'blue'")
        self.assertEqual(saved['note'], 'keep unknown fields')
        writes = len([call for call in f.calls() if call[1] == 'set'])
        result = f.invoke('set', 'gsettings', 'accent-color', "'purple'")
        self.assertFalse(json.loads(result.stdout)['changed'])
        self.assertEqual(len([call for call in f.calls() if call[1] == 'set']), writes)
        self.assertEqual(f.journal.stat().st_mode & 0o777, 0o600)
        f.invoke('restore'); self.assertEqual(f.values()['accent-color'], "'blue'")
        self.assertFalse(f.journal.exists())

    def test_external_change_refused_without_journal_or_native_write(self):
        f = self.fixture; f.install(); before = f.journal.read_bytes()
        f.values(**{'gtk-theme': "'UserTheme'"})
        for arguments in [('set', 'gsettings', 'gtk-theme', "'Other'"), ('check',), ('restore',)]:
            result = f.invoke(*arguments, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Preference changed outside installation', result.stderr)
            self.assertEqual(f.journal.read_bytes(), before)
            self.assertEqual(f.values()['gtk-theme'], "'UserTheme'")

    def test_interrupted_setting_resumes_without_losing_original(self):
        f = self.fixture; f.env['PREF_FAIL_KEY'] = 'accent-color'
        result = f.invoke('set', 'gsettings', 'accent-color', "'orange'", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(f.saved()[0]['pending'], "'orange'")
        self.assertEqual(f.saved()[0]['original'], "'blue'")
        del f.env['PREF_FAIL_KEY']
        f.invoke('set', 'gsettings', 'accent-color', "'orange'")
        self.assertNotIn('pending', f.saved()[0])
        f.invoke('restore'); self.assertEqual(f.values()['accent-color'], "'blue'")

    def test_native_write_before_failed_publication_remains_resumable(self):
        f = self.fixture; f.env['PREF_BREAK_PUBLICATION'] = 'accent-color'
        self.assertNotEqual(f.invoke('set', 'gsettings', 'accent-color', "'orange'", check=False).returncode, 0)
        self.assertEqual(f.values()['accent-color'], "'orange'")
        f.journal.rmdir(); f.journal.with_suffix('.saved').rename(f.journal)
        del f.env['PREF_BREAK_PUBLICATION']
        f.invoke('set', 'gsettings', 'accent-color', "'orange'")
        self.assertEqual(f.saved()[0]['original'], "'blue'")
        f.invoke('restore'); self.assertEqual(f.values()['accent-color'], "'blue'")

    def test_restore_failure_retains_only_remaining_entries(self):
        f = self.fixture; f.install(); f.env['PREF_FAIL_KEY'] = 'gtk-theme'
        self.assertNotEqual(f.invoke('restore', check=False).returncode, 0)
        saved = f.saved()
        self.assertEqual(len(saved), 1)
        self.assertEqual(saved[0]['key'], ['gsettings', 'gtk-theme'])
        self.assertEqual(saved[0]['pending'], "'Adwaita'")
        del f.env['PREF_FAIL_KEY']; f.invoke('restore')
        self.assertEqual(f.values()['gtk-theme'], "'Adwaita'")

    def test_inherited_lock_does_not_release_bridge_lock(self):
        f = self.fixture; f.install()
        with (f.state / 'preferences.lock').open('a') as bridge:
            fcntl.flock(bridge, fcntl.LOCK_EX)
            self.assertEqual(json.loads(f.invoke('check', lock=bridge).stdout)['checked'], 2)
            with (f.state / 'preferences.lock').open('a') as contender:
                with self.assertRaises(BlockingIOError):
                    fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
            f.invoke('restore', lock=bridge)
        self.assertFalse(f.journal.exists())

    def test_unrelated_inherited_descriptor_is_refused(self):
        f = self.fixture; f.install(); before = f.journal.read_bytes()
        with f.values_file.open('a') as unrelated:
            result = f.invoke('restore', lock=unrelated, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('not the state lock', result.stderr)
        self.assertEqual(f.journal.read_bytes(), before)

    def test_legacy_mime_preference_roundtrip(self):
        f = self.fixture; f.values(**{'text/plain': 'previous.desktop'})
        f.invoke('set', 'xdg-mime', 'text/plain', 'new.desktop')
        self.assertEqual(f.values()['text/plain'], 'new.desktop')
        f.invoke('restore'); self.assertEqual(f.values()['text/plain'], 'previous.desktop')

    def test_journal_symlink_duplicate_keys_and_invalid_key_refused(self):
        f = self.fixture; f.install(); outside = f.base / 'outside.json'
        outside.write_bytes(f.journal.read_bytes()); f.journal.unlink(); f.journal.symlink_to(outside)
        before = outside.read_bytes()
        self.assertNotEqual(f.invoke('restore', check=False).returncode, 0)
        self.assertEqual(outside.read_bytes(), before)
        f.journal.unlink(); f.journal.write_text(json.dumps([json.loads(before)[0]] * 2))
        self.assertNotEqual(f.invoke('restore', check=False).returncode, 0)
        calls = len(f.calls())
        result = f.invoke('set', 'unknown', '--invalid', 'value', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(f.calls()), calls)

    def test_gtk_refresh_maps_applies_and_preserves_original_for_rollback(self):
        f = self.fixture; f.palette()
        result = json.loads(f.invoke('gtk-refresh').stdout)
        self.assertEqual(result, {'status': 'applied', 'accent': 'orange', 'changed': True})
        self.assertEqual(f.values()['accent-color'], "'orange'")
        f.palette('#98ccf9'); f.invoke('gtk-refresh')
        self.assertEqual(f.values()['accent-color'], "'blue'")
        self.assertEqual(f.saved()[0]['original'], "'blue'")
        f.invoke('restore'); self.assertEqual(f.values()['accent-color'], "'blue'")

    def test_absent_palette_missing_schema_and_unsupported_enum_skip_write(self):
        f = self.fixture
        self.assertEqual(json.loads(f.invoke('gtk-refresh').stdout)['status'], 'absent')
        self.assertEqual(f.calls(), [])
        f.palette(); f.env['PREF_RANGE_FAIL'] = '1'
        self.assertEqual(json.loads(f.invoke('gtk-refresh').stdout)['status'], 'unavailable')
        self.assertFalse(f.journal.exists())
        del f.env['PREF_RANGE_FAIL']; f.env['PREF_RANGE'] = "enum\n'blue'\n'fake-orange'"
        self.assertEqual(json.loads(f.invoke('gtk-refresh').stdout)['status'], 'unsupported')
        self.assertFalse(f.journal.exists())
        self.assertFalse(any(call[1] == 'set' for call in f.calls()))

    def test_gtk_external_override_is_preserved(self):
        f = self.fixture; f.palette(); f.invoke('gtk-refresh')
        f.values(**{'accent-color': "'purple'"}); before = f.journal.read_bytes()
        result = f.invoke('gtk-refresh', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(f.values()['accent-color'], "'purple'")
        self.assertEqual(f.journal.read_bytes(), before)

    def test_invalid_palette_fails_before_preference_write(self):
        f = self.fixture; f.palette('not-a-color')
        self.assertNotEqual(f.invoke('gtk-refresh', check=False).returncode, 0)
        self.assertFalse(f.journal.exists())
        self.assertFalse(any(call[1] == 'set' for call in f.calls()))

    def test_oversized_palette_is_rejected_before_parse_or_write(self):
        f = self.fixture; f.palette()
        (f.state / 'theme/current/semantic.json').write_text('{"accent":"#98ccf9", "extra":"' + 'x' * 65536 + '"}')
        result = f.invoke('gtk-refresh', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Palette JSON is too large', result.stderr)
        self.assertFalse(f.journal.exists())
        self.assertFalse(any(call[1] == 'set' for call in f.calls()))
