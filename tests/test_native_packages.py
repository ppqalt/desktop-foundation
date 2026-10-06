"""Package cache behavior with private databases and fake commands."""
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest
import sys
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from theme.adapters import render
from theme_pipeline import derive

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'


class NativeForeignCache(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.database = self.base / 'database'
        self.bin = self.base / 'bin'
        self.bin.mkdir(); (self.database / 'local').mkdir(parents=True); (self.database / 'sync').mkdir()
        self.repo = self.database / 'sync/core.db'; self.repo.write_text('initial')
        self.query_log = self.base / 'queries'
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ['PATH'],
                        XDG_CACHE_HOME=str(self.base / 'cache'), DF_DATABASE=str(self.database),
                        DF_PACKAGE_LOG=str(self.query_log), DF_REPOSITORIES='core\n')
        self.cache = self.base / 'cache/desktop-foundation/fastfetch-foreign.json'
        self.fake('pacman-conf', 'print(os.environ["DF_DATABASE"] if sys.argv[1] == "DBPath" else os.environ["DF_REPOSITORIES"], end="\\n" if sys.argv[1] == "DBPath" else "")')
        self.fake('fastfetch', 'print("1234 (pacman)")')
        self.fake('pacman', 'with open(os.environ["DF_PACKAGE_LOG"], "a") as f: f.write("query\\n")\nprint("foreign")')

    def fake(self, name, body):
        file = self.bin / name
        file.write_text('#!/usr/bin/env python3\nimport os,sys\n' + body + '\n'); file.chmod(0o755)

    def invoke(self, check=True):
        return subprocess.run([str(BINARY), '--root', str(ROOT), 'system', 'packages'], env=self.env,
                              capture_output=True, text=True, check=check, timeout=10)

    def queries(self):
        return len(self.query_log.read_text().splitlines()) if self.query_log.exists() else 0

    def test_reuse_and_local_repository_or_repo_list_invalidation(self):
        self.assertEqual(self.invoke().stdout, '1234 (pacman), 1 (AUR)\n')
        self.invoke(); self.assertEqual(self.queries(), 1)
        (self.database / 'local/new-package').mkdir(); self.invoke(); self.assertEqual(self.queries(), 2)
        self.repo.write_text('repository update'); self.invoke(); self.assertEqual(self.queries(), 3)
        self.env['DF_REPOSITORIES'] += 'extra\n'; self.invoke(); self.assertEqual(self.queries(), 4)

    def test_zero_transaction_and_native_error_are_distinguished(self):
        self.fake('pacman', 'with open(os.environ["DF_PACKAGE_LOG"], "a") as f: f.write("query\\n")\nsys.exit(1)')
        self.assertIn('0 (AUR)', self.invoke().stdout); self.invoke(); self.assertEqual(self.queries(), 1)
        (self.database / 'db.lck').touch(); self.invoke(); self.invoke(); self.assertEqual(self.queries(), 3)
        self.fake('pacman', 'print("database error", file=sys.stderr); sys.exit(1)')
        self.assertIn('database error', self.invoke(check=False).stderr)

    def test_existing_python_cache_is_accepted_and_rust_keeps_the_schema(self):
        paths = [self.database / 'local', self.repo]
        stamps = [[str(p), p.stat().st_ino, p.stat().st_mtime_ns, p.stat().st_ctime_ns, p.stat().st_size] for p in paths]
        self.cache.parent.mkdir(parents=True)
        self.cache.write_text(json.dumps({'signature': stamps, 'count': 7}))
        self.assertIn('7 (AUR)', self.invoke().stdout); self.assertEqual(self.queries(), 0)
        self.repo.write_text('changed'); self.invoke()
        saved = json.loads(self.cache.read_text())
        self.assertEqual(saved['count'], 1)
        self.assertEqual(saved['signature'][0], stamps[0])
        self.assertEqual(self.cache.stat().st_mode & 0o777, 0o600)

    def test_invalid_cache_count_types_are_recomputed_without_hiding_query_errors(self):
        self.invoke(); saved = json.loads(self.cache.read_text())
        for value in [True, -1, '5']:
            saved['count'] = value; self.cache.write_text(json.dumps(saved)); self.invoke()
        self.assertEqual(self.queries(), 4)

    def test_relocated_binary_and_deployed_directory_symlink_keep_package_config_routing(self):
        root = self.base / 'relocated'
        target = root / 'native/foundation/target/release/desktop-foundationctl'
        target.parent.mkdir(parents=True); shutil.copy2(BINARY, target)
        (root / 'config').mkdir(); (root / 'config/application-roles.json').write_text('{}')
        terminal = root / 'terminal/fastfetch'; terminal.mkdir(parents=True)
        helper = terminal / 'packages'; helper.write_bytes((ROOT / 'terminal/fastfetch/packages').read_bytes()); helper.chmod(0o755)
        (terminal / 'packages.jsonc').write_bytes((ROOT / 'terminal/fastfetch/packages.jsonc').read_bytes())
        installed = self.base / 'user-config'; installed.mkdir(); (installed / 'fastfetch').symlink_to(terminal)
        self.fake('fastfetch', 'assert sys.argv[2] == ' + repr(str(installed / 'fastfetch/packages.jsonc')) + '\nprint("1234 (pacman)")')
        result = subprocess.run([str(installed / 'fastfetch/packages')], env=self.env, check=True, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.stdout, '1234 (pacman), 1 (AUR)\n')

    def test_relative_source_helper_invocation_preserves_package_config(self):
        root = self.base / 'relative source checkout'
        target = root / 'native/foundation/target/release/desktop-foundationctl'
        target.parent.mkdir(parents=True); shutil.copy2(BINARY, target)
        (root / 'config').mkdir(); (root / 'config/application-roles.json').write_text('{}')
        shutil.copytree(ROOT / 'terminal/fastfetch', root / 'terminal/fastfetch')
        config = root / 'terminal/fastfetch/packages.jsonc'
        self.fake('fastfetch', 'from pathlib import Path\nassert Path(sys.argv[2]).is_absolute()\n'
                  + 'assert Path(sys.argv[2]).resolve() == Path(' + repr(str(config)) + ').resolve()\n'
                  + 'print("1234 (pacman)")')
        for helper in ['./terminal/fastfetch/packages', 'terminal/fastfetch/packages']:
            with self.subTest(helper=helper):
                result = subprocess.run([helper], cwd=root, env=self.env, check=True,
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.stdout, '1234 (pacman), 1 (AUR)\n')

    def test_copied_theme_bundle_resolves_owner_and_keeps_revision_package_config(self):
        # Actual deployment topology: config -> current -> copied revision files.
        root = self.base / "owner checkout with ' quote"
        shutil.copytree(ROOT / 'terminal', root / 'terminal')
        shutil.copytree(ROOT / 'theme/fallback', root / 'theme/fallback')
        target = root / 'native/foundation/target/release/desktop-foundationctl'
        target.parent.mkdir(parents=True); shutil.copy2(BINARY, target)
        (root / 'config').mkdir(); (root / 'config/application-roles.json').write_text('{}')
        revision = self.base / 'state/theme/revisions/example'
        revision.mkdir(parents=True)
        palette = derive({'primary': '#ffb599', 'secondary': '#dfbfaf'}, 'test', 'hash')
        with patch('render_niri.render', return_value='// test'):
            render(root, revision, palette, 'dual-display-example')
        current = revision.parents[1] / 'current'; current.symlink_to(revision)
        installed = self.base / 'config'; installed.mkdir()
        (installed / 'fastfetch').symlink_to(current / 'terminal/fastfetch')
        config = installed / 'fastfetch/packages.jsonc'
        self.fake('fastfetch', 'assert sys.argv[2] == ' + repr(str(config)) + '\nprint("1234 (pacman)")')
        result = subprocess.run([str(installed / 'fastfetch/packages')], env=self.env, check=True,
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.stdout, '1234 (pacman), 1 (AUR)\n')
