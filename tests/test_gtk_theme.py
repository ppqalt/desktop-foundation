import colorsys
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import gtk_theme
import preferences
import theme_pipeline
from preference_fixture import BINARY, ROOT, PreferenceFixture


class GtkAccent(unittest.TestCase):
    def test_wallpaper_accents_match_existing_python_policy(self):
        fixture = PreferenceFixture(self)
        anchors = {'blue': '#3584e4', 'teal': '#2190a4', 'green': '#3a944a', 'yellow': '#c88800',
                   'orange': '#ed5b00', 'red': '#e62d42', 'pink': '#d56199', 'purple': '#9141ac'}
        def hls(color):
            return colorsys.rgb_to_hls(*(int(color[index:index+2], 16) / 255 for index in (1, 3, 5)))
        def expected(color):
            hue, _, saturation = hls(color)
            if saturation < .12: return 'slate'
            return min(anchors, key=lambda key: min(abs(hls(anchors[key])[0] - hue), 1 - abs(hls(anchors[key])[0] - hue)))
        samples = [*anchors.values(), '#98ccf9', '#ffb599', '#888888', '#000000', '#ffffff',
                   '#ff0001', '#ff0100', '#8f7171', '#907070']
        with patch.object(theme_pipeline, 'ROOT', fixture.checkout):
            for color in samples:
                with self.subTest(color=color):
                    self.assertEqual(gtk_theme.accent({'accent': color}), expected(color))
        self.assertEqual(fixture.calls(), [])

    def test_bridge_refresh_skips_unsupported_setting_without_write(self):
        fixture = PreferenceFixture(self); fixture.palette(); fixture.env['PREF_RANGE_FAIL'] = '1'
        with patch.object(preferences, 'ROOT', fixture.checkout), patch.object(preferences, 'STATE', fixture.state), patch.dict(preferences.os.environ, fixture.env, clear=True):
            gtk_theme.refresh()
        self.assertFalse(fixture.journal.exists())
        self.assertFalse(any(call[1] == 'set' for call in fixture.calls()))
