"""Native theme CLI integration with private paths and synthetic Matugen only.

Complete palette goldens are frozen from 1ca553c's Python implementation,
including contrast adjustment and saturation/rounding edge cases. No legacy
implementation runs inside these tests or in the installed desktop.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'
POLICY = 'graphite-v1'
GOLDENS = json.loads(r'''[{"material":{"primary":"#98ccf9","secondary":"#bdc7d5"},"palette":{"background":"#1f262f","elevated":"#2e3845","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#495a6d","accent":"#98ccf9","selected":"#3e5065","selectionBorder":"#607c9b","iconTile":"#293540","hover":"#323e4e","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#9fcbf2","windowActive":"#5e7791","windowInactive":"#364150","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#ffb599","secondary":"#dfbfaf"},"palette":{"background":"#252429","elevated":"#38363c","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#5c565c","accent":"#ffb599","selected":"#4d4c57","selectionBorder":"#7f757e","iconTile":"#333236","hover":"#3d3c44","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#f9b79d","windowActive":"#7b7176","windowInactive":"#403f46","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#fbb5ed","secondary":"#cdc2d0"},"palette":{"background":"#25242e","elevated":"#383645","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#5b566b","accent":"#fbb5ed","selected":"#4d4c65","selectionBorder":"#7e7598","iconTile":"#323241","hover":"#3c3c4e","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#f2b8e7","windowActive":"#7a718e","windowInactive":"#403f4f","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#000000","secondary":"#000001"},"palette":{"background":"#161920","elevated":"#1f242d","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#2e3540","accent":"#9d9d9d","selected":"#242d3b","selectionBorder":"#323f50","iconTile":"#161c22","hover":"#1f2630","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#9d9d9d","windowActive":"#343e4c","windowInactive":"#272d37","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#010203","secondary":"#808080"},"palette":{"background":"#161920","elevated":"#1f242d","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#2e3640","accent":"#9d9d9d","selected":"#242e3b","selectionBorder":"#334051","iconTile":"#161c23","hover":"#1f2631","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#9d9d9d","windowActive":"#343e4c","windowInactive":"#272d37","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#0000ff","secondary":"#ff0000"},"palette":{"background":"#1c1e29","elevated":"#282b3d","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#3d415f","accent":"#9d9dff","selected":"#343a56","selectionBorder":"#4e5482","iconTile":"#222535","hover":"#2b2f43","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#b09deb","windowActive":"#4b507c","windowInactive":"#303347","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#ff0000","secondary":"#00ff00"},"palette":{"background":"#251920","elevated":"#37252d","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#593941","accent":"#ff7979","selected":"#4c2e3d","selectionBorder":"#774754","iconTile":"#301f25","hover":"#3c2631","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#e49279","windowActive":"#744550","windowInactive":"#402d37","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}},{"material":{"primary":"#98CCF9","secondary":"#BDC7D5"},"palette":{"background":"#1f262f","elevated":"#2e3845","foreground":"#eef1f6","muted":"#939eae","subtle":"#616e80","border":"#495a6d","accent":"#98CCF9","selected":"#3e5065","selectionBorder":"#607c9b","iconTile":"#293540","hover":"#323e4e","error":"#f1a5a5","scrim":"#50080b10","source":"fixture","accentStrong":"#9fcbf2","windowActive":"#5e7791","windowInactive":"#364150","wallpaperHash":"1111111111111111111111111111111111111111111111111111111111111111","policy":"graphite-v1"}}]''')

FAKE_MATUGEN = r'''import json, os, sys
from pathlib import Path
args = sys.argv[1:]
config = Path(args[args.index('--config') + 1])
assert config.is_absolute() and config.is_file()
assert config.read_text().strip() == '[config]\n[templates]'
assert config.parent != Path(os.environ['XDG_CONFIG_HOME'])
assert '--dry-run' in args
assert args[args.index('--mode') + 1] == 'dark'
assert args[args.index('--json') + 1] == 'hex'
assert args[args.index('--source-color-index') + 1] == '0'
assert args[-2] == 'image' and Path(args[-1]).is_file()
with open(os.environ['DF_THEME_LOG'], 'a') as trace:
    trace.write(json.dumps({'args': args, 'config': str(config)}) + '\n')
mode = os.environ.get('DF_THEME_MODE', 'modern')
material = {'primary': '#98ccf9', 'secondary': '#bdc7d5'}
if mode == 'fail':
    print('synthetic generator failure', file=sys.stderr)
    sys.exit(2)
if mode == 'malformed':
    print('{not-json')
    sys.exit(0)
if mode == 'oversized':
    print(json.dumps({'colors': {'dark': material}, 'padding': 'x' * (2 * 1024 * 1024)}))
    sys.exit(0)
if mode == 'bad-color':
    material['primary'] = '#wrong'
if mode == 'changed-source':
    Path(args[-1]).write_bytes(b'source changed while extraction was running')
if mode == 'replaced-source':
    image = Path(args[-1])
    replacement = image.with_name('replacement.bin')
    replacement.write_bytes(image.read_bytes())
    os.replace(replacement, image)
if mode == 'legacy':
    colors = {'dark': material}
else:
    colors = {name: {'dark': {'color': value}, 'light': {'color': '#ffffff'}}
              for name, value in material.items()}
print(json.dumps({'colors': colors}))
'''


class NativeTheme(unittest.TestCase):
    def setUp(self):
        if not BINARY.is_file():
            raise RuntimeError('Build native/foundation before running native theme tests')
        self.temp = tempfile.TemporaryDirectory(prefix='foundation-native-theme-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'checkout'
        fallback = self.root / 'theme/fallback'
        fallback.mkdir(parents=True)
        shutil.copy2(ROOT / 'theme/fallback/semantic.json', fallback / 'semantic.json')
        self.bin = self.base / 'bin'; self.bin.mkdir()
        for name in ('cache', 'state', 'config', 'runtime'):
            (self.base / name).mkdir(mode=0o700)
        self.log = self.base / 'matugen.log'
        fake = self.bin / 'matugen'
        fake.write_text('#!' + sys.executable + '\n' + FAKE_MATUGEN)
        fake.chmod(0o755)
        self.env = {'PATH': str(self.bin), 'HOME': str(self.base), 'LC_ALL': 'C',
                    'LANG': 'C', 'TMPDIR': str(self.base),
                    'XDG_CACHE_HOME': str(self.base / 'cache'),
                    'XDG_STATE_HOME': str(self.base / 'state'),
                    'XDG_CONFIG_HOME': str(self.base / 'config'),
                    'XDG_RUNTIME_DIR': str(self.base / 'runtime'),
                    'DF_THEME_LOG': str(self.log)}
        self.image = self.root / 'wallpaper.bin'
        self.image.write_bytes(b'isolated wallpaper bytes for palette extraction')
        self.digest = hashlib.sha256(self.image.read_bytes()).hexdigest()
        self.cache = self.base / 'cache/desktop-foundation/themes'
        self.target = self.cache / (self.digest + '-' + POLICY + '.json')

    def invoke(self, *arguments, input=None, check=True):
        return subprocess.run([str(BINARY), '--root', str(self.root), 'theme', *arguments],
                              input=input, env=self.env, cwd=self.root, capture_output=True, text=True,
                              check=check, timeout=8)

    def generate(self, image=None, check=True):
        return self.invoke('generate', str(image or self.image), check=check)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def frozen_palette(self):
        palette = dict(GOLDENS[0]['palette'])
        palette.update(wallpaperHash=self.digest, source='old-location/wallpaper.bin')
        return palette

    def test_complete_python_baseline_palette_parity(self):
        for case in GOLDENS:
            with self.subTest(primary=case['material']['primary']):
                result = self.invoke('derive', '--source', 'fixture', '--hash', '1' * 64,
                                     input=json.dumps(case['material']))
                self.assertEqual(json.loads(result.stdout), case['palette'])
        # A source so bright that neutral text loses contrast was rejected before
        # migration and must still fail rather than silently changing typography.
        result = self.invoke('derive', '--source', 'fixture', '--hash', '1' * 64,
                             input=json.dumps({'primary': '#ffffff', 'secondary': '#000000'}),
                             check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('contrast', result.stderr.lower())
        self.assertFalse(self.cache.exists())

    def test_current_and_legacy_matugen_output_and_isolated_invocation(self):
        for layout in ('modern', 'legacy'):
            with self.subTest(layout=layout):
                self.env['DF_THEME_MODE'] = layout
                result = json.loads(self.generate().stdout)
                expected = self.frozen_palette(); expected['source'] = 'wallpaper.bin'
                self.assertEqual(result, {'palette': expected, 'cached': False})
                self.assertEqual(json.loads(self.target.read_text()), expected)
                call = self.calls()[-1]
                self.assertEqual(call['args'][-1], str(self.image.resolve()))
                # Config was private and removed after the one-shot process.
                self.assertFalse(Path(call['config']).exists())
                self.target.unlink()
        self.assertEqual(len(self.calls()), 2)
        self.assertEqual(list(self.cache.iterdir()), [])

    def test_unusual_filename_is_one_argument_and_source_is_canonical(self):
        unusual = self.root / 'wall $(touch OWNED); `x` with spaces.bin'
        unusual.write_bytes(self.image.read_bytes())
        alias = self.base / 'alias-image'; alias.symlink_to(unusual)
        result = json.loads(self.generate(alias).stdout)
        self.assertEqual(result['palette']['source'], unusual.name)
        self.assertEqual(result['palette']['wallpaperHash'], self.digest)
        self.assertEqual(self.calls()[0]['args'][-1], str(unusual.resolve()))
        self.assertFalse((self.base / 'OWNED').exists())
        self.assertFalse((self.root / 'OWNED').exists())
        outside = self.base / 'outside.bin'; outside.write_bytes(self.image.read_bytes())
        result = json.loads(self.generate(outside).stdout)
        self.assertTrue(result['cached'])
        self.assertEqual(result['palette']['source'], str(outside.resolve()))
        self.assertEqual(len(self.calls()), 1)

    def test_original_v1_cache_reused_without_generator_execution(self):
        self.cache.mkdir(parents=True)
        old = self.frozen_palette()
        original = json.dumps(old, indent=2) + '\n'
        self.target.write_text(original)
        self.env['DF_THEME_MODE'] = 'fail'
        result = json.loads(self.generate().stdout)
        self.assertTrue(result['cached'])
        expected = dict(old, source='wallpaper.bin')
        self.assertEqual(result['palette'], expected)
        self.assertEqual(self.target.read_text(), original)
        self.assertEqual(self.calls(), [])

    def test_invalid_cache_is_preserved_and_never_invokes_matugen(self):
        self.cache.mkdir(parents=True)
        good = self.frozen_palette()
        invalid = ['{not-json', json.dumps(dict(good, policy='different-policy')),
                   json.dumps(dict(good, wallpaperHash='2' * 64)),
                   json.dumps(dict(good, accent='invalid')),
                   json.dumps(dict(good, foreground='#101010')),
                   json.dumps(dict(good, muted='#939eff')),
                   json.dumps(dict(good, padding='x' * (80 * 1024)))]
        for content in invalid:
            with self.subTest(content=content[:70]):
                self.target.write_text(content)
                result = self.generate(check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.target.read_text(), content)
                self.assertEqual(self.calls(), [])
                self.assertEqual(list(self.cache.iterdir()), [self.target])

    def test_cache_symlink_is_not_replaced_or_followed(self):
        self.cache.mkdir(parents=True)
        external = self.base / 'external.json'
        external.write_text(json.dumps(self.frozen_palette()))
        before = external.read_bytes()
        self.target.symlink_to(external)
        result = self.generate(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.target.is_symlink())
        self.assertEqual(external.read_bytes(), before)
        self.assertEqual(self.calls(), [])

    def test_untrusted_generator_output_is_bounded_and_failure_leaves_cache_intact(self):
        self.cache.mkdir(parents=True)
        last_good = self.cache / ('2' * 64 + '-' + POLICY + '.json')
        last_good.write_text(json.dumps(GOLDENS[1]['palette']))
        before = last_good.read_bytes()
        for mode in ('malformed', 'bad-color', 'oversized', 'fail'):
            with self.subTest(mode=mode):
                self.env['DF_THEME_MODE'] = mode
                result = self.generate(check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(last_good.read_bytes(), before)
                self.assertEqual(list(self.cache.iterdir()), [last_good])
                self.assertFalse(self.target.exists())
                if mode == 'oversized':
                    self.assertTrue(any(word in result.stderr.lower()
                                        for word in ('large', 'limit', 'exceeded')), result.stderr)
        self.assertEqual(len(self.calls()), 4)
        self.assertEqual(list((self.base / 'state').iterdir()), [])

    def test_material_stdin_is_bounded_before_palette_derivation(self):
        result = self.invoke('derive', '--source', 'fixture', '--hash', '1' * 64,
                             input=json.dumps({'primary': '#98ccf9', 'secondary': '#bdc7d5',
                                               'padding': 'x' * (80 * 1024)}), check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('large', result.stderr.lower())
        self.assertFalse(self.cache.exists())

    def test_non_regular_wallpaper_is_rejected_without_blocking_or_spawning(self):
        fifo = self.root / 'wallpaper-fifo'
        os.mkfifo(fifo)
        for image in (fifo, self.root, self.root / 'missing.png'):
            with self.subTest(image=image.name):
                result = self.generate(image, check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])
                self.assertFalse(self.cache.exists())

    def test_invalid_cli_and_fallback_metadata_leave_existing_cache_intact(self):
        self.cache.mkdir(parents=True)
        self.target.write_text(json.dumps(self.frozen_palette()))
        before = self.target.read_bytes()
        result = self.invoke('derive', '--source', 'fixture', check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [])
        fallback = self.root / 'theme/fallback/semantic.json'
        fallback.write_text('{not-json')
        result = self.generate(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.target.read_bytes(), before)
        self.assertEqual(self.calls(), [])

    def test_changed_or_replaced_source_does_not_publish_stale_hash_palette(self):
        self.cache.mkdir(parents=True)
        last_good = self.cache / ('2' * 64 + '-' + POLICY + '.json')
        last_good.write_text(json.dumps(GOLDENS[1]['palette']))
        before = last_good.read_bytes()
        image_bytes = self.image.read_bytes()
        for mode in ('changed-source', 'replaced-source'):
            with self.subTest(mode=mode):
                self.image.write_bytes(image_bytes)
                self.env['DF_THEME_MODE'] = mode
                result = self.generate(check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('changed', result.stderr.lower())
                self.assertEqual(last_good.read_bytes(), before)
                self.assertFalse(self.target.exists())
                self.assertEqual(list(self.cache.iterdir()), [last_good])
        self.assertEqual(len(self.calls()), 2)


if __name__ == '__main__':
    unittest.main()
