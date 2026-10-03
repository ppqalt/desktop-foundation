import hashlib
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import niri_wallpaper as wallpaper
from theme_pipeline import file_hash


class Backdrop(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.image = self.base / 'input.png'
        source = Image.new('RGB', (200, 140))
        source.putdata([(x % 255, (x * 7) % 255, (x * 13) % 255) for x in range(200 * 140)])
        source.save(self.image)
        env = patch.dict(os.environ, {'XDG_CACHE_HOME': str(self.base / 'cache')})
        env.start(); self.addCleanup(env.stop)

    def test_published_blur_pixels_and_hash_match_existing_algorithm(self):
        self.assertEqual(file_hash(self.image), hashlib.sha256(self.image.read_bytes()).hexdigest())
        for mode in ['fill', 'fit', 'center', 'tile']:
            with Image.open(self.image) as original:
                expected = original.convert('RGB')
                if mode in {'fill', 'fit'}: expected.thumbnail((1920, 1920))
                expected = expected.filter(ImageFilter.GaussianBlur(24))
            target = wallpaper.prepare_backdrop(self.image, mode)
            with Image.open(target) as actual:
                self.assertEqual(actual.tobytes(), expected.tobytes())
                self.assertEqual(actual.size, expected.size)
            before = (target.read_bytes(), target.stat().st_ino)
            self.assertEqual(wallpaper.prepare_backdrop(self.image, mode), target)
            self.assertEqual((target.read_bytes(), target.stat().st_ino), before)

    def test_interrupted_write_never_publishes_partial_png(self):
        def broken(image, file, **kwargs):
            file.write(b'partial')
            raise OSError('disk failure')
        with patch.object(Image.Image, 'save', broken):
            with self.assertRaises(OSError): wallpaper.prepare_backdrop(self.image, 'fill')
        cache = self.base / 'cache/desktop-foundation/overview'
        self.assertEqual(list(cache.iterdir()), [])

    def test_corrupt_reproducible_cache_is_repaired_atomically(self):
        target = wallpaper.prepare_backdrop(self.image, 'fill')
        expected = target.read_bytes(); target.write_bytes(b'corrupt')
        self.assertEqual(wallpaper.prepare_backdrop(self.image, 'fill').read_bytes(), expected)
        outside = self.base / 'outside'; outside.write_text('preserve')
        target.unlink(); target.symlink_to(outside)
        with self.assertRaises(RuntimeError): wallpaper.prepare_backdrop(self.image, 'fill')
        self.assertEqual(outside.read_text(), 'preserve')
