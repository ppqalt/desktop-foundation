#!/usr/bin/env python3
"""Allowlisted Brave Origin export and reversible, one-time preference seeding."""
import argparse
import ctypes
import errno
import stat
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import shlex
import subprocess
import tempfile

CHANNEL = 'auto'
HERE = Path(__file__).resolve().parent
MISSING = object()
STORE = 'https://clients2.google.com/service/update2/crx'


def read(path):
    return json.loads(path.read_text()) if path.exists() else {}


def get(data, key):
    for part in key.split('.'):
        if not isinstance(data, dict) or part not in data:
            return MISSING
        data = data[part]
    return data


def put(data, key, value):
    parts = key.split('.')
    for part in parts[:-1]:
        if part in data and not isinstance(data[part], dict):
            raise RuntimeError('Preference structure conflicts at ' + key)
        data = data.setdefault(part, {})
    if value is MISSING:
        data.pop(parts[-1], None)
    else:
        data[parts[-1]] = value


def valid(value, kind):
    if kind == 'bool':
        return type(value) is bool
    if kind == 'int':
        return type(value) is int and -(2 ** 31) <= value < 2 ** 31
    if kind == 'languages':
        return isinstance(value, str) and bool(re.fullmatch(r'[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*(?:,[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*)*', value))
    return kind == 'webrtc' and value in ['default', 'default_public_and_private_interfaces', 'default_public_interface_only', 'disable_non_proxied_udp']


def atomic(path, data):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.is_symlink():
        raise RuntimeError('Refusing symlink preference file: ' + str(path))
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as output:
        candidate = Path(output.name)
        try:
            json.dump(data, output, indent=2, sort_keys=True)
            output.write('\n')
            output.flush()
            os.fsync(output.fileno())
            candidate.chmod(0o600)
            candidate.replace(path)
        finally:
            candidate.unlink(missing_ok=True)


def config_base():
    return Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))


def channel_name(root=None):
    if CHANNEL != 'auto':
        return CHANNEL
    if root is not None and root.name == 'Brave-Origin-Nightly':
        return 'nightly'
    return 'stable' if shutil.which('brave-origin') else 'nightly'


def flags_path(root):
    return config_base() / ('brave-origin-nightly-flags.conf' if channel_name(root) == 'nightly' else 'brave-origin-flags.conf')


def default_root(channel=None):
    selected = channel or channel_name()
    wrapper = shutil.which('brave-origin' if selected == 'stable' else 'brave-origin-nightly')
    if not wrapper:
        raise RuntimeError('Install brave-origin-bin' if selected == 'stable' else 'Install optional brave-origin-nightly-bin')
    # Current stable packaging uses upstream XDG discovery; older Nightly
    # wrappers explicitly exported a home-relative directory instead.
    match = re.search(r'CHROME_USER_DATA_DIR=~/(\.config/BraveSoftware/[^\s]+)', Path(wrapper).read_text())
    if match:
        return Path.home() / match[1]
    return config_base() / 'BraveSoftware' / ('Brave-Origin' if selected == 'stable' else 'Brave-Origin-Nightly')


def process_roots():
    result = []
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            executable = os.readlink(proc / 'exe')
            if 'brave-origin' not in executable or Path(executable).name not in ('brave', 'brave-origin') or proc.stat().st_uid != os.getuid():
                continue
            arguments = [x.decode(errors='replace') for x in (proc / 'cmdline').read_bytes().split(b'\0')]
            root = next((x.split('=', 1)[1] for x in arguments if x.startswith('--user-data-dir=')), None)
            if '--user-data-dir' in arguments:
                root = arguments[arguments.index('--user-data-dir') + 1]
            if not root:
                environment = (proc / 'environ').read_bytes().split(b'\0')
                root = next((x.split(b'=', 1)[1].decode() for x in environment if x.startswith(b'CHROME_USER_DATA_DIR=')), None)
            result.append(Path(root).expanduser().resolve() if root else (config_base() / 'BraveSoftware' / ('Brave-Origin-Nightly' if 'nightly' in executable else 'Brave-Origin')).resolve())
        except (OSError, ValueError, IndexError):
            continue
    return result


def locate(explicit=None, profile=None):
    roots = sorted(set(process_roots()))
    root = Path(explicit).expanduser().resolve() if explicit else default_root().resolve() if CHANNEL != 'auto' else Path(os.environ['CHROME_USER_DATA_DIR']).expanduser().resolve() if os.environ.get('CHROME_USER_DATA_DIR') else roots[0] if len(roots) == 1 else default_root().resolve()
    if not explicit and CHANNEL == 'auto' and len(roots) > 1:
        raise RuntimeError('Multiple Brave Origin data roots are open; specify --user-data-dir')
    state = read(root / 'Local State')
    name = profile or state.get('profile', {}).get('last_used')
    if not name:
        candidates = [p.parent.name for p in root.glob('*/Preferences') if p.parent.name != 'System Profile']
        if len(candidates) > 1:
            raise RuntimeError('Multiple profiles exist; specify --profile rather than guessing')
        name = candidates[0] if candidates else 'Default'
    if not re.fullmatch(r'[A-Za-z0-9 _-]+', name) or name == 'System Profile':
        raise RuntimeError('Invalid personal profile name')
    return root, root / name


def busy(root):
    if root.resolve() in process_roots():
        return True
    # A stale singleton should be cleared by Brave itself, not by this helper.
    lock = root / 'SingletonLock'
    return lock.exists() or lock.is_symlink()


def export(root, profile):
    allow = read(HERE / 'allowlist.json')
    result = {}
    for filename, keys in allow.items():
        source = read((profile if filename == 'Preferences' else root) / filename)
        result[filename] = {key: value for key, kind in keys.items()
                            if (value := get(source, key)) is not MISSING and valid(value, kind)}
    atomic(HERE / 'preferences.json', result)
    extensions = []
    installed = read(profile / 'Preferences').get('extensions', {}).get('settings', {})
    for directory in sorted((profile / 'Extensions').glob('*')):
        if not re.fullmatch('[a-p]{32}', directory.name):
            continue
        manifests = sorted(directory.glob('*/manifest.json'))
        if not manifests:
            continue
        # Existing directories may survive removal. Only retain metadata-backed
        # Web Store extensions, not unpacked/component extensions or directory paths.
        metadata = installed.get(directory.name, {})
        if not metadata or metadata.get('location') not in [1, 2, 3, 6, 7, 9, 10]:
            continue
        manifest = read(manifests[-1])
        if manifest.get('update_url') != STORE:
            continue
        name = manifest.get('name', '')
        if name.startswith('__MSG_'):
            messages = read(manifests[-1].parent / '_locales' / manifest.get('default_locale', 'en') / 'messages.json')
            name = next((v.get('message', '') for k, v in messages.items() if k.lower() == name[6:-2].lower()), '')
        if not isinstance(name, str) or not re.fullmatch(r'[A-Za-z0-9 ()+:—–,._-]{1,100}', name):
            name = 'Chrome Web Store extension'
        extensions.append({'id': directory.name, 'name': name, 'source': 'chrome-web-store',
                           'url': 'https://chromewebstore.google.com/detail/' + directory.name,
                           'installation': 'manual-web-store'})
    atomic(HERE / 'extensions.json', extensions)
    # Protected default-search template contains IDs, timestamps and integrity
    # hashes. Record only the known public stock Brave Search identity.
    source = read(profile / 'Preferences')
    provider = get(source, 'default_search_provider_data.template_url_data.prepopulate_id')
    atomic(HERE / 'search.json', {'provider': 'Brave Search', 'deployment': 'native-default'} if provider == 550 else {'deployment': 'manual', 'reason': 'Non-default provider excluded; configure interactively'})
    flags = flags_path(root)
    safe_flags = []
    if flags.exists() and flags.resolve() != HERE / 'flags.conf':
        for argument in shlex.split(flags.read_text(), comments=True):
            if re.fullmatch(r'--ozone-platform=(wayland|auto|x11)', argument):
                safe_flags.append(argument)
    # Preserve tracked flags when exporting an already-deployed configuration.
    if safe_flags:
        (HERE / 'flags.conf').write_text('# Allowlisted native platform flag.\n' + '\n'.join(sorted(set(safe_flags))) + '\n')
    print(f'Exported {sum(map(len, result.values()))} allowlisted preferences and {len(extensions)} extension IDs. No runtime state exported.')


def load_seeds():
    allow, seeds = read(HERE / 'allowlist.json'), read(HERE / 'preferences.json')
    if set(seeds) - set(allow):
        raise RuntimeError('Unexpected preference document')
    for filename, keys in seeds.items():
        for key, value in keys.items():
            if key not in allow[filename] or not valid(value, allow[filename][key]):
                raise RuntimeError('Unapproved preference: ' + key)
    return seeds


def apply(root, profile, refresh=False, install=False):
    if busy(root):
        print('WARN: Brave Origin is open or owns SingletonLock. Nothing changed. Close Brave, then run scripts/post-install brave.')
        return False
    seeds = load_seeds()
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation/brave'
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    identifier = hashlib.sha256(str(profile).encode()).hexdigest()[:20]
    journal = state / (identifier + '.json')
    with (state / 'lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        record = read(journal)
        if install and not record and (profile / 'Preferences').exists():
            print('Existing Brave profile preserved; use scripts/brave-config apply explicitly to seed its settings.')
            return True
        if record.get('complete') and not refresh:
            print('Brave preferences already seeded; interactive changes preserved.')
            return True
        record.setdefault('files', {})
        candidates = {}
        for filename, keys in seeds.items():
            if not keys:
                continue
            target = (profile if filename == 'Preferences' else root) / filename
            data = read(target)
            entries = record['files'].setdefault(filename, {})
            for key, value in keys.items():
                original = get(data, key)
                if original is not MISSING and not valid(original, read(HERE / 'allowlist.json')[filename][key]):
                    raise RuntimeError('Unexpected existing value type at ' + key + '; no preferences written')
                entries.setdefault(key, {'present': original is not MISSING, 'original': None if original is MISSING else original})
                entries[key]['managed'] = value
                put(data, key, value)
            candidates[target] = data
        flags = flags_path(root)
        source = HERE / 'flags.conf'
        if 'flags' not in record:
            record['flags'] = {'path': str(flags), 'source': str(source),
                               'backup': str(state / (identifier + '-flags.original')) if flags.exists() or flags.is_symlink() else None}
        elif (flags.exists() or flags.is_symlink()) and not (flags.is_symlink() and flags.resolve() == source):
            raise RuntimeError('Flags changed outside deployment; preferences not written')
        record['complete'] = False
        atomic(journal, record)  # Write-ahead originals; only allowlisted fields.
        if busy(root):
            raise RuntimeError('Browser started during preparation; no preference files written. Close it and rerun.')
        entry = record['flags']
        flags.parent.mkdir(parents=True, exist_ok=True)
        if not (flags.is_symlink() and flags.resolve() == source):
            if entry['backup'] and (flags.exists() or flags.is_symlink()):
                backup = Path(entry['backup'])
                if backup.exists() or backup.is_symlink():
                    raise RuntimeError('Ambiguous flags backup; refusing overwrite')
                flags.rename(backup)
            flags.symlink_to(source)
        for target, data in candidates.items():
            atomic(target, data)
        record['complete'] = True
        atomic(journal, record)
    print('Brave allowlisted preferences seeded; unrelated profile state retained. Extensions remain Web Store installation steps.')
    return True


def restore(root, profile):
    if busy(root):
        raise RuntimeError('Close Brave before restoring preferences; nothing changed')
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation/brave'
    journal = state / (hashlib.sha256(str(profile).encode()).hexdigest()[:20] + '.json')
    if not journal.exists():
        print('No Brave preference deployment to restore.')
        return
    with (state / 'lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        record = read(journal)
        candidates = {}
        for filename, entries in record['files'].items():
            target = (profile if filename == 'Preferences' else root) / filename
            data = read(target)
            for key, entry in entries.items():
                current = get(data, key)
                original = entry['original'] if entry['present'] else MISSING
                if current != entry['managed'] and current != original:
                    raise RuntimeError('Preference changed since deployment: ' + key)
                put(data, key, original)
            candidates[target] = data
        flags = record.get('flags')
        if flags:
            target = Path(flags['path'])
            if (target.exists() or target.is_symlink()) and not (target.is_symlink() and target.resolve() == Path(flags['source'])):
                raise RuntimeError('Flags changed since deployment; refusing restore')
        if busy(root):
            raise RuntimeError('Browser started during restoration; no preferences written')
        for target, data in candidates.items():
            atomic(target, data)
        if flags:
            target = Path(flags['path'])
            target.unlink(missing_ok=True)
            if flags['backup']:
                Path(flags['backup']).rename(target)
        journal.unlink()
    print('Original allowlisted preferences and flags restored; unrelated browser state retained.')


# Transient browser coordination/cache files must never cross channel roots.
MIGRATION_SKIP = {'SingletonLock', 'SingletonSocket', 'SingletonCookie', 'DevToolsActivePort',
                  'Cache', 'Code Cache', 'GPUCache', 'ShaderCache', 'GrShaderCache', 'DawnCache', 'Crashpad'}


def version_tuple(value):
    value = value.strip().split(':')[-1].split('-')[0]
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){2,3}', value):
        raise RuntimeError('Browser version metadata is unavailable or ambiguous')
    parts = tuple(map(int, value.split('.')))
    if len(parts) == 4:
        if parts[1] != 1:
            raise RuntimeError('Unrecognized Nightly profile version; do not force a downgrade')
        return parts[1:]
    return parts


def profile_inventory(root):
    entries = {}
    for directory, folders, files in os.walk(root, followlinks=False):
        folders[:] = [name for name in folders if name not in MIGRATION_SKIP]
        for name in folders + files:
            if name in MIGRATION_SKIP:
                continue
            path = Path(directory) / name
            info = path.lstat()
            if not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode)):
                raise RuntimeError('Profile contains an unsupported special file: ' + str(path.relative_to(root)))
            entries[str(path.relative_to(root))] = (stat.S_IFMT(info.st_mode), (0 if stat.S_ISDIR(info.st_mode) else info.st_size), info.st_mtime_ns,
                                                    os.readlink(path) if path.is_symlink() else None)
    return entries


def migrate(source=None, destination=None, target_version=None):
    target = Path(destination) if destination else config_base() / 'BraveSoftware/Brave-Origin'
    if target.exists() or target.is_symlink():
        return {'status': 'existing', 'reason': 'Stable profile preserved; no channel profiles merged'}
    if source is None:
        source = default_root('nightly') if shutil.which('brave-origin-nightly') else config_base() / 'BraveSoftware/Brave-Origin-Nightly'
    source = Path(source)
    if not source.exists():
        return {'status': 'fresh', 'reason': 'No Nightly profile to migrate'}
    state = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation/brave'
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    def pending(reason):
        value = {'status': 'pending', 'source': str(source), 'destination': str(target), 'reason': reason}
        atomic(state / 'migration.json', value)
        return value
    if source.is_symlink() or not source.is_dir():
        return pending('Nightly root must be a regular directory; select the actual profile root explicitly')
    with (state / 'migration.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if target.exists() or target.is_symlink():
            return {'status': 'existing', 'reason': 'Stable profile preserved'}
        if busy(source) or busy(target):
            return pending('Close both browser channels normally before migration; original profile unchanged')
        try:
            previous = version_tuple((source / 'Last Version').read_text())
            if target_version is None:
                result = subprocess.run(['/usr/bin/pacman', '-Q', 'brave-origin-bin'], text=True, capture_output=True, timeout=10, check=True)
                target_version = result.stdout.split()[1]
            if previous > version_tuple(target_version):
                return pending('Nightly profile is newer than Stable; wait for a compatible Stable version. No downgrade override used')
        except (OSError, RuntimeError, subprocess.SubprocessError, IndexError) as error:
            return pending(str(error))
        before = profile_inventory(source)
        required = sum(entry[1] for entry in before.values() if entry[0] == stat.S_IFREG)
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        if shutil.disk_usage(target.parent).free < required + max(256 * 1024**2, required // 10):
            return pending('Insufficient free space for an independent profile copy')
        temporary = Path(tempfile.mkdtemp(prefix='.df-origin-migrate-', dir=target.parent))
        try:
            atomic(state / 'migration.json', {'status': 'copying', 'source': str(source), 'destination': str(target), 'staging': str(temporary)})
            shutil.copytree(source, temporary, dirs_exist_ok=True, symlinks=True,
                            ignore=lambda directory, names: [name for name in names if name in MIGRATION_SKIP])
            temporary.chmod(0o700)
            # Metadata, file contents and symlink targets must match before
            # publication; recheck the source and both process locks afterwards.
            if profile_inventory(source) != before or profile_inventory(temporary) != before:
                return pending('Profile changed while copying; close the browser and retry')
            for relative, info in before.items():
                if info[0] != stat.S_IFREG:
                    continue
                def file_hash(path):
                    result = hashlib.sha256()
                    with path.open('rb') as stream:
                        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                            result.update(chunk)
                    return result.digest()
                if file_hash(source / relative) != file_hash(temporary / relative):
                    return pending('Profile copy integrity check failed; original preserved')
                with (temporary / relative).open('rb') as stream:
                    os.fsync(stream.fileno())
            for directory, _, _ in os.walk(temporary, topdown=False, followlinks=False):
                descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
                try: os.fsync(descriptor)
                finally: os.close(descriptor)
            if busy(source) or busy(target) or profile_inventory(source) != before:
                return pending('Browser started or profile changed during migration; retry after closing it')
            # Linux no-replace publication: never replace even an empty Stable
            # directory created concurrently by another application.
            rename = ctypes.CDLL(None, use_errno=True).renameat2
            rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
            if rename(-100, os.fsencode(temporary), -100, os.fsencode(target), 1) != 0:
                code = ctypes.get_errno()
                return pending('Stable profile appeared during migration' if code == errno.EEXIST else os.strerror(code))
            descriptor = os.open(target.parent, os.O_RDONLY | os.O_DIRECTORY)
            try: os.fsync(descriptor)
            finally: os.close(descriptor)
            value = {'status': 'migrated', 'source': str(source), 'destination': str(target), 'originalPreserved': True}
            atomic(state / 'migration.json', value)
            return value
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)  # Only this invocation's private staging copy.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['export', 'apply', 'restore', 'extensions', 'migrate'])
    parser.add_argument('--channel', choices=['auto', 'stable', 'nightly'], default='auto')
    parser.add_argument('--install', action='store_true', help='Seed fresh profiles only; preserve existing unowned preferences')
    parser.add_argument('--user-data-dir')
    parser.add_argument('--profile')
    parser.add_argument('--refresh', action='store_true', help='Explicitly reapply allowlisted defaults, overwriting only those settings')
    args = parser.parse_args()
    global CHANNEL
    CHANNEL = args.channel
    if args.action == 'migrate':
        result = migrate(source=args.user_data_dir)
        print(json.dumps(result))
        return 75 if result['status'] == 'pending' else 0
    if args.action == 'extensions':
        for extension in read(HERE / 'extensions.json'):
            print(extension['name'] + ': ' + extension['url'])
        return
    root, profile = locate(args.user_data_dir, args.profile)
    if args.action == 'export':
        export(root, profile)
    elif args.action == 'restore':
        restore(root, profile)
    else:
        applied = apply(root, profile, args.refresh, args.install)
        if not applied and args.install:
            print('Brave settings pending; desktop installation can continue. No browser was restarted.')
        elif not args.install:
            print('Default browser:', subprocess.check_output(['xdg-settings', 'get', 'default-web-browser'], text=True).strip())


if __name__ == '__main__':
    raise SystemExit(main())
