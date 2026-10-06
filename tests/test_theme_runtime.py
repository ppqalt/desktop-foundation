import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
import theme_runtime as runtime
import theme_pipeline as pipeline
from theme.adapters import brave,MODES
from PIL import Image


class RuntimeTheme(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.base=Path(self.temp.name)
        env=patch.dict(os.environ,{'XDG_STATE_HOME':str(self.base/'state'),'XDG_CACHE_HOME':str(self.base/'cache')})
        env.start();self.addCleanup(env.stop)
        self.image=self.base/'wall.png';Image.new('RGB',(80,50),'orange').save(self.image)
        self.palette=pipeline.derive({'primary':'#ffb599','secondary':'#dfbfaf'},'test','hash')
        # Keep orchestration spies local: palette derivation now calls the real Rust CLI.
        processes=patch.object(runtime,'subprocess',SimpleNamespace(**vars(runtime.subprocess)))
        processes.start();self.addCleanup(processes.stop)
        calls=patch.object(runtime.subprocess,'run');calls.start();self.addCleanup(calls.stop)
        niri=patch('render_niri.render',return_value='// validated separately by native checks\n');niri.start();self.addCleanup(niri.stop)

    def bundle(self):return runtime.stage(self.palette,self.image)

    def test_bundle_is_complete_and_repository_is_untouched(self):
        paths=[ROOT/'theme/generated.json',ROOT/'terminal/palette.json',ROOT/'compositor/niri/wallpaper.toml']
        before=[p.read_bytes() for p in paths]
        bundle=self.bundle()
        for file in ('semantic.json','wallpaper.toml','overview.png','backdrop.json','niri.kdl','notifications.conf','brave/manifest.json','terminal/kitty/colors.conf','terminal/fish/theme.fish','terminal/fastfetch/config.jsonc'):
            self.assertTrue((bundle/file).is_file(),file)
        self.assertEqual([p.read_bytes() for p in paths],before)
        self.assertIsNone(runtime.current())

    def test_publication_and_previous_are_reversible(self):
        first=self.bundle();second=self.bundle()
        runtime.publish(first,live=False);runtime.publish(second,live=False)
        self.assertEqual(runtime.current(),second)
        self.assertEqual((runtime.state()/'previous').resolve(),first)
        with patch.object(runtime,'reload'):
            runtime.rollback()
        self.assertEqual(runtime.current(),first)
        self.assertEqual((runtime.state()/'previous').resolve(),second)

    def test_failed_validation_preserves_current(self):
        first=self.bundle();runtime.publish(first,live=False)
        with patch.object(runtime.subprocess,'run',side_effect=RuntimeError('validation failed')):
            with self.assertRaises(RuntimeError):self.bundle()
        self.assertEqual(runtime.current(),first)
        self.assertEqual(list((runtime.state()/'revisions').iterdir()),[first])

    def test_failed_reload_restores_palette_wallpaper_and_brave(self):
        first=self.bundle();runtime.publish(first,live=False)
        self.palette=pipeline.derive({'primary':'#98ccf9','secondary':'#bdc7d5'},'blue','hash2')
        second=self.bundle()
        original=(runtime.state()/'brave/manifest.json').read_bytes()
        with patch.object(runtime,'reload',side_effect=[RuntimeError('reload failed'),None]):
            with self.assertRaises(RuntimeError):runtime.publish(second,wallpaper=True)
        self.assertEqual(runtime.current(),first)
        self.assertEqual((runtime.state()/'brave/manifest.json').read_bytes(),original)
        self.assertTrue(Path(runtime.config()['image']).is_file())

    def test_invalid_image_cannot_publish(self):
        self.image.write_bytes(b'not an image')
        with self.assertRaises(OSError):self.bundle()
        self.assertIsNone(runtime.current())

    def test_identical_palette_and_image_skip_reload(self):
        first=self.bundle();runtime.publish(first,live=False)
        p=dict(self.palette,source='same image under another path')
        with patch.object(runtime,'reload') as reload:
            runtime.apply(p,image=self.image,wallpaper=True)
        reload.assert_not_called()
        self.assertEqual(runtime.current(),first)

    def test_brave_manifest_is_permissionless_and_semantic(self):
        manifest=brave(self.palette)
        self.assertEqual(manifest['manifest_version'],3)
        self.assertNotIn('permissions',manifest)
        self.assertNotIn('background',manifest)
        self.assertEqual(manifest['theme']['colors']['frame'],[37,36,41])
        self.assertEqual(manifest['theme']['colors']['toolbar_button_icon'],[255,181,153])
        other=brave(pipeline.derive({'primary':'#98ccf9','secondary':'#bdc7d5'},'blue','hash2'))
        self.assertNotEqual(other['version'],manifest['version'])
        self.assertEqual(MODES['brave']['mode'],'RELOADABLE')

    def test_explicit_wallpaper_restart_resets_only_wallpaper_rate_limit(self):
        import subprocess
        with patch.dict(os.environ, {'NIRI_SOCKET': 'test'}), patch.object(runtime.shutil, 'which', return_value=None), patch.object(runtime.Path, 'iterdir', return_value=[]), patch('spotify_theme.refresh'), patch('gtk_theme.refresh'), patch.object(runtime.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stdout='', stderr='')) as run:
            runtime.reload(wallpaper=True)
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands[:2], [
            ['systemctl', '--user', 'reset-failed', 'desktop-foundation-wallpaper.service'],
            ['systemctl', '--user', 'restart', 'desktop-foundation-wallpaper.service']])
        self.assertNotIn(['systemctl', '--user', 'restart', 'niri.service'], commands)

    def test_retention_failure_does_not_roll_back_successful_publication(self):
        import subprocess
        first=self.bundle();runtime.publish(first,live=False)
        with patch.object(runtime.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'cache', stderr='ownership mismatch')), patch('sys.stderr') as diagnostic:
            runtime.maintain_cache()
        self.assertEqual(runtime.current(), first)
        self.assertTrue(any('maintenance deferred' in str(c) for c in diagnostic.write.call_args_list))
