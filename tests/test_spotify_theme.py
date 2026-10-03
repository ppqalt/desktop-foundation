import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import spotify_theme as theme

class SpotifyTheme(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        base=Path(self.temp.name)
        env=patch.dict(os.environ,{'XDG_CONFIG_HOME':str(base/'config'),'XDG_STATE_HOME':str(base/'state')});env.start();self.addCleanup(env.stop)
        self.config,self.journal,self.generated=theme.paths();self.file=self.config/'config-xpui.ini';self.color=self.config/'Themes/marketplace/color.ini'
        self.color.parent.mkdir(parents=True);self.color.write_text('[Marketplace]\n\n[UserCustom]\nmain = abcdef\n')
        self.file.write_text('[Setting]\ncurrent_theme = marketplace\ncolor_scheme = \nreplace_colors = 1\nspotify_path = /missing\n\n[AdditionalOptions]\ncustom_apps = marketplace|other\nextensions = untouched.js\n')
        self.generated.parent.mkdir(parents=True)
        p=json.loads((theme.ROOT/'theme/generated.json').read_text());self.generated.write_text(theme.colors(p))
        self.css=self.color.parent/'user.css';self.css.write_text('/* user customization */')

    def test_idempotent_install_preserves_marketplace_and_customizations(self):
        theme.install();first=self.file.read_bytes();theme.install()
        self.assertEqual(first,self.file.read_bytes());self.assertEqual(self.css.read_text(),'/* user customization */')
        self.assertEqual(theme.parse(self.color.read_text())['UserCustom']['main'],'abcdef')
        config=theme.parse(self.file.read_text());self.assertEqual(config['Setting']['current_theme'],'marketplace');self.assertEqual(config['AdditionalOptions']['extensions'],'untouched.js')

    def test_restore_changes_only_owned_fields_and_section(self):
        theme.install();self.file.write_text(self.file.read_text().replace('untouched.js','new-user-extension.js'))
        with patch.object(theme.shutil,'which',return_value=None):theme.restore()
        config=theme.parse(self.file.read_text());self.assertEqual(config['Setting']['color_scheme'],'');self.assertEqual(config['AdditionalOptions']['extensions'],'new-user-extension.js');self.assertFalse(theme.parse(self.color.read_text()).has_section(theme.SCHEME));self.assertFalse(self.journal.exists())

    def test_external_owned_setting_change_is_refused(self):
        theme.install();self.file.write_text(self.file.read_text().replace('color_scheme = DesktopFoundation','color_scheme = UserCustom'))
        with self.assertRaises(RuntimeError):theme.refresh()

    def test_optional_missing_client_does_not_run_spicetify(self):
        theme.install()
        with patch.object(theme.subprocess,'run') as run:theme.refresh()
        run.assert_not_called()

    def test_refresh_uses_no_restart_and_not_extensions_or_apps(self):
        theme.install();app=self.config/'client';(app/'Apps/xpui').mkdir(parents=True);(app/'Apps/xpui/spicetify-config.json').write_text('{}')
        self.file.write_text(theme.setting(self.file.read_text(),'spotify_path',str(app)))
        with patch.object(theme.shutil,'which',return_value='/tool/spicetify'),patch.object(theme.subprocess,'run') as run:theme.refresh()
        self.assertEqual(run.call_args.args[0],['/tool/spicetify','refresh','--no-restart'])

    def test_non_marketplace_theme_is_preserved(self):
        self.file.write_text(self.file.read_text().replace('current_theme = marketplace','current_theme = Other'))
        before=self.color.read_bytes()
        with self.assertRaises(RuntimeError):theme.install()
        self.assertEqual(self.color.read_bytes(),before);self.assertFalse(self.journal.exists())

    def test_writeahead_install_resumes_original_settings(self):
        saved={'original':{'color_scheme':'','replace_colors':'1'},'owned':{'color_scheme':theme.SCHEME,'replace_colors':'1'},'pending':True}
        self.journal.parent.mkdir(parents=True);self.journal.write_text(json.dumps(saved));theme.install();self.assertNotIn('pending',json.loads(self.journal.read_text()))

class SpotifyLauncher(unittest.TestCase):
    def run_launcher(self, *, patched=False, prefs=True, status=0):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);scripts=root/'scripts';scripts.mkdir()
            launcher=scripts/'spotify';launcher.write_text((theme.ROOT/'scripts/spotify').read_text());launcher.chmod(0o755)
            setup=scripts/'spotify-setup';setup.write_text('#!/bin/bash\nprintf "%s" "$1" > "$XDG_DATA_HOME/applied"\n');setup.chmod(0o755)
            client=root/'data/desktop-foundation/apps/spotify/usr/share/spotify/spotify';client.parent.mkdir(parents=True)
            client.write_text(f'#!/bin/bash\nexit {status}\n');client.chmod(0o755)
            if patched:
                marker=client.parent/'Apps/xpui/spicetify-config.json';marker.parent.mkdir(parents=True);marker.write_text('{}')
            if prefs:
                preference=root/'config/spotify/prefs';preference.parent.mkdir(parents=True);preference.write_text('not read')
            result=subprocess.run([str(launcher)],env={**os.environ,'XDG_DATA_HOME':str(root/'data'),'XDG_CONFIG_HOME':str(root/'config')},capture_output=True)
            applied=root/'data/applied'
            return result.returncode,applied.read_text() if applied.exists() else None

    def test_first_normal_quit_completes_setup(self):
        self.assertEqual(self.run_launcher(),(0,'apply'))

    def test_patched_launch_does_not_reapply(self):
        self.assertEqual(self.run_launcher(patched=True),(0,None))

    def test_uninitialized_launch_waits_for_login(self):
        self.assertEqual(self.run_launcher(prefs=False),(0,None))

    def test_failed_client_does_not_patch(self):
        self.assertEqual(self.run_launcher(status=7),(7,None))
