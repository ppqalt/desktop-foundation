import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import spotify_setup as setup


class SpotifyToolUpgrade(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name); self.apps = self.base / 'apps'; self.state = self.base / 'state'
        self.apps.mkdir(); self.state.mkdir()
        self.sources = json.loads((setup.ROOT / 'apps/spotify/sources.json').read_text())
        self.destination = self.apps / 'spicetify'; self.destination.mkdir()
        (self.destination / '.foundation-source.json').write_text(json.dumps(self.sources['previous_spicetify'][0]))
        executable = self.destination / 'spicetify'; executable.write_text('#!/bin/sh\necho 2.45.1\n'); executable.chmod(0o755)
        self.package = self.base / 'tool.tar.gz'
        with tarfile.open(self.package, 'w:gz') as archive:
            content = b'#!/bin/sh\necho 2.45.3\n'; entry = tarfile.TarInfo('spicetify'); entry.size = len(content); entry.mode = 0o755
            archive.addfile(entry, io.BytesIO(content))
        for item in [patch.object(setup, 'APPS', self.apps), patch.object(setup, 'STATE', self.state), patch.object(setup, 'download', return_value=self.package)]:
            item.start(); self.addCleanup(item.stop)

    def test_known_previous_pin_upgrades_once_and_can_restore(self):
        setup.upgrade_spicetify(self.sources); setup.upgrade_spicetify(self.sources)
        self.assertEqual(json.loads((self.destination / '.foundation-source.json').read_text()), self.sources['spicetify'])
        self.assertTrue((self.apps / 'spicetify-backup-2.45.1').is_dir())
        setup.restore_tool_upgrade()
        self.assertEqual(json.loads((self.destination / '.foundation-source.json').read_text()), self.sources['previous_spicetify'][0])

    def test_unknown_preexisting_tool_is_preserved(self):
        stamp = self.destination / '.foundation-source.json'; stamp.write_text('{}')
        before = (self.destination / 'spicetify').read_bytes()
        with self.assertRaises(RuntimeError): setup.upgrade_spicetify(self.sources)
        self.assertEqual(before, (self.destination / 'spicetify').read_bytes())
        self.assertFalse((self.state / 'tool-upgrade.json').exists())

    def test_failed_publication_restores_old_tool(self):
        real_rename = Path.rename
        def rename(path, destination):
            if path.name == 'tool': raise OSError('publish failed')
            return real_rename(path, destination)
        with patch.object(Path, 'rename', rename):
            with self.assertRaises(OSError): setup.upgrade_spicetify(self.sources)
        self.assertIn('2.45.1', (self.destination / 'spicetify').read_text())

    def test_externally_modified_tool_refuses_restore(self):
        setup.upgrade_spicetify(self.sources)
        (self.destination / 'spicetify').write_text('external edit')
        with self.assertRaises(RuntimeError): setup.restore_tool_upgrade()
        self.assertTrue((self.apps / 'spicetify-backup-2.45.1').is_dir())

    def test_interrupted_upgrade_recovers_original_backup(self):
        backup = self.apps / 'spicetify-backup-2.45.1'; self.destination.rename(backup)
        (self.state / 'tool-upgrade.json').write_text(json.dumps({'phase': 'prepared', 'backup': str(backup)}))
        setup.recover_tool_upgrade()
        self.assertTrue(self.destination.is_dir()); self.assertFalse((self.state / 'tool-upgrade.json').exists())
