import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import install_engine as installer


class InstallerLayers(unittest.TestCase):
    def test_default_full_and_core_are_two_layers(self):
        self.assertTrue(installer.parse([]).personal)
        self.assertFalse(installer.parse(['--core']).personal)
        self.assertTrue(installer.parse(['--personal']).personal)
        self.assertFalse(installer.parse(['--core', '--no-greeter']).greeter)

    def test_clean_home_dry_runs_are_read_only(self):
        for entry in ['install-core', 'install-all']:
            with tempfile.TemporaryDirectory() as directory:
                home = Path(directory)
                env = {**os.environ, 'HOME': str(home), 'XDG_CONFIG_HOME': str(home / 'config'), 'XDG_STATE_HOME': str(home / 'state'), 'XDG_DATA_HOME': str(home / 'data')}
                result = subprocess.run([str(ROOT / 'scripts' / entry), '--dry-run'], env=env, text=True, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(list(home.iterdir()), [])
                self.assertIn('Roles:', result.stdout)
                self.assertNotIn('brave-origin-nightly-bin', result.stdout) if entry == 'install-core' else self.assertIn('brave-origin-nightly-bin', result.stdout)

    def test_core_does_not_provision_personal_apps(self):
        args = installer.parse(['--core', '--no-packages', '--no-greeter'])
        with patch.object(installer, 'script') as script, patch.object(installer, 'command') as command:
            installer.core(args)
        called = [c.args[0] for c in script.call_args_list]
        self.assertNotIn('spotify-setup', called); self.assertNotIn('paru-bootstrap', called)
        self.assertNotIn('personal', str(command.call_args_list))

    def test_installed_personal_package_is_adopted_without_mutation(self):
        app = {'package': 'package', 'desktop': 'app.desktop', 'executable': 'app'}
        with patch.object(installer, 'installed', return_value=True), patch.object(installer, 'validate_role'), patch.object(installer, 'command') as command:
            installer.provision_package(app)
        command.assert_not_called()

    def test_foreign_variant_is_not_replaced(self):
        app = {'package': 'package', 'desktop': 'app.desktop', 'executable': 'app'}
        with patch.object(installer, 'installed', return_value=False), patch.object(installer.shutil, 'which', return_value='/unrelated/app'), patch.object(installer, 'command') as command:
            with self.assertRaises(RuntimeError): installer.provision_package(app)
        command.assert_not_called()

    def test_package_download_uses_full_upgrade_and_needed(self):
        app = {'package': 'package', 'desktop': 'app.desktop', 'executable': 'app'}
        with patch.object(installer, 'installed', return_value=False), patch.object(installer.shutil, 'which', return_value=None), patch.object(installer, 'desktop_path', return_value=None), patch.object(installer.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)), patch.object(installer, 'validate_role'), patch.object(installer, 'command') as command:
            installer.provision_package(app)
        command.assert_called_once_with('sudo', 'pacman', '-Syu', '--needed', 'package')

    def test_check_does_not_apply_or_deploy(self):
        with patch.object(installer, 'script') as script, patch.object(installer, 'core') as core, patch.object(installer, 'personal') as personal:
            installer.main(['--check'])
        core.assert_not_called(); personal.assert_not_called()
        self.assertEqual(script.call_args.args[0], 'doctor')
