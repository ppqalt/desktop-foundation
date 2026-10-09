"""Application-default recovery through the real Rust CLI in isolated XDG homes."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import application_roles as roles


class ApplicationRoles(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.checkout = self.base / 'checkout'
        self.bin = self.base / 'bin'
        self.data = self.base / 'data'
        for path in [self.checkout / 'config', self.bin, self.data / 'applications']:
            path.mkdir(parents=True)
        self.configured = json.loads((ROOT / 'config/application-roles.json').read_text())
        self.write_config()
        for role in self.configured.values():
            self.executable(role['executable'])
            self.desktop(role['desktop'], role['executable'])
        query = '''#!/usr/bin/env python3
import configparser, os, sys
from pathlib import Path
parser = configparser.ConfigParser(interpolation=None)
parser.read(Path(os.environ['XDG_CONFIG_HOME']) / 'mimeapps.list')
mime = sys.argv[-1] if Path(sys.argv[0]).name == 'xdg-mime' else 'x-scheme-handler/http'
print(parser.get('Default Applications', mime, fallback='').split(';')[0])
'''
        self.executable('xdg-mime', query)
        self.executable('xdg-settings', query)
        env = patch.dict(os.environ, {
            'HOME': str(self.base), 'XDG_CONFIG_HOME': str(self.base / 'config'),
            'XDG_STATE_HOME': str(self.base / 'state'), 'XDG_DATA_HOME': str(self.data),
            'XDG_DATA_DIRS': '', 'PATH': str(self.bin) + os.pathsep + os.environ['PATH'],
        })
        env.start(); self.addCleanup(env.stop)
        root = patch.object(roles, 'ROOT', self.checkout)
        root.start(); self.addCleanup(root.stop)
        self.file, self.journal = roles.paths()
        self.file.parent.mkdir()
        self.original = '# preserve comment\n[Default Applications]\ninode/directory=old-files.desktop;\napplication/x-unrelated=keep.desktop;\n\n[Added Associations]\nimage/png=another.desktop;\n'
        self.file.write_text(self.original)
        self.sentinels = [self.base / target for target in (
            'browser/Profile/History', 'Steam/userdata/unchanged', 'fish/config.fish',
            'niri/custom.kdl', 'ssh/config',
        )]
        for path in self.sentinels:
            path.parent.mkdir(parents=True, exist_ok=True); path.write_text('untouched')

    def write_config(self):
        (self.checkout / 'config/application-roles.json').write_text(json.dumps(self.configured))

    def executable(self, name, text='#!/bin/sh\nexit 0\n'):
        path = self.bin / name; path.write_text(text); path.chmod(0o755)

    def desktop(self, name, executable):
        (self.data / 'applications' / name).write_text(
            '[Desktop Entry]\nType=Application\nExec=' + executable + ' %U\n')

    def query(self, mime):
        return subprocess.check_output(['xdg-mime', 'query', 'default', mime], text=True).strip()

    def saved(self):
        return json.loads(self.journal.read_text())

    def test_apply_rerun_preserves_existing_machine(self):
        roles.apply(True); first = self.file.read_bytes(); journal = self.journal.read_bytes()
        self.assertFalse(roles.apply(True)['changed'])
        self.assertEqual(first, self.file.read_bytes()); self.assertEqual(journal, self.journal.read_bytes())
        self.assertIn('application/x-unrelated=keep.desktop;', self.file.read_text())
        for path in self.sentinels:
            self.assertEqual(path.read_text(), 'untouched')

    def test_restore_preserves_later_unrelated_edits(self):
        roles.apply(); self.file.write_text(self.file.read_text().replace('keep.desktop;', 'new-user-choice.desktop;'))
        roles.restore()
        self.assertIn('inode/directory=old-files.desktop;', self.file.read_text())
        self.assertIn('application/x-unrelated=new-user-choice.desktop;', self.file.read_text())
        self.assertIn('[Added Associations]\nimage/png=another.desktop;', self.file.read_text())
        self.assertNotIn('application/pdf=', self.file.read_text())

    def test_external_owned_change_refused_without_writes(self):
        roles.apply(); self.file.write_text(self.file.read_text().replace('org.gnome.Nautilus.desktop;', 'foreign.desktop;'))
        before = self.file.read_bytes(); journal = self.journal.read_bytes()
        self.assertTrue(roles.ownership_conflicts())
        with self.assertRaises(RuntimeError): roles.apply(True)
        with self.assertRaises(RuntimeError): roles.restore()
        self.assertEqual(self.file.read_bytes(), before); self.assertEqual(self.journal.read_bytes(), journal)

    def test_missing_app_refused_before_journal_or_defaults(self):
        before = self.file.read_bytes()
        (self.data / 'applications' / self.configured['pdf']['desktop']).unlink()
        with self.assertRaisesRegex(RuntimeError, 'Missing pdf'): roles.apply()
        self.assertEqual(before, self.file.read_bytes()); self.assertFalse(self.journal.exists())

    def test_invalid_desktop_executable_refused_before_publication(self):
        self.desktop(self.configured['files']['desktop'], 'unrelated-program')
        with self.assertRaisesRegex(RuntimeError, 'differs'): roles.apply()
        self.assertEqual(self.file.read_text(), self.original); self.assertFalse(self.journal.exists())

    def test_personal_application_metadata_needs_no_mime_list(self):
        app = dict(self.configured['terminal']); app.pop('mimes'); app['source'] = 'package repository'
        self.assertEqual(roles.validate_role('Kitty', app), self.data / 'applications' / app['desktop'])

    def test_first_install_restore_removes_new_empty_file(self):
        self.file.unlink(); roles.apply(); self.assertTrue(self.file.exists())
        roles.restore(); self.assertFalse(self.file.exists())

    def test_core_to_personal_retains_original_for_restore(self):
        self.configured['browser']['desktop'] = 'core-browser.desktop'
        self.configured['personalBrowser']['desktop'] = 'personal-browser.desktop'
        for key in ['browser', 'personalBrowser']:
            role = self.configured[key]; self.desktop(role['desktop'], role['executable'])
        self.write_config()
        roles.apply(); self.assertEqual(self.query('x-scheme-handler/http'), 'core-browser.desktop')
        roles.apply(True); self.assertEqual(self.query('x-scheme-handler/http'), 'personal-browser.desktop')
        roles.restore(); self.assertEqual(self.file.read_text().strip(), self.original.strip())

    def test_interrupted_publication_can_resume(self):
        roles.apply(); saved = self.saved(); entry = saved['entries']['inode/directory']
        entry['pending'] = 'org.gnome.Nautilus.desktop;'; entry['managed'] = entry['original']
        self.journal.write_text(json.dumps(saved)); roles.apply()
        self.assertNotIn('pending', self.saved()['entries']['inode/directory'])

    def test_interrupted_restore_after_removal_can_resume(self):
        self.file.unlink(); roles.apply(); saved = self.saved()
        for entry in saved['entries'].values(): entry['pending'] = None
        self.journal.write_text(json.dumps(saved)); self.file.unlink()
        roles.restore(); self.assertFalse(self.file.exists()); self.assertFalse(self.journal.exists())

    def test_legacy_migration_adopts_only_actual_changed_keys(self):
        self.journal.parent.mkdir(parents=True)
        backup = self.journal.parent / 'mimeapps.original'; backup.write_text(self.original)
        managed = self.original.replace('[Default Applications]\n', '[Default Applications]\nx-scheme-handler/http=brave-origin-nightly.desktop;\n')
        legacy = self.journal.parent / 'mimeapps.json'
        legacy.write_text(json.dumps({'path': str(self.file), 'existed': True, 'managed': managed}))
        self.file.write_text(managed.replace('keep.desktop;', 'later-user-choice.desktop;'))
        self.assertEqual(roles.restore()['restored'], 1)
        self.assertIn('later-user-choice.desktop;', self.file.read_text())
        self.assertNotIn('x-scheme-handler/http=', self.file.read_text())
        self.assertEqual(backup.read_text(), self.original)
        self.assertFalse(legacy.exists()); self.assertTrue(legacy.with_name('mimeapps.v011-migrated.json').exists())

    def test_legacy_foreign_change_retains_recovery_evidence(self):
        self.journal.parent.mkdir(parents=True)
        (self.journal.parent / 'mimeapps.original').write_text(self.original)
        managed = self.original.replace('old-files.desktop;', 'org.gnome.Nautilus.desktop;')
        legacy = self.journal.parent / 'mimeapps.json'
        legacy.write_text(json.dumps({'path': str(self.file), 'existed': True, 'managed': managed}))
        with self.assertRaisesRegex(RuntimeError, 'changed externally'): roles.restore()
        self.assertTrue(legacy.exists()); self.assertFalse(self.journal.exists())
        self.assertEqual(self.file.read_text(), self.original)

    def test_invalid_journal_is_not_overwritten(self):
        roles.apply(); saved = self.saved(); saved['entries']['inode/directory']['pending'] = 4
        self.journal.write_text(json.dumps(saved)); before = self.journal.read_bytes(); mime = self.file.read_bytes()
        with self.assertRaises(RuntimeError): roles.apply()
        with self.assertRaises(RuntimeError): roles.restore()
        self.assertEqual(self.journal.read_bytes(), before); self.assertEqual(self.file.read_bytes(), mime)

    def test_duplicate_section_refused_before_writes(self):
        self.file.write_text(self.original + '\n[Default Applications]\ntext/plain=foreign.desktop;\n')
        before = self.file.read_bytes()
        with self.assertRaisesRegex(RuntimeError, 'Duplicate'): roles.apply()
        self.assertEqual(self.file.read_bytes(), before); self.assertFalse(self.journal.exists())

    def test_symlink_journal_refused_without_touching_target(self):
        self.journal.parent.mkdir(parents=True)
        target = self.base / 'foreign.json'; target.write_text('{}')
        self.journal.symlink_to(target)
        with self.assertRaises(RuntimeError): roles.apply()
        self.assertEqual(target.read_text(), '{}'); self.assertEqual(self.file.read_text(), self.original)

    def test_native_check_returns_failure_status_and_compatibility_details(self):
        roles.apply()
        self.file.write_text(self.file.read_text().replace('org.gnome.Papers.desktop;', 'foreign.desktop;'))
        result = subprocess.run([str(roles.BACKEND), '--root', str(self.checkout), 'app-roles', 'check'], text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(json.loads(result.stdout)['failures'])
        self.assertTrue(roles.check())


if __name__ == '__main__':
    unittest.main()
