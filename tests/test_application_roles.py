import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import application_roles as roles


class ApplicationRoles(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        env = patch.dict(os.environ, {'XDG_CONFIG_HOME': str(self.base / 'config'), 'XDG_STATE_HOME': str(self.base / 'state')})
        env.start(); self.addCleanup(env.stop)
        self.file, self.journal = roles.paths()
        self.file.parent.mkdir()
        self.original = '# preserve comment\n[Default Applications]\ninode/directory=old-files.desktop;\napplication/x-unrelated=keep.desktop;\n\n[Added Associations]\nimage/png=another.desktop;\n'
        self.file.write_text(self.original)
        for target in ('browser/Profile/History', 'Steam/userdata/unchanged', 'fish/config.fish', 'niri/custom.kdl', 'ssh/config'):
            path = self.base / target; path.parent.mkdir(parents=True, exist_ok=True); path.write_text('untouched')
        validation = patch.object(roles, 'validate_role', return_value=Path('/verified.desktop'))
        validation.start(); self.addCleanup(validation.stop)
        query = patch.object(roles, 'query', side_effect=lambda mime: roles.values(self.file.read_text()).get(mime, '').rstrip(';'))
        query.start(); self.addCleanup(query.stop)
        check = patch.object(roles.subprocess, 'check_output', side_effect=lambda *a, **k: roles.values(self.file.read_text()).get('x-scheme-handler/http', '').rstrip(';'))
        check.start(); self.addCleanup(check.stop)

    def test_apply_rerun_preserves_existing_machine(self):
        roles.apply(True); first = self.file.read_bytes(); journal = self.journal.read_bytes(); roles.apply(True)
        self.assertEqual(first, self.file.read_bytes()); self.assertEqual(journal, self.journal.read_bytes())
        self.assertIn('application/x-unrelated=keep.desktop;', self.file.read_text())
        for path in [self.base / p for p in ('browser/Profile/History', 'Steam/userdata/unchanged', 'fish/config.fish', 'niri/custom.kdl', 'ssh/config')]:
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
        before = self.file.read_bytes()
        with self.assertRaises(RuntimeError): roles.apply(True)
        with self.assertRaises(RuntimeError): roles.restore()
        self.assertEqual(self.file.read_bytes(), before)

    def test_missing_app_refused_before_journal_or_defaults(self):
        before = self.file.read_bytes()
        with patch.object(roles, 'validate_role', side_effect=RuntimeError('missing')):
            with self.assertRaises(RuntimeError): roles.apply()
        self.assertEqual(before, self.file.read_bytes()); self.assertFalse(self.journal.exists())

    def test_first_install_restore_removes_new_empty_file(self):
        self.file.unlink(); roles.apply(); self.assertTrue(self.file.exists()); roles.restore(); self.assertFalse(self.file.exists())

    def test_core_to_personal_retains_original_for_restore(self):
        configured = roles.roles()
        def fixture(personal=False):
            browser = dict(configured['browser'], desktop='personal-browser.desktop' if personal else 'core-browser.desktop')
            return dict(configured, browser=browser)
        # Exercise a real role transition even when core and personal browsers
        # are intentionally configured to the same application on this host.
        with patch.object(roles, 'roles', side_effect=fixture):
            roles.apply(); self.assertEqual(roles.query('x-scheme-handler/http'), 'core-browser.desktop')
            roles.apply(True); self.assertEqual(roles.query('x-scheme-handler/http'), 'personal-browser.desktop')
            roles.restore(); self.assertEqual(self.file.read_text().strip(), self.original.strip())

    def test_interrupted_publication_can_resume(self):
        roles.apply(); saved = json.loads(self.journal.read_text()); entry = saved['entries']['inode/directory']
        entry['pending'] = 'org.gnome.Nautilus.desktop;'; entry['managed'] = entry['original']
        self.journal.write_text(json.dumps(saved)); roles.apply()
        self.assertNotIn('pending', json.loads(self.journal.read_text())['entries']['inode/directory'])

    def test_legacy_migration_adopts_only_actual_changed_keys(self):
        self.journal.parent.mkdir(parents=True)
        (self.journal.parent / 'mimeapps.original').write_text(self.original)
        managed = roles.replace(self.original, {'x-scheme-handler/http': 'brave-origin-nightly.desktop;'})
        (self.journal.parent / 'mimeapps.json').write_text(json.dumps({'path': str(self.file), 'existed': True, 'managed': managed}))
        self.file.write_text(managed.replace('keep.desktop;', 'later-user-choice.desktop;'))
        roles.migrate_legacy(self.file, self.journal)
        self.assertEqual(list(json.loads(self.journal.read_text())['entries']), ['x-scheme-handler/http'])
        roles.restore(); self.assertIn('later-user-choice.desktop;', self.file.read_text())
        self.assertNotIn('x-scheme-handler/http=', self.file.read_text())
