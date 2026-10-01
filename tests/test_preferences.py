import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('preferences', Path(__file__).resolve().parents[1] / 'scripts/preferences.py')
prefs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prefs)


class Preferences(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.config = self.root / 'config'
        self.config.mkdir()
        self.mimefile = self.config / 'mimeapps.list'
        self.values = {'color-scheme': "'default'", 'gtk-theme': "'Adwaita'"}
        self.addCleanup(patch.stopall)
        patch.object(prefs, 'STATE', self.root / 'state').start()
        patch.dict(os.environ, {'XDG_CONFIG_HOME': str(self.config), 'XDG_DATA_HOME': str(self.root / 'data')}).start()
        patch.object(prefs, 'get', side_effect=lambda key: self.values[key[1]] if key[0] == 'gsettings' else '').start()
        def apply(args, **kwargs):
            if args[0] == 'gsettings':
                self.values[args[3]] = args[4]
            else:
                previous = self.mimefile.read_text() if self.mimefile.exists() else ''
                self.mimefile.write_text(previous + args[3] + '=' + args[2] + '\n')
        patch.object(prefs.subprocess, 'run', side_effect=apply).start()

    def invoke(self, action):
        with patch('sys.argv', ['preferences', action]):
            prefs.main()

    def test_roundtrip_original_file_and_preferences(self):
        self.mimefile.write_text('original defaults\n')
        self.invoke('install')
        self.assertEqual(self.values['color-scheme'], "'prefer-dark'")
        self.invoke('restore')
        self.assertEqual(self.values['color-scheme'], "'default'")
        self.assertEqual(self.mimefile.read_text(), 'original defaults\n')

    def test_managed_theme_upgrade_preserves_original_for_rollback(self):
        prefs.STATE.mkdir()
        self.values['gtk-theme'] = "'Adwaita-dark'"
        (prefs.STATE / 'preferences.json').write_text(json.dumps([
            {'key': ['gsettings', 'gtk-theme'], 'original': "'Adwaita'", 'managed': "'Adwaita-dark'"}]))
        self.invoke('install')
        self.assertEqual(self.values['gtk-theme'], "'adw-gtk3-dark'")
        self.invoke('restore')
        self.assertEqual(self.values['gtk-theme'], "'Adwaita'")

    def test_absent_original_is_removed(self):
        self.invoke('install')
        self.assertTrue(self.mimefile.exists())
        self.invoke('restore')
        self.assertFalse(self.mimefile.exists())

    def test_foreign_change_refused_before_any_restore(self):
        self.invoke('install')
        self.mimefile.write_text('user changed defaults')
        with self.assertRaises(RuntimeError):
            self.invoke('restore')
        self.assertEqual(self.values['color-scheme'], "'prefer-dark'")
        self.assertEqual(self.mimefile.read_text(), 'user changed defaults')
