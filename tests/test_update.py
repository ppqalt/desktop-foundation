"""Real local Git remotes, isolated XDG state and inert build/deploy commands."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('DF_UPDATE_BINARY', str(ROOT / 'native/foundation/target/debug/desktop-foundationctl')))
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
        for name in ['build-backend', 'build-nothing', 'preflight', 'deploy', 'niri', 'shell-reload', 'monitor']:
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
if name == 'build-backend' and os.environ.get('DF_TEST_SWITCH_BRANCH'):
    import subprocess
    subprocess.run(['git', '-C', os.environ['DF_TEST_LIVE_ROOT'], 'switch', os.environ['DF_TEST_SWITCH_BRANCH']], check=True, capture_output=True)
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

    def test_update_help_describes_local_rebuild(self):
        result = self.invoke('--help')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('--local', result.stdout)
        self.assertIn('Usage: df-update', result.stdout)
        self.assertEqual(self.calls(), [])

    def test_local_rebuild_ignores_unavailable_remote_and_preserves_all_edits(self):
        self.remote_change()
        self.git('remote', 'set-url', 'origin', '/unavailable/local-only')
        (self.root / 'payload.txt').write_text('staged edits')
        self.git('add', 'payload.txt')
        (self.root / 'payload.txt').write_text('unstaged edits')
        (self.root / 'personal.txt').write_text('private')
        before_status = self.git('status', '--porcelain')
        before_index = self.git('diff', '--cached')
        result = self.invoke('--local')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), self.before)
        self.assertEqual(self.git('status', '--porcelain'), before_status)
        self.assertEqual(self.git('diff', '--cached'), before_index)
        calls = self.calls()
        self.assertEqual([c['name'] for c in calls[:4]], ['build-backend', 'build-nothing', 'preflight', 'deploy'])
        self.assertEqual(calls[0]['payload'], 'unstaged edits')
        self.assertEqual(calls[0]['private'], 'private')
        for path in self.sentinels: self.assertEqual(path.read_text(), 'user settings\n')
        self.assertIn('no upstream fetch', (self.state / 'update.log').read_text())

    def test_local_dry_run_is_read_only_without_a_remote(self):
        self.git('remote', 'remove', 'origin')
        result = self.invoke('--local', '--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Source: local checkout', result.stdout)
        self.assertEqual(self.calls(), [])
        self.assertFalse((self.state / 'update.log').exists())
        self.unchanged()

    def test_local_rebuild_failure_never_deploys(self):
        result = self.invoke('--local', DF_FAIL_STAGE='build-nothing')
        self.assertNotEqual(result.returncode, 0)
        self.unchanged()

    def test_local_rejects_remote_options(self):
        for args in [('--check',), ('--branch', 'main'), ('--version', 'v1'), ('--select',)]:
            result = self.invoke('--local', *args)
            self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [])

    def test_update_builds_before_publication_and_preserves_settings(self):
        revision = self.remote_change()
        untracked = self.root / 'personal.txt'; untracked.write_text('private')
        result = self.invoke(DF_TEST_ACTIVE='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), revision)
        calls = self.calls()
        self.assertEqual([c['name'] for c in calls], ['build-backend', 'build-nothing', 'preflight', 'deploy', 'niri', 'shell-reload', 'monitor'])
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
        reference = self.git('for-each-ref', '--format=%(refname)', 'refs/desktop-foundation/local-update-*').strip()
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
        result = self.invoke(DF_FAIL_STAGE='preflight')
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

    def test_missing_updater_bootstraps_then_builds_fetched_source(self):
        self.remote_change()
        shutil.copy(ROOT / 'scripts/df-update', self.root / 'scripts/df-update')
        build = self.root / 'scripts/build-backend'
        source = build.read_text()
        source = source.replace("if name in ['build-backend', 'build-nothing']:", """if name == 'build-backend' and os.environ.get('CARGO_TARGET_DIR'):
    import shutil
    target = Path(os.environ['CARGO_TARGET_DIR']) / 'release/desktop-foundationctl'
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(os.environ['DF_TEST_BOOTSTRAP_NATIVE'], target)
    target.chmod(0o700)
    sys.exit(0)
if name in ['build-backend', 'build-nothing']:""")
        build.write_text(source)
        binary = self.root / 'native/foundation/target/release/desktop-foundationctl'
        binary.unlink()  # Isolated fixture only; never remove the live backend.
        result = subprocess.run([str(self.root / 'scripts/df-update')],
                                env={**self.env, 'DF_TEST_BOOTSTRAP_NATIVE':str(BINARY)},
                                text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(calls[0]['root'], str(self.root))
        staged_build = [c for c in calls if c['name']=='build-backend' and c['root']!=str(self.root)]
        self.assertEqual(len(staged_build), 1)
        self.assertEqual(staged_build[0]['payload'], 'updated\n')
        self.assertEqual((self.root/'payload.txt').read_text(), 'updated\n')
        self.assertIn('new backend', binary.read_text())
        for path in self.sentinels: self.assertEqual(path.read_text(), 'user settings\n')

    def publish_branch(self, name='release/candidate'):
        self.git('branch', name)
        self.git('push', 'origin', name)
        upstream = self.base / 'branch-source'
        self.git('clone', '--branch', name, str(self.remote), str(upstream), cwd=self.base)
        self.git('config', 'user.name', 'Fixture', cwd=upstream)
        self.git('config', 'user.email', 'fixture@example.invalid', cwd=upstream)
        (upstream/'branch-only.txt').write_text('selected source')
        self.git('add', '.', cwd=upstream); self.git('commit', '-m', 'selected branch update', cwd=upstream)
        self.git('push', 'origin', name, cwd=upstream)
        return self.git('rev-parse','HEAD',cwd=upstream).strip()

    def test_selected_branch_fetches_builds_and_keeps_installed_branch(self):
        target = self.publish_branch()
        result = self.invoke('--branch','release/candidate')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(self.git('rev-parse','HEAD').strip(),target)
        self.assertEqual(self.git('branch','--show-current').strip(),'main')
        self.assertEqual((self.root/'branch-only.txt').read_text(),'selected source')
        self.assertEqual([c['name'] for c in self.calls()][:3],['build-backend','build-nothing','preflight'])

    def test_selected_branch_check_does_not_fetch(self):
        target = self.publish_branch()
        result = self.invoke('--check','--branch=release/candidate')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn(target[:12],result.stdout)
        self.assertIn('refs/heads/release/candidate',result.stdout)
        self.assertFalse((self.state/'update.log').exists())
        self.assertEqual(self.git('rev-parse','HEAD').strip(),self.before)

    def test_selected_branch_dry_run_does_not_require_network(self):
        result = self.invoke('--dry-run','--branch','not-published-yet')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn('GitHub branch: not-published-yet',result.stdout)
        self.assertFalse(self.calls())

    def test_missing_selected_branch_stops_before_build(self):
        result = self.invoke('--branch','missing-branch')
        self.assertNotEqual(result.returncode,0)
        self.unchanged()
        self.assertFalse(self.calls())

    def test_invalid_or_ambiguous_branch_selection_is_nonmutating(self):
        for args in [('--branch','../bad'),('--branch','a:b'),('--branch','--upload-pack=evil'),
                     ('--branch',''),('--select','--branch','main'),('--branch',)]:
            result=self.invoke(*args)
            self.assertNotEqual(result.returncode,0,args)
            self.assertFalse((self.state/'update.log').exists())
        self.unchanged()

    def test_release_and_branch_combination_is_validated(self):
        target = self.publish_branch()
        self.git('tag', 'v2', target, cwd=self.base/'branch-source')
        self.git('push','origin','refs/tags/v2',cwd=self.base/'branch-source')
        result = self.invoke('--branch','release/candidate','--version','v2')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(self.git('rev-parse','HEAD').strip(),target)

    def test_release_from_other_branch_is_not_deployed(self):
        target = self.publish_branch()
        self.git('tag','v2',target,cwd=self.base/'branch-source'); self.git('push','origin','refs/tags/v2',cwd=self.base/'branch-source')
        result=self.invoke('--branch','main','--version','v2')
        self.assertNotEqual(result.returncode,0)
        self.assertIn('does not belong',result.stderr)
        self.unchanged()

    def test_terminal_picker_lists_branch_releases_and_cancels_safely(self):
        import pty, select, time
        self.git('tag','-a','v1','-m','release'); self.git('push','origin','refs/tags/v1')
        other = self.publish_branch()
        self.git('tag','v9',other,cwd=self.base/'branch-source')
        self.git('push','origin','refs/tags/v9',cwd=self.base/'branch-source')
        primary, secondary = pty.openpty()
        try:
            child=subprocess.Popen([str(BINARY),'--root',str(self.root),'update','--select'],
                                   env=self.env,stdin=secondary,stdout=secondary,stderr=secondary)
            os.close(secondary); secondary=-1
            os.write(primary,b'1\n1\nn\n')
            output=b''; deadline=time.monotonic()+20
            while time.monotonic()<deadline:
                if select.select([primary],[],[],.1)[0]:
                    try: data=os.read(primary,65536)
                    except OSError: break
                    if not data: break
                    output+=data
                if child.poll() is not None: break
            if child.poll() is None:
                child.kill(); child.wait(); self.fail('terminal picker timed out: '+output.decode(errors='replace'))
            self.assertEqual(child.wait(),0,output.decode(errors='replace'))
            text=output.decode(errors='replace')
            self.assertIn('1 / 2  Branch',text); self.assertIn('2 / 2  Version',text)
            self.assertIn('Release tag  v1',text); self.assertNotIn('Release tag  v9',text); self.assertIn('Update cancelled',text)
            self.unchanged(); self.assertFalse((self.state/'update.log').exists())
            self.assertFalse(list((self.base/'cache/desktop-foundation/update').glob('picker-*')))
        finally:
            os.close(primary)
            if secondary>=0: os.close(secondary)

    def test_terminal_zero_rebuilds_local_files_without_version_picker(self):
        import pty, select, time
        shutil.rmtree(self.remote)
        primary, secondary = pty.openpty()
        try:
            child=subprocess.Popen([str(BINARY),'--root',str(self.root),'update','--select'],
                                   env=self.env,stdin=secondary,stdout=secondary,stderr=secondary)
            os.close(secondary); secondary=-1
            os.write(primary,b'0\n')
            output=b''; deadline=time.monotonic()+20
            while time.monotonic()<deadline:
                if select.select([primary],[],[],.1)[0]:
                    try: data=os.read(primary,65536)
                    except OSError: break
                    if not data: break
                    output+=data
                if child.poll() is not None: break
            if child.poll() is None:
                child.kill(); child.wait(); self.fail('terminal picker timed out: '+output.decode(errors='replace'))
            self.assertEqual(child.wait(),0,output.decode(errors='replace'))
            text=output.decode(errors='replace')
            self.assertIn('0  Rebuild from local files', text)
            self.assertNotIn('2 / 2  Version', text)
            self.assertEqual(self.git('rev-parse', 'HEAD').strip(), self.before)
            self.assertIn('deploy', [call['name'] for call in self.calls()])
        finally:
            os.close(primary)
            if secondary>=0: os.close(secondary)

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

    def test_named_installation_branch_fetches_and_preserves_local_work(self):
        self.git('switch', '-c', 'lucky38/local')
        (self.root / 'local.txt').write_text('local desktop'); self.commit('local desktop')
        local = self.git('rev-parse', 'HEAD').strip()
        other = self.git('rev-parse', 'main').strip()
        upstream = self.remote_change()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('symbolic-ref', '--short', 'HEAD').strip(), 'lucky38/local')
        self.assertEqual(self.git('rev-parse', 'main').strip(), other)
        parents = self.git('show', '-s', '--format=%P', 'HEAD').split()
        self.assertIn(local, parents); self.assertIn(upstream, parents)
        self.assertEqual((self.root / 'local.txt').read_text(), 'local desktop')

    def test_missing_origin_fetches_official_github_without_rewriting_remotes(self):
        revision = self.remote_change()
        self.git('remote', 'remove', 'origin')
        plan = self.invoke('--dry-run')
        self.assertEqual(plan.returncode, 0, plan.stderr)
        self.assertIn('direct GitHub fallback', plan.stdout)
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git('rev-parse', 'HEAD').strip(), revision)
        self.assertEqual(self.git('remote'), '')

    def test_live_check_queries_github_without_fetching_or_deploying(self):
        revision = self.remote_change()
        before = sorted(p.relative_to(self.base) for p in self.base.rglob('*'))
        result = self.invoke('--check')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(revision[:12], result.stdout)
        self.unchanged()
        self.assertEqual(self.calls(), [])
        self.assertEqual(sorted(p.relative_to(self.base) for p in self.base.rglob('*')), before)
        self.assertFalse((self.state / 'update.log').exists())

    def test_check_reports_local_commits_already_containing_github_main(self):
        (self.root / 'local.txt').write_text('mine'); self.commit('local')
        result = self.invoke('--check')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('1 local commit(s)', result.stdout)
        self.assertIn('already included', result.stdout)

    def test_dry_run_on_custom_branch_keeps_branch_and_files(self):
        self.git('switch', '-c', 'lucky38/desktop')
        (self.root / 'payload.txt').write_text('private edits')
        before = self.git('status', '--porcelain')
        result = self.invoke('--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('lucky38/desktop', result.stdout)
        self.assertEqual(self.git('status', '--porcelain'), before)
        self.assertEqual(self.calls(), [])

    def test_branch_switch_during_build_prevents_publication_even_at_same_revision(self):
        self.git('branch', 'other-work')
        result = self.invoke(DF_TEST_SWITCH_BRANCH='other-work', DF_TEST_LIVE_ROOT=str(self.root))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('changed during the build', result.stderr)
        self.assertEqual(self.git('symbolic-ref', '--short', 'HEAD').strip(), 'other-work')
        self.unchanged()

    def test_recovery_ref_retains_previous_committed_source(self):
        self.remote_change()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        reference = self.git('for-each-ref', '--format=%(refname)', 'refs/desktop-foundation/update-before-*').strip()
        self.assertTrue(reference)
        self.assertEqual(self.git('rev-parse', reference).strip(), self.before)

    def test_foreign_origin_is_not_silently_replaced(self):
        self.git('remote', 'set-url', 'origin', 'https://example.invalid/something-else.git')
        result = self.invoke('--dry-run')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('different repository', result.stderr)
        self.assertEqual(self.git('config', '--get', 'remote.origin.url').strip(), 'https://example.invalid/something-else.git')
        self.unchanged()

    def test_detached_installation_reports_specific_recovery_without_switching(self):
        self.git('switch', '--detach', self.before)
        result = self.invoke('--dry-run')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('detached HEAD', result.stderr)
        self.unchanged()

    def test_unchanged_compositor_config_does_not_request_a_redundant_reload(self):
        config = self.state / 'theme/current/niri.kdl'
        config.write_text('// same generated config')
        result = self.invoke(DF_TEST_ACTIVE='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('niri', [c['name'] for c in self.calls()])
        self.assertIn('shell-reload', [c['name'] for c in self.calls()])


@unittest.skipIf(os.geteuid() == 0, 'Updater runs as the desktop user')
class Bootstrap(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'owning checkout'
        (self.root / 'scripts').mkdir(parents=True)
        self.bin = self.base / 'bin'; self.bin.mkdir()
        for name in ['df-update', 'build-backend']:
            shutil.copy(ROOT / 'scripts' / name, self.root / 'scripts' / name)
        self.entry = self.bin / 'df-update'; self.entry.symlink_to(self.root / 'scripts/df-update')
        self.cache = self.base / 'cache'
        self.log = self.base / 'cargo.json'
        self.env = {**os.environ, 'HOME':str(self.base), 'XDG_CACHE_HOME':str(self.cache),
                    'PATH':str(self.bin)+os.pathsep+os.environ['PATH'], 'BOOTSTRAP_LOG':str(self.log)}
        cargo = self.bin / 'cargo'
        cargo.write_text('''#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
Path(os.environ['BOOTSTRAP_LOG']).write_text(json.dumps(sys.argv[1:]))
if os.environ.get('FAIL_BOOTSTRAP'): sys.exit(17)
if os.environ.get('NO_BOOTSTRAP_OUTPUT'): sys.exit(0)
p=Path(os.environ['CARGO_TARGET_DIR'])/'release/desktop-foundationctl'
p.parent.mkdir(parents=True,exist_ok=True)
p.write_text('#!/usr/bin/env python3\\nimport json,sys\\nprint(json.dumps(sys.argv[1:]))\\n')
p.chmod(0o700)
'''); cargo.chmod(0o700)

    def invoke(self, *args, **env):
        return subprocess.run([str(self.entry), *args], env={**self.env, **env},
                              text=True, capture_output=True, timeout=10)

    def test_missing_binary_bootstraps_in_cache_then_runs_updater(self):
        result = self.invoke('--version', 'v0.3.2')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout.splitlines()[-1]),
                         ['--root',str(self.root),'update','--version','v0.3.2'])
        args=json.loads(self.log.read_text())
        self.assertEqual(args[:4], ['+1.98.1','build','--locked','--release'])
        self.assertFalse((self.root/'native/foundation/target').exists())
        self.assertEqual((self.cache/'desktop-foundation/update/bootstrap').stat().st_mode & 0o777, 0o700)

    def test_read_only_modes_do_not_bootstrap(self):
        for flag in ['--dry-run','--check','--help']:
            result=self.invoke(flag)
            self.assertEqual(result.returncode, 0 if flag=='--help' else 1, result.stderr)
            self.assertFalse(self.cache.exists()); self.assertFalse(self.log.exists())

    def test_failed_bootstrap_never_runs_or_publishes(self):
        result=self.invoke(FAIL_BOOTSTRAP='1')
        self.assertNotEqual(result.returncode,0)
        self.assertIn('no desktop files were deployed',result.stderr)
        self.assertFalse((self.root/'native').exists())

    def test_success_without_binary_is_reported(self):
        result=self.invoke(NO_BOOTSTRAP_OUTPUT='1')
        self.assertNotEqual(result.returncode,0)
        self.assertIn('did not produce an executable',result.stderr)

    def test_existing_binary_does_not_bootstrap(self):
        target=self.root/'native/foundation/target/release/desktop-foundationctl'
        target.parent.mkdir(parents=True); target.write_text('#!/bin/sh\necho existing\n'); target.chmod(0o700)
        result=self.invoke()
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(result.stdout.strip(),'existing')
        self.assertFalse(self.cache.exists()); self.assertFalse(self.log.exists())


if __name__ == '__main__': unittest.main()
