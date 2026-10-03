import colorsys
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('theme_pipeline',ROOT/'scripts/theme_pipeline.py')
theme = importlib.util.module_from_spec(spec)
spec.loader.exec_module(theme)


class ThemePipeline(unittest.TestCase):
    def test_graphite_and_contrast_across_chromatic_sources(self):
        palettes = []
        for primary in ('#98ccf9','#ffb599','#fbb5ed','#ffb4ab','#b4d098'):
            p = theme.derive({'primary':primary,'secondary':'#cdc2d0'},'test','hash')
            palettes.append(p)
            for role in ('background','elevated','selected','iconTile'):
                _,l,s = colorsys.rgb_to_hls(*theme.rgb(p[role]))
                self.assertLess(s,.26)
                self.assertLess(l,.37)
            for role in ('background','elevated','selected'):
                self.assertGreaterEqual(theme.contrast(p['foreground'],p[role]),7)
                self.assertGreaterEqual(theme.contrast(p['muted'],p[role]),3)
                self.assertGreaterEqual(theme.contrast(p['accent'],p[role]),4.5)
            self.assertEqual(p['scrim'],'#50080b10')
        self.assertNotEqual(palettes[0]['accent'],palettes[1]['accent'])

    def test_missing_generator_leaves_outputs_untouched(self):
        before=(ROOT/'theme/generated.json').read_bytes()
        with patch.object(theme.shutil,'which',return_value=None):
            with self.assertRaises(RuntimeError): theme.generate(Path('/missing'))
        self.assertEqual(before,(ROOT/'theme/generated.json').read_bytes())

    def test_generator_failure_leaves_theme_untouched(self):
        import subprocess
        before=(ROOT/'theme/generated.json').read_bytes()
        with tempfile.TemporaryDirectory() as d:
            image=Path(d)/'test.png';image.write_bytes(b'unique-invalid-image')
            with patch.dict(theme.os.environ,{'XDG_CACHE_HOME':d}), patch.object(theme.shutil,'which',return_value='/usr/bin/matugen'), patch.object(theme.subprocess,'run',side_effect=subprocess.CalledProcessError(1,'matugen')):
                with self.assertRaises(subprocess.CalledProcessError):theme.generate(image)
        self.assertEqual(before,(ROOT/'theme/generated.json').read_bytes())

    def test_cached_hash_does_not_invoke_matugen(self):
        import hashlib
        with tempfile.TemporaryDirectory() as d:
            image=Path(d)/'wall.png';image.write_bytes(b'cached-image')
            digest=hashlib.sha256(image.read_bytes()).hexdigest()
            p=theme.derive({'primary':'#98ccf9','secondary':'#bdc7d5'},'test',digest)
            cached=Path(d)/'desktop-foundation/themes'/f'{digest}-{theme.VERSION}.json'
            cached.parent.mkdir(parents=True);cached.write_text(json.dumps(p))
            with patch.dict(theme.os.environ,{'XDG_CACHE_HOME':d}), patch.object(theme.shutil,'which',return_value='/usr/bin/matugen'), patch.object(theme.subprocess,'run') as run:
                palette,hit=theme.generate(image)
            self.assertTrue(hit)
            self.assertEqual(palette['wallpaperHash'],digest)
            run.assert_not_called()

    def test_invalid_material_rejected_before_writes(self):
        with self.assertRaises(ValueError):
            theme.derive({'primary':'not a color','secondary':'#ffffff'},'test','hash')

    def test_atomic_replacement_failure_keeps_original(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'palette.json';p.write_text('original')
            with patch.object(theme.os,'replace',side_effect=OSError('test')):
                with self.assertRaises(OSError):theme.atomic(p,'changed')
            self.assertEqual(p.read_text(),'original')
            self.assertEqual(list(Path(d).iterdir()),[p])

    def test_fallback_retains_original_palette(self):
        p=theme.read(ROOT/'theme/fallback/semantic.json')
        self.assertEqual(p['background'],'#171b22')
        self.assertEqual(p['accent'],'#b8ceee')
        self.assertEqual(p['windowActive'],'#485669')
