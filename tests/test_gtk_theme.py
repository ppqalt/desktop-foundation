import sys
from pathlib import Path
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import gtk_theme


class GtkAccent(unittest.TestCase):
    def test_wallpaper_accents_map_to_supported_native_choices(self):
        self.assertEqual(gtk_theme.accent({'accent': '#98ccf9'}), 'blue')
        self.assertEqual(gtk_theme.accent({'accent': '#ffb599'}), 'orange')
        self.assertEqual(gtk_theme.accent({'accent': '#888888'}), 'slate')

    def test_unsupported_setting_skips_without_write(self):
        with patch('theme_runtime.current', return_value=Path('/unused')), patch.object(gtk_theme.subprocess, 'check_output', side_effect=OSError('no schema')), patch('preferences.manage_setting') as manage:
            gtk_theme.refresh()
        manage.assert_not_called()
