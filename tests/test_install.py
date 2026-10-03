"""Real rendering/deployment; native build prerequisites and service effects mocked."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('fresh_deploy', ROOT / 'scripts/deploy.py')
deploy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(deploy)


class FreshDeployment(unittest.TestCase):
    def test_install_twice_restore_and_dry_run(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            config, data, state = base / 'config', base / 'data', base / 'state/desktop-foundation'
            real_run = subprocess.run
            def isolated_run(command, **kwargs):
                if command[0] in {'systemctl', 'fc-cache'} or (len(command) > 1 and command[1] == str(ROOT / 'scripts/preferences.py')):
                    return subprocess.CompletedProcess(command, 0)
                return real_run(command, **kwargs)
            with patch.dict(os.environ, {'HOME': str(base), 'XDG_CONFIG_HOME': str(config), 'XDG_DATA_HOME': str(data), 'XDG_STATE_HOME': str(state.parent)}), patch.multiple(deploy, CONFIG=config, DATA=data, STATE=state), patch.object(deploy.subprocess, 'run', side_effect=isolated_run), patch.object(deploy, 'require_native_backends') as prerequisites, contextlib.redirect_stdout(io.StringIO()):
                with patch.object(sys, 'argv', ['deploy', 'install', '--dry-run']):
                    deploy.main()
                self.assertEqual(list(base.iterdir()), [])
                config.mkdir()
                kitty = config / 'kitty'
                kitty.mkdir()
                (kitty / 'old.conf').write_text('keep me')
                with patch.object(sys, 'argv', ['deploy', 'install']):
                    deploy.main()
                    first = (state / 'manifest.json').read_text()
                    deploy.main()
                    self.assertEqual(first, (state / 'manifest.json').read_text())
                self.assertTrue((data / 'wallpapers/wallhaven-135w7w.png').is_file())
                self.assertTrue((config / 'systemd/user/niri.service.wants/desktop-foundation-wallpaper.service').exists())
                with patch.object(sys, 'argv', ['deploy', 'restore']):
                    deploy.main()
                self.assertFalse(kitty.is_symlink())
                self.assertEqual((kitty / 'old.conf').read_text(), 'keep me')
                self.assertFalse((config / 'niri/config.kdl').exists())
                self.assertEqual(prerequisites.call_count, 2)

    def test_manifest_has_no_duplicates(self):
        for file in (ROOT / 'packages').glob('*.txt'):
            packages = [line for line in file.read_text().splitlines() if line and not line.startswith('#')]
            self.assertEqual(len(packages), len(set(packages)), str(file))


class ParuBootstrap(unittest.TestCase):
    def test_missing_helper_build_is_unprivileged(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            tools = base / 'bin'
            tools.mkdir()
            log = base / 'commands'
            for command in ['mktemp', 'cat', 'rm']:
                (tools / command).symlink_to('/usr/bin/' + command)
            (tools / 'sudo').write_text('#!/bin/bash\nprintf "sudo %s\\n" "$*" >> "$DF_TEST_LOG"\n')
            (tools / 'git').write_text('#!/bin/bash\nprintf "git %s\\n" "$*" >> "$DF_TEST_LOG"\n/bin/mkdir -p "${@: -1}"\necho "# reviewed fake recipe" > "${@: -1}/PKGBUILD"\n')
            (tools / 'makepkg').write_text('#!/bin/bash\n[[ $EUID != 0 ]] || exit 99\nprintf "makepkg %s\\n" "$*" >> "$DF_TEST_LOG"\n')
            for command in ['sudo', 'git', 'makepkg']:
                (tools / command).chmod(0o755)
            result = subprocess.run(['/bin/bash', str(ROOT / 'scripts/paru-bootstrap')], input='y\n', text=True,
                                    env={**os.environ, 'PATH': str(tools), 'DF_TEST_LOG': str(log)}, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            commands = log.read_text()
            self.assertIn('sudo pacman -Syu --needed base-devel git', commands)
            self.assertIn('makepkg -si', commands)
            self.assertNotIn('sudo makepkg', commands)

    def test_grub_arguments_are_idempotent(self):
        dropin = ROOT / 'session/apparmor/60-desktop-foundation.cfg'
        script = 'GRUB_CMDLINE_LINUX="rootflag=1"; GRUB_CMDLINE_LINUX_DEFAULT="quiet lsm=yama,apparmor,bpf"; . "$1"; first="$GRUB_CMDLINE_LINUX $GRUB_CMDLINE_LINUX_DEFAULT"; . "$1"; printf "%s\\n%s\\n" "$first" "$GRUB_CMDLINE_LINUX $GRUB_CMDLINE_LINUX_DEFAULT"'
        result = subprocess.check_output(['/bin/sh', '-c', script, 'sh', str(dropin)], text=True).splitlines()
        self.assertEqual(result[0].split(), result[1].split())
        self.assertEqual(result[1].count('apparmor=1'), 1)
        self.assertIn('lsm=yama,apparmor,bpf', result[1])
        common = subprocess.check_output(['/bin/sh', '-c', 'GRUB_CMDLINE_LINUX="rootflag=1"; GRUB_CMDLINE_LINUX_DEFAULT="quiet"; DF_APPARMOR_LSM="yama,apparmor,bpf"; . "$1"; printf "%s" "$GRUB_CMDLINE_LINUX"', 'sh', str(dropin)], text=True)
        self.assertIn('rootflag=1', common)
        self.assertIn('apparmor=1 lsm=yama,apparmor,bpf', common)
