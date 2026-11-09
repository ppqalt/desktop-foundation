"""Pinned downloads retain their identity when rolling repositories expire files."""
import hashlib
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import spotify_setup as setup


class PinnedDownload(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.cache = Path(self.temp.name)
        self.data = b'exact pinned package'
        self.source = {'url':'https://mirror.invalid/package.zst',
                       'fallback_urls':['https://archive.invalid/package.zst'],
                       'sha256':hashlib.sha256(self.data).hexdigest()}
        item=patch.object(setup,'CACHE',self.cache);item.start();self.addCleanup(item.stop)

    def test_missing_package_uses_same_checksum_from_archive(self):
        missing=HTTPError(self.source['url'],404,'Gone',{},None)
        with patch.object(setup.urllib.request,'urlopen',side_effect=[missing,io.BytesIO(self.data)]) as request:
            result=setup.download(self.source)
        self.assertEqual(result.read_bytes(),self.data)
        self.assertEqual([c.args[0] for c in request.call_args_list],
                         [self.source['url'],self.source['fallback_urls'][0]])
        self.assertFalse(list(self.cache.glob('*.part')))

    def test_checksum_failure_never_falls_back_or_publishes(self):
        with patch.object(setup.urllib.request,'urlopen',return_value=io.BytesIO(b'wrong package')) as request:
            with self.assertRaisesRegex(RuntimeError,'checksum mismatch'):setup.download(self.source)
        self.assertEqual(request.call_count,1)
        self.assertEqual(list(self.cache.iterdir()),[])

    def test_authentication_failure_does_not_try_archive(self):
        denied=HTTPError(self.source['url'],403,'Denied',{},io.BytesIO())
        self.addCleanup(denied.close)
        with patch.object(setup.urllib.request,'urlopen',side_effect=denied) as request:
            with self.assertRaises(HTTPError):setup.download(self.source)
        self.assertEqual(request.call_count,1)
        self.assertEqual(list(self.cache.iterdir()),[])

    def test_verified_cache_is_offline_and_bad_cache_is_not_replaced(self):
        path=self.cache/'package.zst';path.write_bytes(self.data)
        with patch.object(setup.urllib.request,'urlopen') as request:
            self.assertEqual(setup.download(self.source),path)
            path.write_bytes(b'user-changed')
            with self.assertRaisesRegex(RuntimeError,'checksum mismatch'):setup.download(self.source)
        request.assert_not_called()
        self.assertEqual(path.read_bytes(),b'user-changed')
