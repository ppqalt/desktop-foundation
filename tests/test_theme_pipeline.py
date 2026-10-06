import base64
import colorsys
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('theme_pipeline', ROOT/'scripts/theme_pipeline.py')
theme = importlib.util.module_from_spec(spec)
spec.loader.exec_module(theme)


def rgb(color):
    return tuple(int(color[i:i+2], 16)/255 for i in (1, 3, 5))


def contrast(first, second):
    def luminance(color):
        linear = [v/12.92 if v <= .04045 else ((v+.055)/1.055)**2.4 for v in rgb(color)]
        return sum(v*w for v, w in zip(linear, (.2126, .7152, .0722)))
    low, high = sorted((luminance(first), luminance(second)))
    return (high+.05)/(low+.05)


class ThemePipeline(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.bin = self.base/'bin'
        self.bin.mkdir()
        # scripts/foundation needs bash; no real Matugen or other desktop command
        # is available through this private PATH.
        (self.bin/'bash').symlink_to('/bin/bash')
        self.calls = self.base/'matugen-calls.jsonl'
        self.cache = self.base/'cache'
        environment = patch.dict(os.environ, {
            'HOME': str(self.base/'home'),
            'XDG_CACHE_HOME': str(self.cache),
            'XDG_STATE_HOME': str(self.base/'state'),
            'XDG_CONFIG_HOME': str(self.base/'config'),
            'XDG_DATA_HOME': str(self.base/'data'),
            'PATH': str(self.bin),
            'DF_TEST_MATUGEN_CALLS': str(self.calls),
            'DF_TEST_MATUGEN_RESULT': 'ok',
            'DF_TEST_MATUGEN_PRIMARY': '#98ccf9',
        })
        environment.start()
        self.addCleanup(environment.stop)
        self.image = self.base/'wallpaper.png'
        self.image.write_bytes(base64.b64decode(
            'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a1ioAAAAASUVORK5CYII='))

    def fake_matugen(self):
        executable = self.bin/'matugen'
        executable.write_text('#!' + sys.executable + '\n' + '''import json, os, pathlib, sys
with pathlib.Path(os.environ['DF_TEST_MATUGEN_CALLS']).open('a') as stream:
    stream.write(json.dumps(sys.argv[1:]) + '\\n')
mode = os.environ['DF_TEST_MATUGEN_RESULT']
if mode == 'fail':
    print('fixture Matugen failed', file=sys.stderr)
    raise SystemExit(9)
if mode == 'invalid-json':
    print('invalid JSON from fixture')
else:
    print(json.dumps({'colors': {
        'primary': {'dark': {'color': os.environ['DF_TEST_MATUGEN_PRIMARY']}},
        'secondary': {'dark': {'color': '#bdc7d5'}}}}))
''')
        executable.chmod(0o755)

    def call_count(self):
        return len(self.calls.read_text().splitlines()) if self.calls.exists() else 0

    def assert_graphite(self, palette):
        # Test-side calculations are independent of the Rust implementation.
        for role in ('background', 'elevated', 'selected', 'iconTile'):
            _, lightness, saturation = colorsys.rgb_to_hls(*rgb(palette[role]))
            self.assertLess(saturation, .26)
            self.assertLess(lightness, .37)
        for role in ('background', 'elevated', 'selected'):
            self.assertGreaterEqual(contrast(palette['foreground'], palette[role]), 7)
            self.assertGreaterEqual(contrast(palette['muted'], palette[role]), 3)
            self.assertGreaterEqual(contrast(palette['accent'], palette[role]), 4.5)
            self.assertGreaterEqual(contrast(palette['accentStrong'], palette[role]), 4.5)
        self.assertEqual(palette['scrim'], '#50080b10')

    def test_graphite_and_contrast_across_chromatic_sources(self):
        palettes = []
        for primary in ('#98ccf9', '#ffb599', '#fbb5ed', '#ffb4ab', '#b4d098'):
            palette = theme.derive({'primary': primary, 'secondary': '#cdc2d0'}, 'test', 'hash')
            palettes.append(palette)
            self.assert_graphite(palette)
            self.assertEqual(palette['source'], 'test')
            self.assertEqual(palette['wallpaperHash'], 'hash')
        self.assertNotEqual(palettes[0]['accent'], palettes[1]['accent'])

    def test_missing_generator_leaves_outputs_untouched(self):
        before = (ROOT/'theme/generated.json').read_bytes()
        with self.assertRaisesRegex(RuntimeError, 'matugen'):
            theme.generate(self.image)
        self.assertEqual(before, (ROOT/'theme/generated.json').read_bytes())
        self.assertFalse(list(self.cache.rglob('*.json')))

    def test_generator_failure_leaves_previous_cache_and_theme_untouched(self):
        self.fake_matugen()
        theme.generate(self.image)
        cached = next((self.cache/'desktop-foundation/themes').glob('*.json'))
        before_cache = cached.read_bytes()
        before_theme = (ROOT/'theme/generated.json').read_bytes()
        self.image.write_bytes(self.image.read_bytes() + b'changed')
        for failure in ('fail', 'invalid-json'):
            with self.subTest(failure=failure), patch.dict(os.environ, {'DF_TEST_MATUGEN_RESULT': failure}):
                with self.assertRaises(RuntimeError):
                    theme.generate(self.image)
            self.assertEqual(before_cache, cached.read_bytes())
            self.assertEqual(before_theme, (ROOT/'theme/generated.json').read_bytes())
            self.assertEqual(list(cached.parent.glob('*.json')), [cached])

    def test_cached_hash_does_not_invoke_matugen_and_updates_source(self):
        self.fake_matugen()
        first, hit = theme.generate(self.image)
        self.assertFalse(hit)
        self.assertEqual(self.call_count(), 1)
        other = self.base/'same wallpaper.png'
        other.write_bytes(self.image.read_bytes())
        with patch.dict(os.environ, {'DF_TEST_MATUGEN_RESULT': 'fail'}):
            palette, hit = theme.generate(other)
        self.assertTrue(hit)
        self.assertEqual(self.call_count(), 1)
        self.assertEqual(palette['wallpaperHash'], hashlib.sha256(self.image.read_bytes()).hexdigest())
        self.assertEqual(palette['source'], str(other.resolve()))
        self.assertEqual(palette['accent'], first['accent'])

    def test_blue_and_orange_generate_through_native_bridge(self):
        self.fake_matugen()
        before = (ROOT/'theme/generated.json').read_bytes()
        blue, _ = theme.generate(self.image)
        home = self.base/'home'
        home.mkdir()
        orange_image = home/'orange.png'
        orange_image.write_bytes(self.image.read_bytes() + b'orange fixture')
        with patch.dict(os.environ, {'DF_TEST_MATUGEN_PRIMARY': '#ffb599'}):
            orange, cached = theme.generate(Path('~/orange.png'))
        self.assertFalse(cached)
        self.assertEqual(orange['source'], str(orange_image.resolve()))
        self.assertEqual(self.call_count(), 2)
        self.assert_graphite(blue)
        self.assert_graphite(orange)
        self.assertEqual(blue['accent'], '#98ccf9')
        self.assertEqual(orange['accent'], '#ffb599')
        self.assertNotEqual(blue['background'], orange['background'])
        self.assertEqual(before, (ROOT/'theme/generated.json').read_bytes())
        self.assertFalse((self.base/'state').exists())

    def test_invalid_material_rejected_before_writes(self):
        with self.assertRaises(RuntimeError):
            theme.derive({'primary': 'not a color', 'secondary': '#ffffff'}, 'test', 'hash')
        self.assertFalse(self.cache.exists())

    def test_atomic_replacement_failure_keeps_original(self):
        path = self.base/'palette.json'
        path.write_text('original')
        with patch.object(theme.os, 'replace', side_effect=OSError('test')):
            with self.assertRaises(OSError):
                theme.atomic(path, 'changed')
        self.assertEqual(path.read_text(), 'original')
        self.assertEqual(list(self.base.glob('.*palette.json*')), [])

    def test_fallback_retains_original_palette(self):
        palette = theme.read(ROOT/'theme/fallback/semantic.json')
        self.assertEqual(palette['background'], '#171b22')
        self.assertEqual(palette['accent'], '#b8ceee')
        self.assertEqual(palette['windowActive'], '#485669')
