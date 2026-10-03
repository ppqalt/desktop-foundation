import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import application_roles
spec = importlib.util.spec_from_file_location('preferences', Path(__file__).resolve().parents[1] / 'scripts/preferences.py')
prefs = importlib.util.module_from_spec(spec); spec.loader.exec_module(prefs)


class Preferences(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name); self.values = {'color-scheme': "'default'", 'gtk-theme': "'Adwaita'"}
        patches = [patch.object(prefs, 'STATE', self.root / 'state'),
                   patch.object(prefs, 'get', side_effect=lambda key: self.values[key[1]]),
                   patch.object(prefs, 'set_value', side_effect=lambda key, value: self.values.update({key[1]: value})),
                   patch.object(application_roles, 'restore')]
        for item in patches: item.start(); self.addCleanup(item.stop)

    def invoke(self, action):
        with patch('sys.argv', ['preferences', action, '--theme-only']): prefs.main()

    def test_roundtrip_dark_preferences(self):
        self.invoke('install'); self.assertEqual(self.values['color-scheme'], "'prefer-dark'")
        self.invoke('restore'); self.assertEqual(self.values['color-scheme'], "'default'")

    def test_managed_theme_upgrade_preserves_original(self):
        prefs.STATE.mkdir(); self.values['gtk-theme'] = "'Adwaita-dark'"
        (prefs.STATE / 'preferences.json').write_text(json.dumps([{'key': ['gsettings', 'gtk-theme'], 'original': "'Adwaita'", 'managed': "'Adwaita-dark'"}]))
        self.invoke('install'); self.assertEqual(self.values['gtk-theme'], "'adw-gtk3-dark'")
        self.invoke('restore'); self.assertEqual(self.values['gtk-theme'], "'Adwaita'")

    def test_foreign_preference_change_refused(self):
        self.invoke('install'); self.values['gtk-theme'] = "'UserTheme'"
        with self.assertRaises(RuntimeError): self.invoke('restore')
        self.assertEqual(self.values['gtk-theme'], "'UserTheme'")

    def test_theme_only_does_not_mutate_roles(self):
        with patch.object(application_roles, 'apply') as apply:
            self.invoke('install'); apply.assert_not_called()
