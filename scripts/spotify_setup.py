#!/usr/bin/env python3
"""Optional user-local Spotify and Spicetify installation; no sudo required."""
import argparse
import configparser
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


def extract_marketplace(package,destination):
    with zipfile.ZipFile(package) as archive:
        for member in archive.infolist():
            relative = Path(member.filename)
            if relative.is_absolute() or '..' in relative.parts:
                raise RuntimeError('Unsafe Marketplace archive path')
        for member in archive.infolist():
            relative = Path(member.filename)
            parts = relative.parts[1:] if relative.parts[0] == 'marketplace-dist' else relative.parts
            if not parts or member.is_dir():
                continue
            target = destination.joinpath(*parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.read(member))
    if not all((destination/name).is_file() for name in ("manifest.json","index.js")):
        raise RuntimeError("Marketplace archive layout changed")


def recover_tool_upgrade():
    journal = STATE / 'tool-upgrade.json'
    if not journal.exists():
        return
    saved = json.loads(journal.read_text())
    destination = APPS / 'spicetify'
    backup = Path(saved['backup'])
    if saved['phase'] == 'prepared' and not destination.exists():
        if not backup.is_dir():
            raise RuntimeError('Interrupted Spicetify upgrade lacks backup; inspect tool-upgrade.json')
        backup.rename(destination)
        journal.unlink()
        print('Recovered original Spicetify after interrupted publication')


def upgrade_spicetify(sources):
    """Only the known v0.10/v0.11 tool pin may upgrade; retain its binary backup."""
    from theme_pipeline import atomic
    destination = APPS / 'spicetify'; stamp = destination / '.foundation-source.json'
    source = sources['spicetify']
    old = json.loads(stamp.read_text())
    version = subprocess.check_output([str(destination / 'spicetify'), '--version'], text=True).strip()
    if version == source['version']:
        return
    if old not in sources.get('previous_spicetify', []) or version != old['version']:
        raise RuntimeError('Unknown Spicetify version/source; refusing replacement')
    package = download(source)
    journal = STATE / 'tool-upgrade.json'; backup = APPS / ('spicetify-backup-' + old['version'])
    if backup.exists():
        raise RuntimeError('Spicetify upgrade backup already exists; review tool-upgrade.json')
    with tempfile.TemporaryDirectory(dir=APPS) as scratch:
        stage = Path(scratch) / 'tool'; stage.mkdir()
        with tarfile.open(package) as archive:
            archive.extractall(stage, filter='data')
        binary = stage / 'spicetify'
        if subprocess.check_output([str(binary), '--version'], text=True).strip() != source['version']:
            raise RuntimeError('Downloaded Spicetify version differs from pin')
        (stage / '.foundation-source.json').write_text(json.dumps(source, indent=2) + '\n')
        saved = {'phase': 'prepared', 'backup': str(backup), 'source': source,
                 'binaryHash': hashlib.sha256(binary.read_bytes()).hexdigest()}
        atomic(journal, json.dumps(saved, indent=2) + '\n')
        destination.rename(backup)
        try:
            stage.rename(destination)
        except BaseException:
            backup.rename(destination); journal.unlink(); raise
        saved['phase'] = 'published'; atomic(journal, json.dumps(saved, indent=2) + '\n')
    print('Upgraded owned Spicetify; original tool retained for rollback: ' + str(backup))


def restore_tool_upgrade():
    journal = STATE / 'tool-upgrade.json'
    if not journal.exists():
        return
    saved = json.loads(journal.read_text()); destination = APPS / 'spicetify'
    backup = Path(saved['backup']); binary = destination / 'spicetify'
    if not backup.is_dir() or not binary.is_file() or hashlib.sha256(binary.read_bytes()).hexdigest() != saved['binaryHash']:
        raise RuntimeError('Spicetify tool changed externally or backup missing; refusing rollback')
    archive = APPS / ('spicetify-after-' + str(time.time_ns()))
    destination.rename(archive)
    try:
        backup.rename(destination)
    except BaseException:
        archive.rename(destination); raise
    journal.unlink()


def install():
    existing=CONFIG/'spicetify/config-xpui.ini'
    if existing.exists():
        check=configparser.ConfigParser(interpolation=None);check.read(existing)
        if check.get('Setting','current_theme',fallback='') not in ('','marketplace'):
            raise RuntimeError('Existing Spotify theme is not marketplace; refusing to replace user customization')
    if platform.machine() != 'x86_64':
        raise RuntimeError('Pinned Spotify client is x86_64 only')
    STATE.mkdir(parents=True, exist_ok=True)
    APPS.mkdir(parents=True, exist_ok=True)
    sources = json.loads((ROOT / 'apps/spotify/sources.json').read_text())
    recover_tool_upgrade()
    for name in ['spotify', 'spicetify']:
        source = sources[name]
        destination = APPS / name
        stamp = destination / '.foundation-source.json'
        if destination.exists():
            if name == 'spicetify' and stamp.exists() and (destination/'spicetify').is_file():
                version=subprocess.check_output([str(destination/'spicetify'),'--version'],text=True).strip()
                if version==source['version']:
                    continue  # Preserve an already upgraded, repo-owned matching tool.
                upgrade_spicetify(sources)
                continue
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
    if marketplace.exists() and not all((marketplace/name).is_file() for name in ('manifest.json','index.js')):
        raise RuntimeError('Existing Marketplace installation is incomplete; refusing to overwrite it')
    if not marketplace.exists():
        marketplace.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=marketplace.parent) as scratch:
            staged_marketplace=Path(scratch)/'marketplace'
            staged_marketplace.mkdir()
            extract_marketplace(download(sources['marketplace']),staged_marketplace)
            staged_marketplace.rename(marketplace)
    theme = spicetify_config / 'Themes/marketplace'
    theme.mkdir(parents=True, exist_ok=True)
    if not (theme / 'color.ini').exists():
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
    configured = STATE / 'portable-config-initialized'
    if not configured.exists() or not (CONFIG / 'spicetify/config-xpui.ini').exists():
        subprocess.run([str(ROOT / 'scripts/spicetify'), 'config', 'spotify_path', str(APPS / 'spotify/usr/share/spotify'),
                        'prefs_path', str(CONFIG / 'spotify/prefs'),
                        'inject_css', '1', 'replace_colors', '1', 'current_theme', 'marketplace'], check=True)
        # CLI custom_apps appends; write a deduplicated list to keep reruns convergent.
        config_path = CONFIG / 'spicetify/config-xpui.ini'
        portable = configparser.ConfigParser(interpolation=None)
        portable.read(config_path)
        if not portable.has_section('AdditionalOptions'):
            portable.add_section('AdditionalOptions')
        apps = [item.strip() for item in portable.get('AdditionalOptions', 'custom_apps', fallback='').split('|') if item.strip()]
        portable.set('AdditionalOptions', 'custom_apps', '|'.join(dict.fromkeys([*apps, 'marketplace'])))
        with config_path.open('w') as stream:
            portable.write(stream)
        configured.write_text('Portable defaults initialized; future user theme choices are preserved.\n')
    subprocess.run(['update-desktop-database', str(DATA / 'applications')], check=False)
    from spotify_theme import install as install_theme
    install_theme()
    print('Installed Spotify, Spicetify and Marketplace. Open Spotify, log in, wait a minute, quit normally and reopen; the launcher completes patching.')


def apply():
    if not (CONFIG / 'spotify/prefs').exists():
        raise RuntimeError('Open Spotify and log in first; its prefs file is not available yet.')
    subprocess.run([str(ROOT / 'scripts/spicetify'), 'backup', 'apply', '--no-restart'], check=True)
    from spotify_theme import install as install_theme, refresh as refresh_theme
    install_theme()
    refresh_theme()


def check():
    """Read only tool/config paths, never Spotify account preference contents."""
    source = json.loads((ROOT / 'apps/spotify/sources.json').read_text())
    for name in ('spotify', 'spicetify'):
        target = APPS / name
        stamp = target / '.foundation-source.json'
        if not stamp.is_file():
            raise RuntimeError('Missing repo-owned application: ' + name)
        if name == 'spotify' and json.loads(stamp.read_text()) != source[name]:
            raise RuntimeError('Spotify source differs from pinned client')
    binary = APPS / 'spicetify/spicetify'
    if not binary.is_file() or subprocess.check_output([str(binary), '--version'], text=True).strip() != source['spicetify']['version']:
        raise RuntimeError('Spicetify version differs from pin')
    config = configparser.ConfigParser(interpolation=None)
    config.read(CONFIG / 'spicetify/config-xpui.ini')
    expected = {'spotify_path': str(APPS / 'spotify/usr/share/spotify'), 'prefs_path': str(CONFIG / 'spotify/prefs')}
    for key, value in expected.items():
        if config.get('Setting', key, fallback='') != value:
            raise RuntimeError('Spicetify path mismatch: ' + key)
    for name in ('manifest.json', 'index.js'):
        if not (CONFIG / 'spicetify/CustomApps/marketplace' / name).is_file():
            raise RuntimeError('Marketplace missing: ' + name)
    if config.get('Setting', 'current_theme', fallback='') != 'marketplace' or config.get('Setting', 'color_scheme', fallback='') != 'DesktopFoundation':
        raise RuntimeError('Spotify graphite/Marketplace scheme not selected')
    print('PASS Spotify/Spicetify paths, source pins, Marketplace and graphite scheme')
    if not (CONFIG / 'spotify/prefs').exists():
        print('WARN first login and normal quit/reopen remain user actions')


def restore():
    from spotify_theme import restore as restore_theme
    restore_theme()
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
    (STATE / 'portable-config-initialized').unlink(missing_ok=True)
    restore_tool_upgrade()
    print('Application links/config restored. Downloaded clients, Spotify account data and caches retained.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'apply', 'restore', 'check'])
    args = parser.parse_args()
    if args.action == 'check':
        check()
        raise SystemExit(0)
    STATE.mkdir(parents=True, exist_ok=True)
    with (STATE / 'lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        {'install': install, 'apply': apply, 'restore': restore}[args.action]()
