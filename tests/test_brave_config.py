import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('brave_config', ROOT / 'browser/brave/config.py')
brave = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(brave)


class BrowserConfiguration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'Brave-Origin-Nightly'
        self.profile = self.root / 'Default'
        self.profile.mkdir(parents=True)
        self.config = self.base / 'config'
        self.config.mkdir()
        self.theme = self.base / 'tracked'
        self.theme.mkdir()
        self.allow = {'Preferences': {'browser.show_forward_button': 'bool', 'intl.accept_languages': 'languages'}, 'Local State': {}}
        brave.atomic(self.theme / 'allowlist.json', self.allow)
        brave.atomic(self.theme / 'preferences.json', {'Preferences': {'browser.show_forward_button': False, 'intl.accept_languages': 'en-US,en'}, 'Local State': {}})
        (self.theme / 'flags.conf').write_text('# flags\n')
        patcher = patch.object(brave, 'HERE', self.theme)
        patcher.start()
        self.addCleanup(patcher.stop)
        patcher = patch.dict(os.environ, {'XDG_STATE_HOME': str(self.base / 'state'), 'XDG_CONFIG_HOME': str(self.config)})
        patcher.start()
        self.addCleanup(patcher.stop)
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()
        self.addCleanup(self.output.__exit__, None, None, None)

    def test_seed_preserves_private_data_and_repeat_keeps_interactive_change(self):
        original = {'browser': {'show_forward_button': True}, 'private_runtime': {'credential': 'not-exportable'}}
        brave.atomic(self.profile / 'Preferences', original)
        with patch.object(brave, 'busy', return_value=False):
            self.assertTrue(brave.apply(self.root, self.profile))
            data = brave.read(self.profile / 'Preferences')
            self.assertEqual(data['private_runtime'], original['private_runtime'])
            data['browser']['show_forward_button'] = True
            brave.atomic(self.profile / 'Preferences', data)
            self.assertTrue(brave.apply(self.root, self.profile))
            self.assertTrue(brave.read(self.profile / 'Preferences')['browser']['show_forward_button'])
            data['intl']['accept_languages'] = 'fi'
            brave.atomic(self.profile / 'Preferences', data)
            with self.assertRaises(RuntimeError):
                brave.restore(self.root, self.profile)
            brave.apply(self.root, self.profile, refresh=True)
            brave.restore(self.root, self.profile)
        self.assertEqual(brave.read(self.profile / 'Preferences')['private_runtime'], original['private_runtime'])
        self.assertTrue(brave.read(self.profile / 'Preferences')['browser']['show_forward_button'])

    def test_install_preserves_existing_profile_and_seeds_fresh_once(self):
        original = {'browser': {'show_forward_button': True}, 'private': 'keep'}
        brave.atomic(self.profile / 'Preferences', original)
        with patch.object(brave, 'busy', return_value=False):
            self.assertTrue(brave.apply(self.root, self.profile, install=True))
            self.assertEqual(brave.read(self.profile / 'Preferences'), original)
            (self.profile / 'Preferences').unlink()
            self.assertTrue(brave.apply(self.root, self.profile, install=True))
            seeded = brave.read(self.profile / 'Preferences')
            seeded['browser']['show_forward_button'] = True
            brave.atomic(self.profile / 'Preferences', seeded)
            self.assertTrue(brave.apply(self.root, self.profile, install=True))
            self.assertTrue(brave.read(self.profile / 'Preferences')['browser']['show_forward_button'])

    def test_channel_copy_preserves_profile_and_refuses_newer_or_busy_sources(self):
        (self.root / 'Last Version').write_text('158.1.99.0')
        brave.atomic(self.profile / 'Preferences', {'private': 'fixture-data', 'browser': {'show_forward_button': True}})
        cache = self.profile / 'Cache'
        cache.mkdir()
        (cache / 'transient').write_text('not copied')
        target = self.base / 'Stable'
        with patch.object(brave, 'busy', return_value=False):
            result = brave.migrate(self.root, target, '1:1.99.57-1')
        self.assertEqual(result['status'], 'migrated')
        self.assertEqual(brave.read(target / 'Default/Preferences'), brave.read(self.profile / 'Preferences'))
        self.assertTrue((self.root / 'Last Version').exists())
        self.assertFalse((target / 'Default/Cache').exists())
        with patch.object(brave, 'busy', side_effect=AssertionError('Existing Stable untouched')):
            self.assertEqual(brave.migrate(self.root, target, '1.99.57')['status'], 'existing')
        newer_target = self.base / 'Too-new'
        (self.root / 'Last Version').write_text('158.1.102.6')
        with patch.object(brave, 'busy', return_value=False):
            self.assertEqual(brave.migrate(self.root, newer_target, '1.99.57')['status'], 'pending')
        self.assertFalse(newer_target.exists())
        with patch.object(brave, 'busy', return_value=True):
            self.assertEqual(brave.migrate(self.root, newer_target, '1.103.0')['status'], 'pending')
        self.assertFalse(newer_target.exists())

    def test_migration_never_publishes_a_copy_changed_during_capture(self):
        (self.root / 'Last Version').write_text('158.1.99.0')
        brave.atomic(self.profile / 'Preferences', {'private': 'fixture'})
        target = self.base / 'Changed'
        original = brave.shutil.copytree
        def changed(*args, **kwargs):
            result = original(*args, **kwargs)
            (self.profile / 'Preferences').write_text('{"changed": true}')
            return result
        with patch.object(brave, 'busy', return_value=False), patch.object(brave.shutil, 'copytree', side_effect=changed):
            self.assertEqual(brave.migrate(self.root, target, '1.99.57')['status'], 'pending')
        self.assertFalse(target.exists())
        self.assertFalse(list(self.base.glob('.df-origin-migrate-*')))

    def test_open_browser_writes_nothing(self):
        with patch.object(brave, 'busy', return_value=True):
            self.assertFalse(brave.apply(self.root, self.profile))
        self.assertFalse((self.base / 'state').exists())
        self.assertFalse((self.profile / 'Preferences').exists())

    def test_export_drops_nested_secrets_and_wrong_types(self):
        data = {'browser': {'show_forward_button': False, 'account': {'token': 'private'}},
                'intl': {'accept_languages': 'unsafe://token'}, 'cookies': {'value': 'private'}}
        brave.atomic(self.profile / 'Preferences', data)
        brave.export(self.root, self.profile)
        self.assertEqual(brave.read(self.theme / 'preferences.json'), {'Preferences': {'browser.show_forward_button': False}, 'Local State': {}})
        self.assertNotIn('private', (self.theme / 'preferences.json').read_text())

    def test_unknown_seed_rejected(self):
        brave.atomic(self.theme / 'preferences.json', {'Preferences': {'account.token': 'private'}})
        with self.assertRaises(RuntimeError):
            brave.load_seeds()

    def test_fresh_profile_and_flags_backup_restore(self):
        flags = self.config / 'brave-origin-nightly-flags.conf'
        flags.write_text('# original\n')
        with patch.object(brave, 'busy', return_value=False):
            brave.apply(self.root, self.profile)
            self.assertTrue(flags.is_symlink())
            self.assertFalse((self.root / 'Local State').exists())
            brave.restore(self.root, self.profile)
        self.assertEqual(flags.read_text(), '# original\n')
        self.assertFalse(flags.is_symlink())

    def test_invalid_profile_paths_rejected(self):
        with self.assertRaises(RuntimeError):
            brave.locate(str(self.root), '../other')
