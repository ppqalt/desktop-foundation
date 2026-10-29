"""Real local Git remotes, isolated XDG state and inert build/deploy commands."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'
GIT = shutil.which('git')
OFFICIAL = 'https://github.com/ppqalt/desktop-foundation.git'


@unittest.skipIf(os.geteuid() == 0, 'Updater runs as the desktop user')
class Update(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'checkout with spaces'
        self.remote = self.base / 'origin.git'
        self.state = self.base / 'state/desktop-foundation'
        self.config = self.base / 'config'
        self.bin = self.base / 'bin'
        self.bin.mkdir(); self.state.mkdir(parents=True); self.root.mkdir()
        self.log = self.base / 'commands.jsonl'
        self.env = {**os.environ, 'HOME': str(self.base), 'XDG_STATE_HOME': str(self.state.parent),
                    'XDG_CONFIG_HOME': str(self.config), 'XDG_CACHE_HOME': str(self.base / 'cache'),
                    'XDG_DATA_HOME': str(self.base / 'data'), 'NIRI_SOCKET': 'unused-test-socket',
                    'GIT_CONFIG_GLOBAL': str(self.base / 'gitconfig'), 'GIT_CONFIG_NOSYSTEM': '1',
                    'GIT_TERMINAL_PROMPT': '0', 'DF_UPDATE_TEST_LOG': str(self.log),
                    'PATH': str(self.bin) + os.pathsep + os.environ['PATH']}
        self.git('init', '--bare', str(self.remote), cwd=self.base)
        self.git('init', '-b', 'main')
        self.git('config', 'user.name', 'Fixture'); self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', '--global', 'url.' + self.remote.as_uri() + '.insteadOf', OFFICIAL)
        (self.root / '.gitignore').write_text('/native/*/target/\n')
        (self.root / 'profiles/default').mkdir(parents=True)
        (self.root / 'profiles/default/niri.kdl').write_text('// fixture\n')
        for name in ['build-backend', 'build-nothing', 'preflight.py', 'deploy', 'niri', 'shell-reload', 'monitor']:
            self.script(name)
        (self.root / 'payload.txt').write_text('initial\n')
        self.commit('initial')
        self.git('remote', 'add', 'origin', OFFICIAL)
        self.git('push', 'origin', 'main')
        self.before = self.git('rev-parse', 'HEAD').strip()
        target = self.base / 'owned-source'; target.write_text('owned')
        link = self.config / 'niri/config.kdl'; link.parent.mkdir(parents=True); link.symlink_to(target)
        self.manifest = {'root': str(self.root), 'compositor': 'niri', 'profile': 'default',
                         'entries': [{'path': str(link), 'source': str(target), 'backup': None}]}
        (self.state / 'manifest.json').write_text(json.dumps(self.manifest))
        self.sentinels = [self.state / 'monitor/btop.conf', self.state / 'theme/current/wallpaper.toml',
                          self.config / 'mimeapps.list', self.config / 'desktop-foundation/local-ai.toml']
        for p in self.sentinels:
            p.parent.mkdir(parents=True, exist_ok=True); p.write_text('user settings\n')
        for relative in ['native/foundation/target/release/desktop-foundationctl',
                         'native/foundation/target/release/foundation-oo7-ready',
                         'native/foundation/target/release/foundation-browser-policy',
                         'native/nothing/target/release/foundation-nothing']:
            p = self.root / relative; p.parent.mkdir(parents=True, exist_ok=True); p.write_text('old backend'); p.chmod(0o755)
        for name in ['sudo', 'pacman', 'paru', 'yay']:
            p = self.bin / name
            p.write_text('#!/bin/sh\nprintf "PACKAGE COMMAND\\n" >> "$DF_UPDATE_TEST_LOG"\nexit 99\n'); p.chmod(0o755)
        niri = self.bin / 'niri'
        niri.write_text('#!/usr/bin/env python3\nimport json,os,sys\nwith open(os.environ["DF_UPDATE_TEST_LOG"], "a") as f: f.write(json.dumps({"name":"niri","args":sys.argv[1:]}) + "\\n")\n')
        niri.chmod(0o755)
        systemctl = self.bin / 'systemctl'
        systemctl.write_text('#!/bin/sh\n[ "${DF_TEST_ACTIVE:-0}" = 1 ]\n'); systemctl.chmod(0o755)

    def git(self, *args, cwd=None):
        return subprocess.check_output([GIT, '-C', str(cwd or self.root), *args], env=self.env, text=True, stderr=subprocess.DEVNULL)

    def commit(self, message):
        self.git('add', '.'); self.git('commit', '-m', message)

    def script(self, name):
        p = self.root / 'scripts' / name; p.parent.mkdir(exist_ok=True)
        p.write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
root = Path(__file__).resolve().parents[1]
name = Path(__file__).name
with open(os.environ['DF_UPDATE_TEST_LOG'], 'a') as f:
    f.write(json.dumps({'name':name, 'root':str(root), 'args':sys.argv[1:], 'payload':(root/'payload.txt').read_text(), 'private':(root/'personal.txt').read_text() if (root/'personal.txt').exists() else None}) + '\\n')
print('fixture build/deploy output: '+name)
if os.environ.get('DF_FAIL_STAGE') == name:
    print('fixture failure details',file=sys.stderr)
    sys.exit(7)
if name in ['build-backend', 'build-nothing']:
    crate = 'foundation' if name == 'build-backend' else 'nothing'
    names = ['desktop-foundationctl','foundation-oo7-ready','foundation-browser-policy'] if crate == 'foundation' else ['foundation-nothing']
    target = root / 'native' / crate / 'target/release'
    target.mkdir(parents=True, exist_ok=True)
    for binary in names:
        p = target / binary
        p.write_text('#!/bin/sh\\n# new backend\\nexit 0\\n')
        p.chmod(0o755)
''')
        p.chmod(0o755)
        # Real worktree checkout needs the crate directory, even with fake builds.
        for crate in ['foundation', 'nothing']:
            d = self.root / 'native' / crate; d.mkdir(parents=True, exist_ok=True)
            (d / 'Cargo.toml').write_text('# fixture\n')

    def remote_change(self, name='payload.txt'):
        clone = self.base / 'upstream'
        self.git('clone', '--branch', 'main', str(self.remote), str(clone), cwd=self.base)
        self.git('config', 'user.name', 'Fixture', cwd=clone); self.git('config', 'user.email', 'fixture@example.invalid', cwd=clone)
        (clone / name).write_text('updated\n')
        self.git('add', '.', cwd=clone); self.git('commit', '-m', 'remote change', cwd=clone)
        self.git('push', 'origin', 'main', cwd=clone)
        return self.git('rev-parse', 'HEAD', cwd=clone).strip()

    def invoke(self, *args, **env):
        return subprocess.run([str(BINARY), '--root', str(self.root), 'update', *args],
                              env={**self.env, **env}, text=True, capture_output=True, timeout=30)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def unchanged(self):
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), self.before)
        self.assertEqual((self.root / 'native/foundation/target/release/desktop-foundationctl').read_text(), 'old backend')
        self.assertNotIn('deploy', [call['name'] for call in self.calls()])

    def test_update_builds_before_publication_and_preserves_settings(self):
        revision = self.remote_change()
        untracked = self.root / 'personal.txt'; untracked.write_text('private')
        result = self.invoke(DF_TEST_ACTIVE='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), revision)
        calls = self.calls()
        self.assertEqual([c['name'] for c in calls], ['build-backend', 'build-nothing', 'preflight.py', 'deploy', 'niri', 'shell-reload', 'monitor'])
        self.assertTrue(all(c['root'] != str(self.root) for c in calls[:3]))
        self.assertEqual(calls[0]['private'], 'private')
        self.assertEqual(calls[3]['root'], str(self.root))
        self.assertEqual(calls[3]['args'], ['--update', '--compositor', 'niri', '--profile', 'default'])
        for path in self.sentinels: self.assertEqual(path.read_text(), 'user settings\n')
        self.assertEqual(untracked.read_text(), 'private')
        self.assertIn('new backend', (self.root / 'native/foundation/target/release/desktop-foundationctl').read_text())
        self.assertFalse((self.state / 'update.json').exists())
        self.assertEqual(len(result.stdout.splitlines()),4)
        self.assertNotIn('fixture build/deploy output',result.stdout+result.stderr)
        self.assertIn('fixture build/deploy output: build-backend',(self.state/'update.log').read_text())
        self.assertEqual((self.state/'update.log').stat().st_mode & 0o777,0o600)
        self.assertEqual(len(self.git('worktree', 'list', '--porcelain').split('worktree ')), 2)

    def test_local_edits_are_preserved_and_applied_to_build(self):
        revision = self.remote_change('upstream.txt')
        (self.root / 'payload.txt').write_text('my edits')
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), revision)
        self.assertEqual((self.root / 'payload.txt').read_text(), 'my edits')
        self.assertIn('payload.txt', self.git('diff', '--name-only'))
        reference = self.git('for-each-ref', '--format=%(refname)', 'refs/desktop-foundation').strip()
        self.assertEqual(self.git('show', reference + ':payload.txt'), 'my edits')
        build = next(call for call in self.calls() if call['name'] == 'build-backend')
        self.assertNotIn('Preserving local settings', result.stdout)
        self.assertIn('Preserving local settings', (self.state/'update.log').read_text())
        self.assertEqual(build['payload'], 'my edits')

    def test_diverging_commits_merge_without_rebase_or_reset(self):
        upstream = self.remote_change()
        (self.root / 'local.txt').write_text('mine'); self.commit('local commit')
        local = self.git('rev-parse', 'HEAD').strip()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        parents = self.git('show', '-s', '--format=%P', 'HEAD').split()
        self.assertIn(local, parents); self.assertIn(upstream, parents)
        self.assertEqual((self.root / 'local.txt').read_text(), 'mine')

    def test_true_local_conflict_stops_before_live_changes(self):
        self.remote_change()
        (self.root / 'payload.txt').write_text('my conflicting edits')
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.unchanged()
        self.assertEqual((self.root / 'payload.txt').read_text(), 'my conflicting edits')
        self.assertEqual(self.calls(), [])
        self.assertTrue(self.git('for-each-ref', '--format=%(refname)', 'refs/desktop-foundation').strip())

    def test_staged_and_unstaged_changes_keep_their_index_state(self):
        self.remote_change('upstream.txt')
        (self.root / 'payload.txt').write_text('staged content')
        self.git('add', 'payload.txt')
        (self.root / 'payload.txt').write_text('unstaged content after staging')
        new = self.root / 'added.txt'; new.write_text('new staged file'); self.git('add', 'added.txt')
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('show', ':payload.txt'), 'staged content')
        self.assertEqual((self.root / 'payload.txt').read_text(), 'unstaged content after staging')
        self.assertIn('added.txt', self.git('diff', '--cached', '--name-only'))
        self.assertEqual(self.git('show', ':added.txt'), 'new staged file')

    def test_dry_run_accepts_local_edits_without_creating_backups(self):
        (self.root / 'payload.txt').write_text('private edits')
        result = self.invoke('--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('for-each-ref', '--format=%(refname)', 'refs/desktop-foundation'), '')
        self.assertEqual(self.calls(), [])

    def test_failed_build_leaves_local_edits_and_user_stashes_in_place(self):
        (self.root / 'payload.txt').write_text('older stash'); self.git('stash', 'push', '-m', 'user stash')
        stash = self.git('rev-parse', 'refs/stash')
        (self.root / 'payload.txt').write_text('current local edits')
        result = self.invoke(DF_FAIL_STAGE='build-backend')
        self.assertNotEqual(result.returncode, 0); self.unchanged()
        self.assertEqual((self.root / 'payload.txt').read_text(), 'current local edits')
        self.assertEqual(self.git('rev-parse', 'refs/stash'), stash)
        successful = self.invoke()
        self.assertEqual(successful.returncode, 0, successful.stderr)
        self.assertEqual(self.git('rev-parse', 'refs/stash'), stash)
        self.assertEqual((self.root / 'payload.txt').read_text(), 'current local edits')

    def test_nonoverlapping_changes_in_same_file_merge(self):
        lines = ['line %02d\n' % n for n in range(30)]
        (self.root / 'payload.txt').write_text(''.join(lines)); self.commit('long config')
        self.git('push', 'origin', 'main')
        self.before = self.git('rev-parse', 'HEAD').strip()
        self.remote_change('other.txt')
        upstream = self.base / 'upstream'
        remote = lines.copy(); remote[-1] = 'upstream setting\n'
        (upstream / 'payload.txt').write_text(''.join(remote))
        self.git('add', '.', cwd=upstream); self.git('commit', '-m', 'upstream setting', cwd=upstream)
        self.git('push', 'origin', 'main', cwd=upstream)
        local = lines.copy(); local[0] = 'local setting\n'
        (self.root / 'payload.txt').write_text(''.join(local)); self.git('add', 'payload.txt')
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        expected = remote.copy(); expected[0] = 'local setting\n'
        self.assertEqual((self.root / 'payload.txt').read_text(), ''.join(expected))
        self.assertEqual(self.git('show', ':payload.txt'), ''.join(expected))

    def test_diverging_conflicting_commits_leave_live_checkout_unchanged(self):
        self.remote_change()
        (self.root / 'payload.txt').write_text('committed private setting'); self.commit('private setting')
        self.before = self.git('rev-parse', 'HEAD').strip()
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0); self.unchanged()
        self.assertEqual(self.git('status', '--porcelain'), '')
        self.assertEqual((self.root / 'payload.txt').read_text(), 'committed private setting')

    def test_untracked_parent_cannot_escape_isolated_checkout(self):
        self.remote_change('other.txt')
        upstream = self.base / 'upstream'
        outside = self.base / 'outside'; outside.mkdir()
        (upstream / 'extras').symlink_to(outside, target_is_directory=True)
        self.git('add', '.', cwd=upstream); self.git('commit', '-m', 'upstream symlink', cwd=upstream)
        self.git('push', 'origin', 'main', cwd=upstream)
        private = self.root / 'extras/private.txt'; private.parent.mkdir(); private.write_text('local setting')
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0); self.unchanged()
        self.assertEqual(private.read_text(), 'local setting')
        self.assertFalse((outside / 'private.txt').exists())

    def test_build_failure_does_not_publish(self):
        self.remote_change()
        result = self.invoke(DF_FAIL_STAGE='build-nothing')
        self.assertNotEqual(result.returncode, 0); self.assertIn('build/validation', result.stderr)
        self.assertIn(str(self.state/'update.log'),result.stderr)
        self.assertIn('fixture failure details',(self.state/'update.log').read_text())
        self.unchanged()
        self.assertEqual(len(self.git('worktree', 'list', '--porcelain').split('worktree ')), 2)

    def test_validation_failure_does_not_publish(self):
        self.remote_change()
        result = self.invoke(DF_FAIL_STAGE='preflight.py')
        self.assertNotEqual(result.returncode, 0); self.unchanged()

    def test_failed_fetch_does_not_publish(self):
        self.remote.rename(self.base / 'missing-remote')
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0); self.assertIn('fetch', result.stderr)
        self.unchanged(); self.assertEqual(self.calls(), [])

    def test_deployment_failure_retains_recovery_revision_and_reruns(self):
        revision = self.remote_change()
        result = self.invoke(DF_FAIL_STAGE='deploy')
        self.assertNotEqual(result.returncode, 0); self.assertIn('deployment', result.stderr)
        saved = json.loads((self.state / 'update.json').read_text())
        self.assertEqual(saved['previousRevision'], self.before); self.assertEqual(saved['revision'], revision)
        failed_again = self.invoke(DF_FAIL_STAGE='deploy')
        self.assertNotEqual(failed_again.returncode, 0)
        self.assertEqual(json.loads((self.state / 'update.json').read_text())['previousRevision'], self.before)
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.state / 'update.json').exists())

    def test_untracked_collision_is_not_overwritten(self):
        self.remote_change('personal.txt')
        path = self.root / 'personal.txt'; path.write_text('private')
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0); self.unchanged(); self.assertEqual(path.read_text(), 'private')

    def test_foreign_deployment_refused(self):
        link = self.config / 'niri/config.kdl'; link.unlink(); link.write_text('user replaced this')
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0); self.unchanged()
        self.assertEqual(link.read_text(), 'user replaced this')

    def test_dry_run_does_not_fetch_or_write(self):
        before = sorted(p.relative_to(self.base) for p in self.base.rglob('*'))
        result = self.invoke('--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [])
        self.assertEqual(sorted(p.relative_to(self.base) for p in self.base.rglob('*')), before)

    def test_local_commits_are_retained_when_upstream_is_ancestor(self):
        (self.root / 'local.txt').write_text('mine'); self.commit('local addition')
        revision = self.git('rev-parse', 'HEAD').strip()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), revision)

    def test_installed_symlink_resolves_the_owning_checkout(self):
        shutil.copy(ROOT / 'scripts/df-update', self.root / 'scripts/df-update')
        binary = self.root / 'native/foundation/target/release/desktop-foundationctl'
        binary.write_text('#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\n')
        entry = self.bin / 'df-update'; entry.symlink_to(self.root / 'scripts/df-update')
        result = subprocess.run(['df-update', '--dry-run'], env=self.env, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), ['--root', str(self.root), 'update', '--dry-run'])

    def test_legacy_manifest_reuses_theme_profile_and_active_compositor(self):
        self.manifest.pop('profile'); self.manifest.pop('compositor')
        (self.state / 'manifest.json').write_text(json.dumps(self.manifest))
        (self.state / 'theme/profile.json').write_text('{"profile":"default"}')
        result = self.invoke('--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('niri profile default', result.stdout)


if __name__ == '__main__': unittest.main()
