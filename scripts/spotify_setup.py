#!/usr/bin/env python3
"""Optional user-local Spotify and Spicetify installation; no sudo required."""
import argparse
import fcntl
import time
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
import zipfile

from deploy import fingerprint, owned

ROOT = Path(__file__).resolve().parent.parent
DATA = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
CONFIG = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
CACHE = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'desktop-foundation/spotify'
STATE = Path(os.environ.get('XDG_STATE_HOME', Path.home() / '.local/state')) / 'desktop-foundation/spotify'
APPS = DATA / 'desktop-foundation/apps'


def download(source):
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / source['url'].rsplit('/', 1)[-1]
    if not path.exists():
        temporary = path.with_suffix(path.suffix + '.part')
        with urllib.request.urlopen(source['url'], timeout=60) as remote, temporary.open('wb') as local:
            shutil.copyfileobj(remote, local)
        temporary.replace(path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != source['sha256']:
        raise RuntimeError(f'Download checksum mismatch: {path.name}')
    return path


def link(path, source):
    # Journal ownership/backups independently of desktop configuration deployment.
    journal = STATE / 'links.json'
    entries = json.loads(journal.read_text()) if journal.exists() else []
    old = next((e for e in entries if e['path'] == str(path)), None)
    if old:
        if not path.is_symlink() or os.readlink(path) != old['source']:
            raise RuntimeError(f'Application link changed outside setup: {path}')
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    backup = STATE / f'backup-{len(entries)}' if path.exists() or path.is_symlink() else None
    if backup and backup.exists():
        raise RuntimeError(f'Backup already exists: {backup}')
    entries.append({'path': str(path), 'source': str(source), 'backup': str(backup) if backup else None, 'original': fingerprint(path)})
    temporary = journal.with_suffix('.tmp')
    temporary.write_text(json.dumps(entries, indent=2) + '\n')
    temporary.replace(journal)
    if backup:
        path.rename(backup)
    path.symlink_to(source)


def install():
    if platform.machine() != 'x86_64':
        raise RuntimeError('Pinned Spotify client is x86_64 only')
    STATE.mkdir(parents=True, exist_ok=True)
    APPS.mkdir(parents=True, exist_ok=True)
    sources = json.loads((ROOT / 'apps/spotify/sources.json').read_text())
    for name in ['spotify', 'spicetify']:
        source = sources[name]
        destination = APPS / name
        stamp = destination / '.foundation-source.json'
        if destination.exists():
            if not stamp.exists() or json.loads(stamp.read_text()) != source:
                raise RuntimeError(f'Existing app installation differs: {destination}')
            continue
        package = download(source)
        with tempfile.TemporaryDirectory(dir=APPS) as scratch:
            stage = Path(scratch)
            if name == 'spotify':
                members = subprocess.check_output(['ar', 't', str(package)], text=True).splitlines()
                payload = next(m for m in members if m.startswith('data.tar'))
                data = stage / payload
                with data.open('wb') as output:
                    subprocess.run(['ar', 'p', str(package), payload], stdout=output, check=True)
                with tarfile.open(data) as archive:
                    archive.extractall(stage, filter='data')
                data.unlink()
            else:
                with tarfile.open(package) as archive:
                    archive.extractall(stage, filter='data')
            (stage / '.foundation-source.json').write_text(json.dumps(source, indent=2) + '\n')
            stage.rename(destination)
    libraries = APPS / 'spotify-libs'
    libraries.mkdir(parents=True, exist_ok=True)
    for source in sources['runtime_libraries']:
        package = download(source)
        with tarfile.open(package) as archive:
            archive.extractall(libraries, filter='data')
    spicetify_config = CONFIG / 'spicetify'
    if spicetify_config.exists() and not (STATE / 'spicetify-config-backup').exists() and not (STATE / 'config-created').exists():
        shutil.copytree(spicetify_config, STATE / 'spicetify-config-backup')
    if not spicetify_config.exists():
        spicetify_config.mkdir(parents=True)
        (STATE / 'config-created').touch()
    marketplace = spicetify_config / 'CustomApps/marketplace'
    if not marketplace.exists():
        marketplace.mkdir(parents=True)
        with zipfile.ZipFile(download(sources['marketplace'])) as archive:
            for member in archive.infolist():
                relative = Path(member.filename)
                if relative.is_absolute() or '..' in relative.parts:
                    raise RuntimeError('Unsafe Marketplace archive path')
            for member in archive.infolist():
                relative = Path(member.filename)
                parts = relative.parts[1:] if relative.parts[0] == 'marketplace-dist' else relative.parts
                if not parts or member.is_dir():
                    continue
                target = marketplace.joinpath(*parts)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(archive.read(member))
    theme = spicetify_config / 'Themes/marketplace'
    theme.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / 'apps/spotify/marketplace-color.ini', theme / 'color.ini')
    link(Path.home() / '.local/bin/spotify', ROOT / 'scripts/spotify')
    link(Path.home() / '.local/bin/spicetify', ROOT / 'scripts/spicetify')
    desktop = STATE / 'spotify.desktop'
    desktop.write_text('[Desktop Entry]\nType=Application\nName=Spotify\nComment=Music with Spicetify Marketplace\n'
                       f'Exec="{ROOT / "scripts/spotify"}" %U\n'
                       f'Icon={APPS / "spotify/usr/share/spotify/icons/spotify-linux-128.png"}\n'
                       'Terminal=false\nCategories=AudioVideo;Audio;Music;\n'
                       'MimeType=x-scheme-handler/spotify;\nStartupWMClass=spotify\n')
    link(DATA / 'applications/spotify.desktop', desktop)
    subprocess.run([str(ROOT / 'scripts/spicetify'), 'config', 'spotify_path', str(APPS / 'spotify/usr/share/spotify'),
                    'prefs_path', str(CONFIG / 'spotify/prefs'), 'custom_apps', 'marketplace',
                    'inject_css', '1', 'replace_colors', '1', 'current_theme', 'marketplace'], check=True)
    subprocess.run(['update-desktop-database', str(DATA / 'applications')], check=False)
    print('Installed Spotify, Spicetify and Marketplace. Open Spotify, log in, then run scripts/spotify-setup apply.')


def apply():
    if not (CONFIG / 'spotify/prefs').exists():
        raise RuntimeError('Open Spotify and log in first; its prefs file is not available yet.')
    subprocess.run([str(ROOT / 'scripts/spicetify'), 'backup', 'apply'], check=True)


def restore():
    journal = STATE / 'links.json'
    entries = json.loads(journal.read_text()) if journal.exists() else []
    for entry in entries:
        path = Path(entry['path'])
        backup = Path(entry['backup']) if entry['backup'] else None
        original = entry.get('original', fingerprint(backup) if backup else None)
        if fingerprint(path) is not None and not owned(entry) and fingerprint(path) != original:
            raise RuntimeError(f'Refusing to replace modified application path: {path}')
    for entry in reversed(entries[:]):
        path = Path(entry['path'])
        if owned(entry):
            path.unlink()
        if entry['backup'] and Path(entry['backup']).exists():
            Path(entry['backup']).rename(path)
        entries.remove(entry)
        temporary = journal.with_suffix('.tmp')
        temporary.write_text(json.dumps(entries, indent=2) + '\n')
        temporary.replace(journal)
    backup = STATE / 'spicetify-config-backup'
    if backup.exists() or (STATE / 'config-created').exists():
        config = CONFIG / 'spicetify'
        if config.exists():
            config.rename(STATE / f'spicetify-config-after-{time.time_ns()}')
        if backup.exists():
            backup.rename(config)
        (STATE / 'config-created').unlink(missing_ok=True)
    print('Application links/config restored. Downloaded clients, Spotify account data and caches retained.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'apply', 'restore'])
    args = parser.parse_args()
    STATE.mkdir(parents=True, exist_ok=True)
    with (STATE / 'lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        {'install': install, 'apply': apply, 'restore': restore}[args.action]()
