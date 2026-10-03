#!/usr/bin/env python3
"""Journal config replacements before mutation; restore only owned paths."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
CONFIG = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
STATE = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation'
DATA = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))


def lua_string(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n') + '"'


def save(manifest):
    temporary = STATE / 'manifest.tmp'
    with temporary.open('w') as stream:
        stream.write(json.dumps(manifest, indent=2) + '\n')
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(STATE / 'manifest.json')
    descriptor = os.open(STATE, os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def owned(entry):
    path = Path(entry['path'])
    return path.is_symlink() and os.readlink(path) in {entry['source'], entry.get('pending_source')}


def fingerprint(path):
    if not (path.exists() or path.is_symlink()):
        return None
    stat = path.lstat()
    return [stat.st_dev, stat.st_ino, stat.st_mode]


def restore(manifest):
    for entry in manifest['entries']:
        path = Path(entry['path'])
        backup = Path(entry['backup']) if entry['backup'] else None
        if 'original' not in entry:
            entry['original'] = fingerprint(backup) if backup else None
        original_present = entry['original'] is not None and fingerprint(path) == entry['original']
        if fingerprint(path) is not None and not owned(entry) and not original_present:
            raise RuntimeError(f'Refusing to overwrite modified config: {path}')
        if original_present and backup and fingerprint(backup) is not None:
            raise RuntimeError(f'Ambiguous recovery: {path}')
    save(manifest)
    persistent_units = sorted({Path(e['path']).name for e in manifest['entries']
                               if '/systemd/user/' in e['path'] and '/niri.service.wants/' not in e['path']
                               and Path(e['path']).name.startswith('desktop-foundation-')
                               and '@' not in Path(e['path']).name})
    if persistent_units:
        subprocess.run(['systemctl', '--user', 'stop', *persistent_units,
                        'desktop-foundation-clipboard@text.service', 'desktop-foundation-clipboard@image.service'], check=True)
    for entry in reversed(manifest['entries'][:]):
        path = Path(entry['path'])
        backup = Path(entry['backup']) if entry['backup'] else None
        if owned(entry):
            path.unlink()
        if backup and fingerprint(backup) is not None:
            backup.rename(path)
        manifest['entries'].remove(entry)
        save(manifest)
    (STATE / 'manifest.json').unlink()
    subprocess.run(['systemctl', '--user', 'daemon-reload'], check=True)
    subprocess.run(['fc-cache', '-f'], check=True)
    print('Original configuration restored. Reload your active compositor if needed.')


def common_targets():
    return [(CONFIG / 'gtk-3.0/settings.ini', ROOT / 'theme/gtk-3.0/settings.ini'),
            (CONFIG / 'gtk-4.0/settings.ini', ROOT / 'theme/gtk-4.0/settings.ini'),
            (CONFIG / 'wireplumber/wireplumber.conf.d/60-desktop-foundation-bluetooth.conf', ROOT / 'audio/wireplumber/60-desktop-foundation-bluetooth.conf'),
            (CONFIG / 'kitty', STATE / 'theme/current/terminal/kitty'),
            (CONFIG / 'fish', STATE / 'terminal/fish'),
            (CONFIG / 'fastfetch', STATE / 'theme/current/terminal/fastfetch'),
            (DATA / 'fonts/desktop-foundation', ROOT / 'fonts')]


def require_native_backends():
    required = [
        ('native/foundation/target/release/desktop-foundationctl', 'scripts/build-backend'),
        ('native/nothing/target/release/foundation-nothing', 'scripts/build-nothing'),
    ]
    missing = [build for binary, build in required
               if not (ROOT / binary).is_file() or not os.access(ROOT / binary, os.X_OK)]
    if missing:
        raise RuntimeError('Required native backends are not built or executable. Run '
                           + ' and '.join(missing) + ' in this checkout; nothing deployed.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'restore'])
    parser.add_argument('--profile', default='default')
    parser.add_argument('--compositor', choices=['niri', 'hyprland'], default='niri')
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    if args.dry_run:
        from session_units import targets as session_targets
        paths = session_targets(ROOT, CONFIG, STATE, DATA, write=False, niri=args.compositor == 'niri')
        paths.extend([(CONFIG / 'niri/config.kdl', STATE / 'niri.kdl'),
                      (CONFIG / 'quickshell/desktop-foundation', ROOT / 'shell')])
        paths.extend(common_targets())
        for path, source in paths:
            print(f'Deploy with backup: {path} <- {source}')
        return
    if args.action == 'install':
        require_native_backends()
    STATE.mkdir(parents=True, exist_ok=True)
    with (STATE / 'lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        manifest_path = STATE / 'manifest.json'
        manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {'root': str(ROOT), 'entries': []}
        if manifest['root'] != str(ROOT):
            raise RuntimeError('A different checkout owns this installation; restore it first.')
        if args.action == 'restore':
            if manifest_path.exists():
                restore(manifest)
            else:
                print('Nothing deployed.')
            return
        if not re.fullmatch(r'[A-Za-z0-9_-]+', args.profile):
            raise RuntimeError('Invalid profile name')
        if not (ROOT / 'profiles' / args.profile).is_dir():
            print(f'Profile {args.profile} unavailable; using portable defaults.')
        from theme_runtime import ensure
        ensure(args.profile)
        if args.compositor == 'niri':
            wrapper = 'include ' + json.dumps(str(STATE / 'theme/current/niri.kdl')) + '\n'
            with tempfile.NamedTemporaryFile(mode='w', suffix='.kdl') as candidate:
                candidate.write(wrapper)
                candidate.flush()
                subprocess.run(['niri', 'validate', '-c', candidate.name], check=True)
            source = STATE / 'niri.kdl'
            targets = [(CONFIG / 'niri/config.kdl', source), (CONFIG / 'quickshell/desktop-foundation', ROOT / 'shell')]
        else:
            wrapper = (f'-- Generated by desktop-foundation; edit repository files.\nFOUNDATION_ROOT = {lua_string(ROOT)}\n'
                       f'FOUNDATION_PROFILE = {lua_string(args.profile)}\n'
                       'dofile(FOUNDATION_ROOT .. "/compositor/hyprland/hyprland.lua")\n')
            with tempfile.NamedTemporaryFile(mode='w', suffix='.lua') as candidate:
                candidate.write(wrapper)
                candidate.flush()
                subprocess.run(['Hyprland', '--verify-config', '-c', candidate.name], check=True)
            source = STATE / 'hyprland.lua'
            targets = [(CONFIG / 'hypr/hyprland.lua', source), (CONFIG / 'quickshell/desktop-foundation', ROOT / 'shell')]
        from session_units import targets as session_targets
        targets.extend(session_targets(ROOT, CONFIG, STATE, DATA, niri=args.compositor == 'niri'))
        # Fish owns writable universal variables outside the source checkout.
        fish_runtime = STATE / 'terminal/fish'
        fish_runtime.mkdir(parents=True, exist_ok=True)
        for child in (ROOT / 'terminal/fish').iterdir():
            link = fish_runtime / child.name
            if child.name == 'theme.fish':
                source_theme = STATE / 'theme/current/terminal/fish/theme.fish'
                if link.is_symlink() and os.readlink(link) == str(child):
                    link.unlink()
                if not link.exists() and not link.is_symlink():
                    link.symlink_to(source_theme)
                continue
            if not link.exists() and not link.is_symlink():
                link.symlink_to(child)
        subprocess.run(['fish', '--no-config', '-n', str(ROOT / 'terminal/fish/config.fish')], check=True)
        targets.extend(common_targets())
        for path, src in targets:
            entries = [e for e in manifest['entries'] if e['path'] == str(path)]
            if entries and not owned(entries[0]):
                raise RuntimeError(f'Deployed path changed outside this script: {path}')
        temporary = source.with_suffix('.tmp')
        temporary.write_text(wrapper)
        temporary.replace(source)
        try:
            for path, src in targets:
                existing = next((e for e in manifest['entries'] if e['path'] == str(path)), None)
                if existing:
                    if existing['source'] != str(src):
                        existing['pending_source'] = str(src)
                        save(manifest)
                        replacement = path.with_name(path.name + '.foundation-link')
                        replacement.symlink_to(src)
                        replacement.replace(path)
                        existing['source'] = str(src)
                        existing.pop('pending_source')
                        save(manifest)
                    continue
                path.parent.mkdir(parents=True, exist_ok=True)
                backup = None
                original = fingerprint(path)
                if original is not None:
                    backup = STATE / f'backup-{len(manifest["entries"])}'
                    if fingerprint(backup) is not None:
                        raise RuntimeError(f'Unexpected backup exists: {backup}')
                entry = {'path': str(path), 'source': str(src),
                         'backup': str(backup) if backup else None, 'original': original}
                manifest['entries'].append(entry)
                save(manifest)  # Write-ahead: SIGKILL after any following step is recoverable.
                if backup:
                    path.rename(backup)
                path.symlink_to(src)

        except Exception:
            restore(manifest)
            raise
        subprocess.run(['systemctl', '--user', 'daemon-reload'], check=True)
        subprocess.run(['python3', str(ROOT / 'scripts/preferences.py'), 'install', '--theme-only'], check=True)
        subprocess.run(['fc-cache', '-f'], check=True)
        print(f'Deployed {args.compositor} profile {args.profile}; backups and manifest: {STATE}')
        print('No session restart performed. Config changes may auto-reload in the selected compositor.')


if __name__ == '__main__':
    main()
