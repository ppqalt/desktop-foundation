import importlib.util
import json
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import application_roles
from preference_fixture import PreferenceFixture
spec = importlib.util.spec_from_file_location('preferences', Path(__file__).resolve().parents[1] / 'scripts/preferences.py')
prefs = importlib.util.module_from_spec(spec); spec.loader.exec_module(prefs)


class Preferences(unittest.TestCase):
    def setUp(self):
        self.fixture = PreferenceFixture(self)
        self.root = self.fixture.base
        patches = [patch.object(prefs, 'STATE', self.fixture.state),
                   patch.object(prefs, 'ROOT', self.fixture.checkout),
                   patch.dict(os.environ, self.fixture.env, clear=True),
                   patch.object(application_roles, 'restore')]
        for item in patches: item.start(); self.addCleanup(item.stop)

    def invoke(self, action):
        with patch('sys.argv', ['preferences', action, '--theme-only']): prefs.main()

    def test_roundtrip_dark_preferences(self):
        self.invoke('install'); self.assertEqual(self.fixture.values()['color-scheme'], "'prefer-dark'")
        self.invoke('restore'); self.assertEqual(self.fixture.values()['color-scheme'], "'default'")

    def test_managed_theme_upgrade_preserves_original(self):
        prefs.STATE.mkdir(parents=True); self.fixture.values(**{'gtk-theme': "'Adwaita-dark'"})
        self.fixture.journal.write_text(json.dumps([{'key': ['gsettings', 'gtk-theme'], 'original': "'Adwaita'", 'managed': "'Adwaita-dark'"}]))
        self.invoke('install'); self.assertEqual(self.fixture.values()['gtk-theme'], "'adw-gtk3-dark'")
        self.invoke('restore'); self.assertEqual(self.fixture.values()['gtk-theme'], "'Adwaita'")

    def test_foreign_preference_change_refused_before_role_restore(self):
        self.invoke('install'); self.fixture.values(**{'gtk-theme': "'UserTheme'"})
        with patch.object(application_roles, 'restore') as roles:
            with self.assertRaises(RuntimeError): self.invoke('restore')
            roles.assert_not_called()
        self.assertEqual(self.fixture.values()['gtk-theme'], "'UserTheme'")

    def test_theme_only_does_not_mutate_roles(self):
        with patch.object(application_roles, 'apply') as apply:
            self.invoke('install'); apply.assert_not_called()

    def test_interrupted_restore_retains_only_unfinished_settings(self):
        self.invoke('install')
        with patch.dict(os.environ, {'PREF_FAIL_KEY': 'gtk-theme'}):
            with self.assertRaises(RuntimeError): self.invoke('restore')
        remaining = self.fixture.saved()
        self.assertEqual([entry['key'][1] for entry in remaining], ['gtk-theme'])
        self.assertEqual(self.fixture.values()['color-scheme'], "'default'")
        self.invoke('restore')
        self.assertEqual(self.fixture.values()['gtk-theme'], "'Adwaita'")
        self.assertFalse(self.fixture.journal.exists())

    def test_crash_after_native_restore_before_journal_update_can_resume(self):
        self.invoke('install')
        with patch.dict(os.environ, {'PREF_BREAK_PUBLICATION': 'color-scheme'}):
            with self.assertRaises(RuntimeError): self.invoke('restore')
        self.assertEqual(self.fixture.values()['color-scheme'], "'default'")
        self.fixture.journal.rmdir()
        self.fixture.journal.with_suffix('.saved').rename(self.fixture.journal)
        self.invoke('restore')
        self.assertEqual(self.fixture.values()['gtk-theme'], "'Adwaita'")

    def test_bridge_keeps_preference_lock_during_role_restore(self):
        import fcntl
        self.invoke('install')
        def restore_roles():
            with (self.fixture.state / 'preferences.lock').open('a') as contender:
                with self.assertRaises(BlockingIOError):
                    fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
        with patch.object(application_roles, 'restore', side_effect=restore_roles):
            self.invoke('restore')
        self.assertFalse(self.fixture.journal.exists())

    def test_wrapper_timeout_keeps_pending_journal_and_stops_late_native_write(self):
        import fcntl
        import time
        native_run = prefs.subprocess.run
        def shorter_deadline(command, **options):
            options['timeout'] = .75
            return native_run(command, **options)
        with patch.dict(os.environ, {'PREF_STALL_KEY': 'accent-color'}), patch.object(prefs.subprocess, 'run', side_effect=shorter_deadline):
            with prefs.preference_lock() as bridge:
                with self.assertRaisesRegex(RuntimeError, 'timed out'):
                    prefs.native('set', 'gsettings', 'accent-color', "'orange'", lock=bridge)
                self.assertEqual(self.fixture.saved()[0]['pending'], "'orange'")
                with (self.fixture.state / 'preferences.lock').open('a') as contender:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
        with (self.fixture.state / 'preferences.lock').open('a') as contender:
            fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
        time.sleep(2.1)
        self.assertEqual(self.fixture.values()['accent-color'], "'blue'")
        prefs.manage_setting(['gsettings', 'accent-color'], "'orange'")
        self.assertEqual(self.fixture.saved()[0]['original'], "'blue'")

    def test_legacy_mime_restored_by_role_adapter_is_accepted(self):
        self.fixture.values(**{'text/plain': 'previous.desktop'})
        prefs.manage_setting(['xdg-mime', 'text/plain'], 'new.desktop')
        def restore_roles():
            self.assertEqual(self.fixture.saved()[0]['pending'], 'previous.desktop')
            self.fixture.values(**{'text/plain': 'previous.desktop'})
        with patch.object(application_roles, 'restore', side_effect=restore_roles):
            self.invoke('restore')
        self.assertFalse(self.fixture.journal.exists())
        self.assertEqual(self.fixture.values()['text/plain'], 'previous.desktop')

    def test_legacy_mime_prepare_does_not_accept_unrelated_value(self):
        self.fixture.values(**{'text/plain': 'previous.desktop'})
        prefs.manage_setting(['xdg-mime', 'text/plain'], 'new.desktop')
        with patch.object(application_roles, 'restore', side_effect=lambda: self.fixture.values(**{'text/plain': 'foreign.desktop'})):
            with self.assertRaises(RuntimeError): self.invoke('restore')
        self.assertEqual(self.fixture.values()['text/plain'], 'foreign.desktop')
        self.assertTrue(self.fixture.journal.exists())

    def test_interrupted_mime_apply_prepares_original_before_role_restore(self):
        self.fixture.state.mkdir(parents=True)
        self.fixture.journal.write_text(json.dumps([{'key': ['xdg-mime', 'text/plain'], 'original': 'previous.desktop', 'managed': 'previous.desktop', 'pending': 'new.desktop'}]))
        self.fixture.values(**{'text/plain': 'new.desktop'})
        with patch.object(application_roles, 'restore', side_effect=lambda: self.fixture.values(**{'text/plain': 'previous.desktop'})):
            self.invoke('restore')
        self.assertFalse(self.fixture.journal.exists())
