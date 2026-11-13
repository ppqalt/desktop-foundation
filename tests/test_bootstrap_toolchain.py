"""Execute actual bootstrap with inert package, privilege and rustup tools."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class BootstrapToolchain(unittest.TestCase):
    def run_bootstrap(self, missing='', dev=False, install_fails=False):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            scripts = base / 'repo/scripts'
            scripts.mkdir(parents=True)
            shutil.copy2(ROOT / 'scripts/bootstrap', scripts / 'bootstrap')
            shutil.copytree(ROOT / 'packages', base / 'repo/packages')
            tools = base / 'bin'
            tools.mkdir()
            log = base / 'commands'
            for name, content in {
                'pacman': 'exit 0',
                'sudo': 'printf "sudo %s\\n" "$*" >> "$LOG"',
                'rustup': '''printf 'rustup %s\\n' "$*" >> "$LOG"
if [ "$1" = run ] && [ "$4" = --version ] && [ "$3" = "$MISSING" ]; then exit 1; fi
if [ "$1 $2" = 'toolchain install' ] && [ "$FAIL_INSTALL" = 1 ]; then exit 1; fi
''',
            }.items():
                path = tools / name
                path.write_text('#!/bin/sh\n' + content + '\n')
                path.chmod(0o755)
            env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                       LOG=str(log), MISSING=missing, FAIL_INSTALL=str(int(install_fails)))
            # Private copy redirects only Qt executable presence probes. Actual
            # option, package and toolchain logic remains the production script.
            if dev:
                text = (scripts / 'bootstrap').read_text().replace('test -x "/usr/lib/qt6/bin/$tool"', ': "$tool"')
                (scripts / 'bootstrap').write_text(text)
            result = subprocess.run(['bash', str(scripts / 'bootstrap')] + (['--dev'] if dev else []),
                                    env=env, text=True, capture_output=True, timeout=10)
            return result, log.read_text().splitlines()

    def test_complete_pinned_toolchain_is_reused(self):
        result, commands = self.run_bootstrap()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('rustup run 1.98.1 rustc --version', commands)
        self.assertIn('rustup run 1.98.1 cargo --version', commands)
        self.assertFalse(any('toolchain install' in c for c in commands))

    def test_missing_or_partial_toolchain_is_installed_and_failure_stops(self):
        for missing in ['rustc', 'cargo']:
            with self.subTest(missing=missing):
                result, commands = self.run_bootstrap(missing=missing)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn('rustup toolchain install 1.98.1 --profile minimal', commands)
        result, commands = self.run_bootstrap(missing='cargo', install_fails=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any('enable --now' in c for c in commands))

    def test_dev_tools_use_pin_without_changing_global_default(self):
        result, commands = self.run_bootstrap(dev=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('rustup component add --toolchain 1.98.1 rustfmt clippy rust-analyzer', commands)
        self.assertIn('rustup run 1.98.1 cargo clippy --version', commands)
        self.assertFalse(any(c.startswith('rustup default ') for c in commands))
        for tool in ['rustc', 'cargo', 'rustfmt', 'rust-analyzer']:
            self.assertIn(f'rustup run 1.98.1 {tool} --version', commands)
