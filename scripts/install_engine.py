"""Two-layer Arch-family provisioning; inspect first, reuse reversible helpers."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from application_roles import roles, validate_role, desktop_path, ownership_conflicts

ROOT = Path(__file__).resolve().parent.parent


def manifest(name):
    return [line for line in (ROOT / 'packages' / (name + '.txt')).read_text().splitlines() if line and not line.startswith('#')]


def installed(package):
    return bool(shutil.which('pacman')) and subprocess.run(['pacman', '-T', package], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0


def command(*args):
    print('+ ' + ' '.join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True)


def script(name, *args):
    command(ROOT / 'scripts' / name, *args)


def parse(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    choice = p.add_mutually_exclusive_group()
    choice.add_argument('--core', dest='personal', action='store_false')
    choice.add_argument('--personal', dest='personal', action='store_true')
    p.set_defaults(personal=True)
    p.add_argument('--profile', '--hardware-profile', default='default')
    p.add_argument('--no-greeter', dest='greeter', action='store_false', default=True)
    p.add_argument('--greeter', dest='greeter', action='store_true')
    p.add_argument('--no-packages', dest='packages', action='store_false', default=True)
    p.add_argument('--clean-boot', action='store_true')
    inspection = p.add_mutually_exclusive_group()
    inspection.add_argument('--dry-run', '--plan', action='store_true')
    inspection.add_argument('--check', action='store_true')
    args = p.parse_args(argv)
    if not args.profile.replace('-', '').replace('_', '').isalnum() or not (ROOT / 'profiles' / args.profile).is_dir():
        p.error('Unknown/invalid hardware profile')
    return args


def deployment_conflicts():
    # Inspect before any package/system mutation. Deploy still stages/validates.
    from deploy import STATE, owned
    path = STATE / 'manifest.json'
    if not path.exists():
        return ownership_conflicts()
    saved = json.loads(path.read_text())
    if saved['root'] != str(ROOT):
        return ['A different checkout owns deployment; restore from that checkout first']
    return [entry['path'] for entry in saved['entries'] if not owned(entry)] + ownership_conflicts()


def plan(args):
    groups = ['core', *(['greeter'] if args.greeter else []), *(['personal'] if args.personal else [])]
    print('Layer: ' + ('core + personal' if args.personal else 'core desktop'))
    for group in groups:
        for package in manifest(group):
            print(f'{group}: {package}: ' + ('already installed' if installed(package) else 'missing'))
    if args.personal:
        browser = roles(True)['browser']; chatgpt = json.loads((ROOT / 'apps/personal.json').read_text())['chatgpt']
        for app in [browser, chatgpt, json.loads((ROOT / 'apps/personal.json').read_text())['steam']]:
            print(f"personal: {app['package']}: " + ('adopt installed package' if installed(app['package']) else 'install from configured signed repository (Brave may use its verified AUR recipe)'))
        print('Spotify: checksum-pinned tools/Marketplace; first login, wait a minute, normal quit/reopen')
        print('Steam: stock native client; Millennium/Material excluded from v0.12; see docs/steam-theme-plan.md')
        print('Brave: stable generated theme folder; native Load unpacked required; debugging remains OFF')
    print('Firmware: ' + ', '.join(manifest('firmware')) + ' (repository preferred, reviewed AUR fallback)')
    print('Deploy backups: existing Kitty/Fish/Fastfetch directories and Niri config are retained in the ownership journal')
    print('Roles: ' + ', '.join(f"{name}={role['desktop']}" for name, role in roles(args.personal).items()))
    print('MIME writes: designated Default Applications keys only; unrelated associations/comments preserved')
    print('Services: Bluetooth/AppArmor enabled; never restart display manager')
    print('Boot: ' + ('explicit reversible GRUB/mkinitcpio cleanup' if args.clean_boot else 'preserve existing boot/initramfs stack; configure AppArmor only if kernel needs it'))
    conflicts = deployment_conflicts()
    for conflict in conflicts:
        print('CONFLICT: ' + conflict)
    subprocess.run([sys.executable, str(ROOT / 'scripts/deploy.py'), 'install', '--profile', args.profile, '--dry-run'], check=True)
    return bool(conflicts)


def provision_package(app, aur=False):
    package = app['package']
    if installed(package):
        validate_role(package, app)
        print('Adopted installed ' + package + '; account data untouched')
        return
    if shutil.which(app['executable']) or desktop_path(app['desktop']):
        raise RuntimeError('Unrelated existing application variant: ' + app['executable'] + '; resolve explicitly, nothing replaced')
    available = subprocess.run(['pacman', '-Si', package], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    if available:
        command('sudo', 'pacman', '-Syu', '--needed', package)
    elif aur:
        script('paru-bootstrap')
        # Paru presents the recipe for review; do not bypass it with --noconfirm.
        command('paru', '-S', '--needed', package)
    else:
        raise RuntimeError(package + ' is unavailable in configured signed repositories; no repository added. See docs/personal-apps.md')
    validate_role(package, app)


def core(args):
    if args.packages:
        script('bootstrap', *(['--greeter'] if args.greeter else []))
    script('build-nothing')
    script('build-backend')
    command(sys.executable, ROOT / 'scripts/preflight.py', '--profile', args.profile)
    # Stage/validate before replacing paths, retaining the original backup journal.
    script('deploy', '--compositor', 'niri', '--profile', args.profile)
    command(sys.executable, ROOT / 'scripts/preferences.py', 'install', '--theme-only')
    if not args.personal:
        script('applications', 'apply')
    command('sudo', 'systemctl', 'enable', 'bluetooth.service', 'apparmor.service')
    enabled = Path('/sys/module/apparmor/parameters/enabled')
    if args.clean_boot:
        if shutil.which('dracut') and not Path('/etc/mkinitcpio.conf').exists():
            raise RuntimeError('--clean-boot requires an existing mkinitcpio/GRUB system; dracut is never converted')
        command('sudo', ROOT / 'scripts/boot-setup', 'install')
    elif not enabled.exists() or enabled.read_text().strip() != 'Y':
        command('sudo', ROOT / 'scripts/apparmor-setup', 'install')
    if args.greeter:
        command('sudo', ROOT / 'scripts/system-setup', 'install')
    if args.clean_boot and args.greeter:
        command('sudo', ROOT / 'scripts/greeter-console-setup', 'install')
    if args.clean_boot:
        command('sudo', ROOT / 'scripts/boot-optimize', 'install')


def personal(args):
    if args.packages:
        missing = [p for p in manifest('personal') if not installed(p)]
        if missing:
            command('sudo', 'pacman', '-Syu', '--needed', *missing)
        provision_package(roles(True)['browser'], aur=True)
        provision_package(json.loads((ROOT / 'apps/personal.json').read_text())['chatgpt'])
        provision_package(json.loads((ROOT / 'apps/personal.json').read_text())['steam'])
        script('spotify-setup', 'install')
    else:
        validate_role('browser', roles(True)['browser'])
        validate_role('chatgpt', json.loads((ROOT / 'apps/personal.json').read_text())['chatgpt'])
        validate_role('steam', json.loads((ROOT / 'apps/personal.json').read_text())['steam'])
        script('spotify-setup', 'check')
    script('applications', 'apply', '--personal')
    # Browser preference seeds remain explicit post-install functionality. Normal
    # convergence never reads/writes a real profile, even to detect initialization.
    print('Brave: import theme/brave via native Load unpacked; preferences/extensions are preserved.')
    print('Native Steam provisioned/adopted; Millennium/Material excluded. Steam account/library files untouched.')


def main(argv=None):
    args = parse(argv)
    if args.dry_run:
        return plan(args)
    if args.check:
        script('doctor', '--installation', *(['--personal'] if args.personal else ['--core']), *([] if args.greeter else ['--no-greeter']))
        return 0
    if os.geteuid() == 0:
        raise RuntimeError('Run as your normal login user; AUR/native builds must not run as root')
    if not shutil.which('pacman'):
        raise RuntimeError('Arch-family pacman installation required')
    conflicts = deployment_conflicts()
    if conflicts:
        raise RuntimeError('Externally changed deployed paths: ' + ', '.join(conflicts))
    command('sudo', '-v')
    core(args)
    if args.personal:
        personal(args)
    script('doctor', '--installation', *(['--personal'] if args.personal else ['--core']), *([] if args.greeter else ['--no-greeter']))
    print('Install complete. Log into Niri and run scripts/doctor. Packages/user data retained on rollback: scripts/uninstall.')
    return 0


def cli():
    try:
        return main()
    except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
        print('Installation stopped: ' + str(error) + '\nBackups retained. Resolve the conflict/error, then rerun the same command.', file=sys.stderr)
        return 1
