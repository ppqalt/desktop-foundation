#!/usr/bin/env python3
"""Resumable CachyOS EAC/systemd-boot + independently signed Limine recovery.

The standard-library planner runs before Rust/compiler bootstrap and is shared
with the protected writer, avoiding separate fresh-install and repair planners.
Discovery and planning never write. Mutations require the installed root-owned
copy. Official CachyOS tools own enrollment, kernel installation and signing.
No command here reboots, deletes certificates, or changes Windows boot entries.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import struct
import subprocess
import sys
import tempfile

STATE = Path('/var/lib/desktop-foundation/boot')
HELPER = Path('/usr/local/libexec/desktop-foundation-boot')
CA = Path('/usr/share/cachyos-eac/cachyos-eac-ca.crt')
OWNER_CERT = Path('/var/lib/sbctl/keys/db/db.pem')
EFI = Path('/sys/firmware/efi/efivars')
GLOBAL = '8be4df61-93ca-11d2-aa0d-00e098032b8c'
IMAGE_SECURITY = 'd719b2cb-3d3a-4596-a3bc-dad00e67656f'
EAC_PATH = r'\EFI\cachyos-eac\systemd-bootx64.efi'
EAC_OPTIONS = ('lockdown=integrity', 'lsm=landlock,lockdown,yama,integrity,bpf',
               'iommu.passthrough=0', 'intel_iommu=on', 'module.sig_enforce=1',
               'sysctl.kernel.kexec_load_disabled=1')
PACKAGES = ('linux-cachyos-eac', 'linux-lts', 'systemd-boot-cachyos-signed',
            'cachyos-eac-signing-key', 'sbctl', 'sbsigntools', 'tpm2-tools',
            'efibootmgr', 'limine', 'limine-mkinitcpio-hook', 'snapper',
            'snap-pac', 'limine-snapper-sync')
MIN_SETTINGS = '1.4.4'
LIMIT = 8 * 1024 * 1024


class Checkpoint(RuntimeError):
    """Persistent work needs a firmware reboot or an explicit owner action."""
    desktop_safe = False


def command(*args, required=True, interactive=False, timeout=30, input=None, boot_manager=False):
    # No shell, caller-supplied executable, inherited loader path or sudo inside
    # the helper. Package/authentication prompts keep the user's terminal.
    env = {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C.UTF-8', 'HOME': '/root' if os.geteuid() == 0 else str(Path.home())}
    if boot_manager:
        env['EAC_BOOT_MANAGER'] = 'limine'
    result = subprocess.run(list(args), input=input, text=True, env=env,
                            capture_output=not interactive,
                            timeout=None if interactive else timeout)
    if not interactive and len(result.stdout) + len(result.stderr) > LIMIT:
        raise RuntimeError('Command output exceeds bound: ' + args[0])
    if required and result.returncode:
        detail = '' if interactive else (result.stderr or result.stdout).strip()[-1500:]
        raise RuntimeError(f'{args[0]} failed ({result.returncode}): {detail}')
    return result


def optional(*args):
    try:
        r = command(*args, required=False)
        return r.stdout.strip() if r.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired):
        return None


def read(path, default=None):
    try:
        p = Path(path)
        if p.stat().st_size > LIMIT:
            raise RuntimeError('Oversized input: ' + str(p))
        return p.read_text().strip()
    except (FileNotFoundError, PermissionError):
        return default


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def protected(path, directory=False):
    p = Path(path)
    info = p.lstat()
    if info.st_uid != 0 or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
        raise RuntimeError('Expected protected root-owned path: ' + str(p))
    if directory and not stat.S_ISDIR(info.st_mode):
        raise RuntimeError('Expected directory: ' + str(p))
    return info


def safe_parents(path):
    for parent in reversed(Path(path).parents):
        if parent.exists():
            protected(parent, True)


def atomic(path, data, mode=0o600):
    p = Path(path)
    safe_parents(p)
    if p.exists() or p.is_symlink():
        protected(p)
    p.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    safe_parents(p)
    fd, tmp = tempfile.mkstemp(prefix='.' + p.name, dir=p.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data if isinstance(data, bytes) else data.encode())
            os.fchmod(stream.fileno(), mode)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(tmp, p)
        fd = os.open(p.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def efivar_bool(name):
    try:
        data = (EFI / (name + '-' + GLOBAL)).read_bytes()
        return data[4] == 1 if len(data) == 5 else None
    except (OSError, IndexError):
        return None


def efi_certificates(data):
    """Parse EFI_SIGNATURE_LIST; compare complete DER identities, never CNs."""
    if len(data) < 4:
        raise ValueError('Missing EFI variable attributes')
    result = []
    offset = 4  # efivarfs attributes
    x509 = bytes.fromhex('a159c0a5e494a74a87b5ab155c2bf072')
    while offset < len(data):
        if len(data) - offset < 28:
            raise ValueError('Truncated EFI signature list')
        kind = data[offset:offset + 16]
        size, header, signature = struct.unpack_from('<III', data, offset + 16)
        if size < 28 + header or size > len(data) - offset or signature < 16:
            raise ValueError('Invalid EFI signature list dimensions')
        payload = size - 28 - header
        if payload % signature:
            raise ValueError('Partial EFI signature record')
        if kind == x509:
            for start in range(offset + 28 + header, offset + size, signature):
                result.append(hashlib.sha256(data[start + 16:start + signature]).hexdigest())
        offset += size
    return result


def certificate_hash(path):
    if not Path(path).is_file():
        return None
    r = subprocess.run(['/usr/bin/openssl', 'x509', '-in', str(path), '-outform', 'DER'],
                       capture_output=True, timeout=10, env={'PATH': '/usr/bin:/bin'})
    return hashlib.sha256(r.stdout).hexdigest() if r.returncode == 0 else None


def enrolled(name):
    try:
        return efi_certificates((EFI / (name + '-' + (IMAGE_SECURITY if name == 'db' else GLOBAL))).read_bytes())
    except FileNotFoundError:
        return [] if efivar_bool('SetupMode') is True else None
    except (OSError, ValueError):
        return None


def normalize_efi(path):
    return path.replace('/', '\\').lower()


def parse_nvram(text):
    result = {'order': [], 'entries': {}, 'current': None, 'next': None}
    for line in text.splitlines():
        for prefix, field in [('BootOrder:', 'order'), ('BootCurrent:', 'current'), ('BootNext:', 'next')]:
            if line.startswith(prefix):
                value = line.split(':', 1)[1].strip().upper()
                if field == 'order':
                    result[field] = value.split(',') if value else []
                else:
                    result[field] = value
        entry = re.match(r'^Boot([0-9A-Fa-f]{4})\*?\s+(.*?)\t(.+)$', line)
        if entry:
            number, label, device = entry.groups()
            identity = {'label': label, 'partuuid': None, 'path': normalize_efi(device)}
            partition = re.search(r'HD\(\d+,GPT,([0-9a-fA-F-]{36}),[^)]*\)', device, re.I)
            executable = re.search(r'(?:/File\()?(/?\\[^\n]*?\.efi)', device, re.I)
            if partition and executable:
                identity.update(partuuid=partition[1].lower(), path=normalize_efi(executable[1].lstrip('/')))
            result['entries'][number.upper()] = identity
    if len(result['order']) != len(set(result['order'])):
        raise ValueError('Duplicate BootOrder numbers')
    if any(not re.fullmatch(r'[0-9A-F]{4}', n) for n in result['order']):
        raise ValueError('Invalid firmware BootOrder')
    return result


def resolve_entry(nvram, partuuid, path=None, limine=False):
    matches = [n for n, e in nvram['entries'].items() if e['partuuid'] == partuuid.lower()
               and (('limine' in e['path']) if limine else e['path'] == normalize_efi(path))]
    if len(matches) > 1:
        raise Checkpoint('Multiple matching firmware entries; resolve duplicates before continuing')
    return matches[0] if matches else None


def selected_order(nvram, eac, limine):
    return [eac, limine] + [n for n in nvram['order'] if n not in (eac, limine)]


def restored_order(original, current):
    """Resolve recreated IDs by partition/path, preserve unrelated/new entries."""
    order = []
    for n in original['order']:
        identity = original['entries'].get(n)
        if identity:
            matches = [new for new, entry in current['entries'].items()
                       if entry['partuuid'] == identity['partuuid'] and entry['path'] == identity['path']]
            if len(matches) > 1:
                raise Checkpoint('Ambiguous recreated firmware entry; original order retained in journal')
            found = matches[0] if matches else None
        else:
            # Never reuse an old number now assigned to a different EFI path.
            found = n if n in current['order'] and n not in current['entries'] else None
        if found and found not in order:
            order.append(found)
    return order + [n for n in current['order'] if n not in order]


def assignments(text):
    result = {}
    for line in (text or '').splitlines():
        match = re.fullmatch(r'\s*([A-Za-z_][A-Za-z_0-9]*)\s*=\s*(.*?)\s*', line)
        if match:
            value = match[2]
            if value[:1] in ('"', "'") and value[-1:] == value[:1]:
                value = value[1:-1]
            result[match[1]] = value
    return result


def kernel_options(cmdline):
    keys = {x.split('=', 1)[0] for x in EAC_OPTIONS} | {'BOOT_IMAGE', 'initrd', 'apparmor'}
    words = cmdline.split()
    if any('\\' in w or any(c in w for c in '\n\r\x00') for w in words if w.startswith('root=')):
        raise Checkpoint('Unsupported root parameter encoding')
    if any('.snapshots' in w for w in words):
        raise Checkpoint('Boot the normal root, not a snapshot, before regenerating EAC entries')
    kept = [w for w in words if w.split('=', 1)[0] not in keys]
    if not any(w.startswith('root=') for w in kept):
        raise Checkpoint('Running command line has no root=; use the upstream supported root setup')
    return ' '.join(kept + list(EAC_OPTIONS))


def driver_plan(packages, kernels, compute):
    additions, replacements, blockers = [], [], []
    nvidia = bool(compute) or any(p in packages for p in ('nvidia', 'nvidia-open', 'nvidia-dkms', 'nvidia-open-dkms', 'nvidia-580xx-dkms', 'nvidia-open-lts')) or any(p.endswith(('-nvidia', '-nvidia-open', '-nvidia-580xx')) for p in packages)
    if nvidia:
        try:
            caps = [float(x) for x in compute.splitlines()]
        except (ValueError, AttributeError):
            caps = []
        if not caps or min(caps) < 5 or (min(caps) < 7.5 <= max(caps)):
            return [], [], ['NVIDIA architecture is unknown, unsupported or mixed; choose compatible precompiled drivers explicitly']
        family = 'nvidia-open' if min(caps) >= 7.5 else 'nvidia-580xx'
        for kernel in sorted(set(kernels) | {'linux-cachyos-eac', 'linux-lts'}):
            if kernel == 'linux-lts':
                name = 'nvidia-open-lts' if family == 'nvidia-open' else 'nvidia-580xx-lts'
            elif kernel == 'linux':
                name = family
            elif kernel.startswith('linux-cachyos'):
                name = kernel + '-' + family
            else:
                blockers.append('No verified precompiled NVIDIA mapping for retained kernel ' + kernel)
                continue
            additions.append(name)
        replacements = [p for p in packages if p in ('nvidia-open-dkms', 'nvidia-dkms', 'nvidia-580xx-dkms')]
    for module, enabled in [('zfs', 'zfs-utils' in packages), ('r8125', 'r8125-dkms' in packages)]:
        if enabled:
            additions.append('linux-cachyos-eac-' + module)
    for package in packages:
        if package.endswith('-dkms') and package not in replacements:
            # Keep consumers on other kernels; do not pretend a DKMS-only module
            # will work in EAC. Optional modules need an explicit compatibility review.
            if package not in ('zfs-dkms', 'r8125-dkms'):
                blockers.append('DKMS module unavailable on EAC: ' + package + '; explicitly retain it only for recovery kernels or provide an upstream prebuilt replacement')
    if any(name.endswith('-dkms') and name not in replacements for name in packages):
        # Retained DKMS consumers need headers on their conventional kernels,
        # including a newly selected LTS. EAC is never a DKMS target.
        for kernel in sorted(set(kernels) | {'linux-lts'}):
            if kernel == 'linux-cachyos-eac':
                continue
            if kernel in ('linux', 'linux-lts') or kernel.startswith('linux-cachyos'):
                additions.append(kernel + '-headers')
            else:
                blockers.append('No verified header mapping for retained DKMS consumer ' + kernel)
    if 'linux-cachyos-eac-headers' in packages:
        blockers.append('EAC headers are installed; remove the unsupported DKMS/header setup explicitly')
    return sorted(set(additions)), replacements, blockers


def scan():
    facts = {'schema': 1, 'privileged_discovery': os.geteuid() == 0, 'kernel': os.uname().release, 'boot_id': read('/proc/sys/kernel/random/boot_id'),
             'os': assignments(read('/etc/os-release')), 'board_vendor': read('/sys/class/dmi/id/board_vendor', ''),
             'uefi': Path('/sys/firmware/efi').is_dir(), 'secure_boot': efivar_bool('SecureBoot'),
             'setup_mode': efivar_bool('SetupMode'), 'cmdline': read('/proc/cmdline', ''),
             'tpm2': read('/sys/class/tpm/tpm0/tpm_version_major') == '2',
             'iommu_firmware': any(Path('/sys/firmware/acpi/tables', n).exists() for n in ('DMAR', 'IVRS')),
             'iommu_groups': len(list(Path('/sys/kernel/iommu_groups').glob('[0-9]*'))),
             'keys': {}, 'notes': []}
    facts['packages'] = dict(line.split(' ', 1) for line in (optional('/usr/bin/pacman', '-Q') or '').splitlines() if ' ' in line)
    facts['kernels'] = {p.parent.name: read(p) for p in Path('/usr/lib/modules').glob('*/pkgbase') if read(p) in facts['packages']}
    facts['dkms'] = optional('/usr/bin/dkms', 'status') if shutil.which('dkms') else ''
    facts['compute'] = optional('/usr/bin/nvidia-smi', '--query-gpu=compute_cap', '--format=csv,noheader') if shutil.which('nvidia-smi') else None
    facts['gpu'] = optional('/usr/bin/lspci', '-nnk')
    for name, source in [('PK', '/var/lib/sbctl/keys/PK/PK.pem'), ('KEK', '/var/lib/sbctl/keys/KEK/KEK.pem'), ('db', str(OWNER_CERT))]:
        certificates = enrolled(name)
        local = certificate_hash(source)
        facts['keys'][name] = {'fingerprints': certificates, 'local': local,
                               'local_enrolled': local in certificates if local and certificates is not None else None}
    db = facts['keys']['db']['fingerprints']
    ca = certificate_hash(CA)
    facts['eac_ca_available'] = bool(ca)
    facts['eac_ca_enrolled'] = ca in db if ca and db is not None else None
    facts['sbctl_database'] = optional('/usr/bin/sbctl', 'list-files')
    try:
        facts['nvram'] = parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)
    except (OSError, RuntimeError, ValueError):
        facts['nvram'] = None
    esp = optional('/usr/bin/bootctl', '--print-esp-path')
    facts['esp'] = None
    if esp and Path(esp).is_absolute():
        mount = optional('/usr/bin/findmnt', '-J', '--mountpoint', esp, '-o', 'TARGET,SOURCE,FSTYPE,UUID,OPTIONS')
        if mount:
            row = json.loads(mount)['filesystems'][0]
            device = row['source']
            block = optional('/usr/bin/lsblk', '-J', '-o', 'PATH,PKNAME,PARTUUID', device)
            if block:
                block = json.loads(block)['blockdevices'][0]
                part = read('/sys/class/block/' + Path(device).name + '/partition')
                space = os.statvfs(esp)
                facts['esp'] = {**row, 'device': device, 'disk': '/dev/' + Path(block.get('pkname') or '').name,
                                'partuuid': block.get('partuuid'), 'partition': part,
                                'capacity': space.f_blocks * space.f_frsize, 'free': space.f_bavail * space.f_frsize}
    root = optional('/usr/bin/findmnt', '-J', '--mountpoint', '/', '-o', 'SOURCE,FSTYPE,UUID,OPTIONS')
    facts['root'] = json.loads(root)['filesystems'][0] if root else None
    facts['backing_root'] = None
    if facts['root'] and facts['root']['fstype'] == 'overlay':
        lower = re.search(r'(?:^|,)lowerdir=([^,:]+)', facts['root'].get('options', ''))
        if lower and Path(lower[1]).is_absolute():
            backing = optional('/usr/bin/findmnt', '-J', '-T', lower[1], '-o', 'SOURCE,FSTYPE,UUID,OPTIONS')
            if backing:
                facts['backing_root'] = json.loads(backing)['filesystems'][0]
    facts['subvolumes'] = optional('/usr/bin/btrfs', 'subvolume', 'list', '-a', '/') if facts['root'] and facts['root']['fstype'] == 'btrfs' else None
    facts['efi_files'] = None
    if facts['esp'] and os.access(Path(facts['esp']['target']) / 'EFI', os.R_OK | os.X_OK):
        facts['efi_files'] = [{'path': str(p.relative_to(esp)), 'size': p.stat().st_size}
                              for p in Path(esp).joinpath('EFI').rglob('*') if p.is_file() and not p.is_symlink()]
    facts['block_devices'] = json.loads(optional('/usr/bin/lsblk', '-J', '-o', 'PATH,TYPE,FSTYPE,UUID,MOUNTPOINTS') or '{"blockdevices":[]}')
    crypttab = [line.split() for line in read('/etc/crypttab', '').splitlines() if line.strip() and not line.lstrip().startswith('#')]
    facts['encryption'] = {'crypttab': [{'name': fields[0], 'device': fields[1], 'options': fields[3] if len(fields) > 3 else ''} for fields in crypttab if len(fields) >= 2], 'tpm_bound': 'tpm2' in read('/etc/crypttab', '').lower(),
                           'bitlocker_possible': 'bitlocker' in json.dumps(facts['block_devices']).lower() or any('microsoft' in entry['path'] or 'windows' in entry['label'].lower() for entry in (facts['nvram'] or {}).get('entries', {}).values())}
    # Key files are never read or copied. Enumerate TPM tokens in LUKS metadata.
    def luks_devices(rows):
        for row in rows:
            if str(row.get('fstype', '')).lower() == 'crypto_luks':
                yield row['path']
            yield from luks_devices(row.get('children', []))
    for device in luks_devices(facts['block_devices']['blockdevices']):
        metadata = optional('/usr/bin/cryptsetup', 'luksDump', '--dump-json-metadata', device)
        if metadata and 'systemd-tpm2' in metadata:
            facts['encryption']['tpm_bound'] = True
    facts['limine'] = assignments(read('/etc/default/limine'))
    facts['snapper'] = {p.name: assignments(read(p)) for p in Path('/etc/snapper/configs').glob('*') if p.is_file()}
    try:
        result = command('/usr/bin/bootctl', 'status', '--no-pager', required=False)
        facts['bootctl'] = {'text': result.stdout, 'error': result.stderr, 'returncode': result.returncode}
    except OSError as error:
        facts['bootctl'] = {'error': str(error)}
    facts['initramfs'] = [n for n in ('mkinitcpio', 'dracut') if shutil.which(n)]
    facts['hooks'] = sorted(str(p) for directory in ('/etc/pacman.d/hooks', '/usr/share/libalpm/hooks')
                            for p in Path(directory).glob('*') if any(s in p.name.lower() for s in ('limine', 'snap', 'eac', 'sbctl', 'kernel')))
    return facts


def plan(facts, allow_dkms=False):
    checks = []
    def check(name, category, message):
        checks.append({'name': name, 'category': category, 'message': message})
    check('distribution', 'ready' if facts['os'].get('ID') == 'cachyos' else 'unsupported', 'CachyOS is required')
    check('firmware', 'ready' if facts['uefi'] else 'unsupported', 'UEFI x86_64 boot is required')
    check('architecture', 'ready' if os.uname().machine == 'x86_64' else 'unsupported', 'Upstream EAC binaries are x86_64')
    esp = facts['esp']
    check('esp', 'ready' if esp and esp['fstype'] == 'vfat' and re.fullmatch(r'[0-9a-fA-F-]{36}', esp.get('partuuid') or '') and re.fullmatch(r'[1-9][0-9]*', esp.get('partition') or '') and re.fullmatch(r'/dev/[A-Za-z0-9_.-]+', esp.get('disk') or '') and re.fullmatch(r'/[A-Za-z0-9_./-]+', esp.get('target') or '') else 'explicit-action', 'Mounted FAT ESP and disk/partition identity must be discoverable')
    if esp:
        check('esp-space', 'ready' if esp['free'] >= 512 * 1024**2 else 'explicit-action', 'At least 512 MiB free ESP space is reserved for independent boot/recovery images')
    check('root', 'ready' if facts['root'] and facts['root']['fstype'] == 'btrfs' and facts['root'].get('uuid') else 'unsupported', 'Snapshot recovery requires a Btrfs root; no filesystem conversion is performed')
    check('tpm', 'ready' if facts['tpm2'] else 'firmware', 'Enable TPM 2.0 / AMD fTPM / Intel PTT')
    check('iommu', 'ready' if facts['iommu_firmware'] else 'firmware', 'Enable IOMMU / AMD-Vi / Intel VT-d')
    check('nvram', 'ready' if facts['nvram'] is not None else 'explicit-action', 'Firmware entries must be readable before any boot mutation')
    check('initramfs', 'ready' if 'mkinitcpio' in facts['initramfs'] and 'dracut' not in facts['initramfs'] else 'explicit-action', 'Use the supported Limine/mkinitcpio workflow; dracut is not converted automatically')
    check('uki', 'explicit-action' if facts['limine'].get('ENABLE_UKI', 'no') == 'yes' else 'ready', 'Limine UKIs must remain disabled for EAC')
    check('limine-fallback', 'explicit-action' if facts['limine'].get('ENABLE_LIMINE_FALLBACK') == 'yes' else 'ready', 'Explicit fallback replacement is enabled; preserve other OS fallback loaders before proceeding')
    try:
        kernel_options(facts['cmdline'])
        check('root-parameters', 'ready', 'Root, subvolume and encryption options retained')
    except Checkpoint as error:
        check('root-parameters', 'explicit-action', str(error))
    packages = facts['packages']
    settings = packages.get('cachyos-settings')
    settings_ok = settings and command('/usr/bin/vercmp', settings, MIN_SETTINGS).stdout.strip() in ('0', '1')
    check('upstream-tools', 'ready' if settings_ok else 'repairable', 'cachyos-settings >= ' + MIN_SETTINGS + ' required; installed upstream enrollment behavior will be inspected')
    additions, replacements, blockers = driver_plan(packages, list(facts['kernels'].values()), facts['compute'])
    for issue in blockers:
        if not (allow_dkms and issue.startswith('DKMS module unavailable')):
            check('drivers', 'explicit-action', issue)
    if allow_dkms:
        check('dkms-retained', 'ready', 'Owner explicitly accepts listed unsupported DKMS modules only on conventional/recovery kernels; never compile them for EAC')
    wanted = list(PACKAGES) + additions
    if not settings_ok:
        wanted.append('cachyos-settings')
    missing = [p for p in wanted if p not in packages]
    unavailable = [p for p in wanted if optional('/usr/bin/pacman', '-Si', p) is None]
    check('packages', 'explicit-action' if unavailable else 'repairable' if missing else 'ready', 'Unavailable: ' + ', '.join(unavailable) if unavailable else 'All selected packages have a configured signed repository source')
    if replacements:
        check('driver-transaction', 'repairable', 'Replace ' + ', '.join(replacements) + ' only in the same pacman transaction as all retained-kernel replacements')
    if not facts.get('eac_ca_available', True):
        enrollment = 'needs-ca-package'
    elif facts['setup_mode'] is True:
        enrollment = 'setup-mode'
    elif facts['eac_ca_enrolled'] and facts['keys']['db']['local_enrolled']:
        enrollment = 'enrolled'
    elif facts['keys']['KEK']['local_enrolled'] and Path('/var/lib/sbctl/keys/KEK/KEK.key').is_file():
        enrollment = 'append-db'
    else:
        enrollment = 'setup-mode-required'
    unknown = enrollment != 'needs-ca-package' and facts['setup_mode'] is not True and (facts['eac_ca_enrolled'] is None or (not facts.get('privileged_discovery', False) and facts['keys']['db']['local_enrolled'] is None))
    check('enrollment', 'repairable' if enrollment == 'needs-ca-package' else 'explicit-action' if unknown else 'firmware' if enrollment == 'setup-mode-required' else 'ready',
          'Certificate ownership is unreadable; use privileged preflight.' if unknown else enrollment)
    if facts['secure_boot'] is None or facts['setup_mode'] is None:
        check('secure-boot-state', 'explicit-action', 'Secure Boot and Setup Mode must be readable; run privileged preflight')
    return {'schema': 1, 'profile': 'eac', 'facts': facts, 'checks': checks, 'packages': sorted(set(wanted)),
            'missing': missing, 'driver_replacements': replacements, 'enrollment': enrollment,
            'blocked': any(c['category'] in ('unsupported', 'explicit-action') for c in checks)}


class Transaction:
    def __init__(self, wait=False):
        if os.geteuid() != 0:
            raise RuntimeError('Use scripts/boot-integrate for administrator authentication')
        protected(Path(__file__).resolve())
        safe_parents(Path(__file__).resolve())
        safe_parents(STATE / 'journal.json')
        STATE.mkdir(mode=0o700, parents=True, exist_ok=True)
        protected(STATE, True)
        self.journal_path = STATE / 'journal.json'
        lock_path = STATE / 'operation.lock'
        if lock_path.exists() or lock_path.is_symlink():
            protected(lock_path)
        self.lock = os.fdopen(os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600), 'a')
        protected(lock_path)
        try:
            fcntl.flock(self.lock, fcntl.LOCK_EX | (0 if wait else fcntl.LOCK_NB))
        except BaseException:
            self.lock.close()
            raise
        try:
            os.chmod(STATE / 'operation.lock', 0o600)
            if self.journal_path.exists() or self.journal_path.is_symlink():
                protected(self.journal_path)
            if self.journal_path.exists() and self.journal_path.stat().st_size > LIMIT:
                raise RuntimeError('Oversized boot journal; preserved')
            self.data = json.loads(self.journal_path.read_text() if self.journal_path.exists() else '{}')
            if not isinstance(self.data, dict):
                raise RuntimeError('Invalid boot journal object; preserved')
            if self.data and self.data.get('version') != 1:
                raise RuntimeError('Unsupported boot journal; preserved')
        except BaseException:
            self.lock.close()
            raise

    def __enter__(self):
        return self

    def __exit__(self, kind, error, traceback):
        try:
            if error is not None and self.data:
                self.data['error'] = str(error) or kind.__name__
                self.save()
        finally:
            self.lock.close()
        return False

    def save(self):
        atomic(self.journal_path, json.dumps(self.data, indent=2) + '\n')

    def phase(self, name, operation):
        self.data['phase'] = name
        self.data['pending'] = name
        self.data['error'] = None
        self.save()
        try:
            operation()
        except BaseException as error:
            self.data['error'] = str(error)
            self.save()
            raise
        self.data.setdefault('completed', [])
        if name not in self.data['completed']:
            self.data['completed'].append(name)
        self.data['pending'] = None
        self.save()

    def backup(self, path):
        path = Path(path)
        files = self.data.setdefault('files', {})
        name = str(path)
        if name in files:
            entry = files[name]
            if entry['existed']:
                backup = Path(entry['backup'])
                protected(backup)
                if backup.parent != STATE / 'originals' or digest(backup) != entry['original']:
                    raise RuntimeError('Original boot backup changed: ' + name)
            return
        if path.is_symlink():
            raise Checkpoint('Refusing to back up a symlink boot target: ' + name)
        entry = {'existed': path.is_file(), 'original': digest(path) if path.is_file() else None}
        if entry['existed']:
            backup = STATE / 'originals' / hashlib.sha256(name.encode()).hexdigest()
            backup.parent.mkdir(mode=0o700, exist_ok=True)
            safe_parents(backup)
            if backup.exists() or backup.is_symlink():
                raise Checkpoint('Unjournaled original backup exists; inspect before resuming: ' + str(backup))
            with path.open('rb') as source, backup.open('xb') as target:
                shutil.copyfileobj(source, target)
                target.flush()
                os.fsync(target.fileno())
            os.chmod(backup, 0o600)
            with backup.open('rb') as stream:
                os.fsync(stream.fileno())
            entry['backup'] = str(backup)
        files[name] = entry
        self.save()

    def prepare(self, p, allow_dkms=False):
        f = p['facts']
        if p['blocked']:
            raise Checkpoint('; '.join(c['message'] for c in p['checks'] if c['category'] in ('unsupported', 'explicit-action')))
        identity = {'root_uuid': f['root']['uuid'], 'esp_partuuid': f['esp']['partuuid']}
        if self.data and self.data.get('identity') != identity:
            raise Checkpoint('Root/ESP identity differs from the saved transaction; inspect recovery journal')
        if not self.data:
            self.data = {'version': 1, 'profile': 'eac', 'identity': identity, 'original_nvram': f['nvram'], 'original_packages': f['packages'],
                         'completed': [], 'phase': 'preflight', 'allow_dkms': allow_dkms,
                         'verified_boot': None, 'recovery_acceptance': {}, 'default': 'temporary'}
            self.save()
        # Preserve enrolled PUBLIC variables, never the private signing keys.
        # Firmware setup remains manual; these are evidence/recovery material,
        # not permission to replay or clear a firmware key database.
        for name in ('PK', 'KEK', 'db'):
            self.backup(EFI / (name + '-' + (IMAGE_SECURITY if name == 'db' else GLOBAL)))
        esp = Path(f['esp']['target'])
        protected(esp, True)
        safe_parents(esp)
        for path in [Path('/etc/default/limine'), esp / 'limine.conf', esp / 'loader/loader.conf',
                     esp / 'loader/entries/cachyos-eac.conf', esp / EAC_PATH.lstrip('\\').replace('\\', '/')]:
            self.backup(path)
        machine = read('/etc/machine-id', '')
        if not re.fullmatch(r'[0-9a-f]{32}', machine):
            raise Checkpoint('Invalid machine ID')
        image_paths = [esp / machine / package / name for package in sorted(set(f['kernels'].values()) | {'linux-cachyos-eac', 'linux-lts'})
                       for name in ('vmlinuz', 'initramfs')]
        needed = sum(path.stat().st_size for path in image_paths if path.is_file() and str(path) not in self.data.get('files', {}))
        if shutil.disk_usage(STATE).free < needed + 256 * 1024**2:
            raise Checkpoint('Insufficient root space to preserve original ESP recovery assets')
        for path in image_paths:
            self.backup(path)
        for path in (esp / 'EFI').glob('**/*'):
            if path.is_file() and path.suffix.lower() == '.efi' and ('limine' in str(path).lower() or path.name.lower() == 'bootx64.efi'):
                self.backup(path)

    def checkpoint(self, name, message):
        self.data['phase'] = name
        self.data['checkpoint'] = message
        self.save()
        error = Checkpoint(message)
        error.desktop_safe = name in {'firmware-prerequisites', 'firmware-setup', 'firmware-enable', 'boot-test', 'boot-selection-failed', 'eac-verification-failed', 'recovery-keys', 'existing-bootnext'}
        raise error


def verify_upstream():
    path = Path('/usr/bin/eac-setup')
    protected(path)
    source = read(path, '')
    # Refuse older, non-idempotent or changed enrollment contracts. No bespoke
    # certificate-enrollment replacement is maintained by Foundation.
    for token in ('enrolled_in_db', '--microsoft --custom', 'asus|gigabyte', 'protect_cachyos_files', 'EAC_BOOT_MANAGER'):
        if token not in source:
            raise Checkpoint('Installed eac-setup contract needs review: missing ' + token)
    for option in EAC_OPTIONS:
        if option not in source:
            raise Checkpoint('Upstream EAC required options changed; review before configuring boot')


def signatures(facts):
    esp = Path(facts['esp']['target'])
    machine = read('/etc/machine-id', '')
    if not re.fullmatch(r'[0-9a-f]{32}', machine):
        raise RuntimeError('Invalid machine ID')
    kernel = esp / machine / 'linux-cachyos-eac/vmlinuz'
    initrd = esp / machine / 'linux-cachyos-eac/initramfs'
    loader = esp / EAC_PATH.lstrip('\\').replace('\\', '/')
    for path in [kernel, loader]:
        command('/usr/bin/sbverify', '--cert', str(CA), str(path))
        listing = command('/usr/bin/sbverify', '--list', str(path)).stdout
        if len(re.findall(r'^signature\s', listing, re.M)) != 1:
            raise Checkpoint('CachyOS image has multiple or missing signatures; run repair through upstream eac-setup')
    if not initrd.is_file() or initrd.stat().st_size == 0:
        raise Checkpoint('EAC initramfs missing or empty')
    database = command('/usr/bin/sbctl', 'list-files').stdout
    for line in database.splitlines():
        if line.startswith('/') and ('cachyos-eac' in line or '/systemd/boot/efi/systemd-bootx64.efi' in line):
            raise Checkpoint('CachyOS image is in local signing database; use upstream repair')
    return kernel, initrd, loader


def validate_modules(facts):
    modules = []
    if any('nvidia' in p for p in facts['packages']):
        modules += ['nvidia', 'nvidia_modeset', 'nvidia_drm', 'nvidia_uvm']
    if 'zfs-utils' in facts['packages']:
        modules.append('zfs')
    if any('r8125' in p for p in facts['packages']):
        modules.append('r8125')
    if not {'linux-cachyos-eac', 'linux-lts'}.issubset(set(facts['kernels'].values())):
        raise Checkpoint('Both EAC and LTS kernel installations must be discoverable')
    for release, package in facts['kernels'].items():
        if package not in ('linux-cachyos-eac', 'linux-lts'):
            continue
        # Limine uses machine-ID image directories, not necessarily /boot links.
        initrd = Path(facts['esp']['target']) / read('/etc/machine-id', '') / package / 'initramfs'
        if not initrd.is_file() or not initrd.stat().st_size:
            raise Checkpoint('Recovery/EAC initramfs missing for ' + package)
        command('/usr/bin/lsinitcpio', str(initrd))
        for module in modules:
            filename = command('/usr/bin/modinfo', '-k', release, '-F', 'filename', module).stdout.strip()
            owner = command('/usr/bin/pacman', '-Qoq', filename).stdout.strip()
            signer = command('/usr/bin/modinfo', '-k', release, '-F', 'signer', module).stdout.strip()
            magic = command('/usr/bin/modinfo', '-k', release, '-F', 'vermagic', module).stdout
            if not signer or not magic.startswith(release + ' '):
                raise Checkpoint('Unsigned or mismatched ' + module + ' for ' + release)
            if package == 'linux-cachyos-eac' and not owner.startswith('linux-cachyos-eac-'):
                raise Checkpoint('EAC module is not an upstream prebuilt package: ' + owner)


def boot_entries(facts, timeout=0):
    kernel, initrd, _ = signatures(facts)
    esp = Path(facts['esp']['target'])
    content = ('title CachyOS (EAC)\nlinux /' + str(kernel.relative_to(esp)) + '\ninitrd /'
               + str(initrd.relative_to(esp)) + '\noptions ' + kernel_options(facts['cmdline']) + '\n')
    atomic(esp / 'loader/entries/cachyos-eac.conf', content, 0o644)
    existing = read(esp / 'loader/loader.conf', '')
    retained = [line for line in existing.splitlines() if not re.match(r'^\s*(default|timeout)\s+', line)]
    atomic(esp / 'loader/loader.conf', '\n'.join(retained + ['default cachyos-eac.conf', f'timeout {timeout}']) + '\n', 0o644)


def ensure_entries(facts):
    esp = facts['esp']
    nvram = parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)
    for label, path, is_limine in [('CachyOS EAC', EAC_PATH, False), ('Limine', r'\EFI\limine\limine_x64.efi', True)]:
        if resolve_entry(nvram, esp['partuuid'], path, is_limine) is None:
            image = Path(esp['target']) / path.lstrip('\\').replace('\\', '/')
            if not image.is_file():
                raise Checkpoint('EFI image unavailable: ' + str(image))
            command('/usr/bin/efibootmgr', '--create-only', '--disk', esp['disk'], '--part', esp['partition'], '--label', label, '--loader', path)
            after = parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)
            if after['order'] != nvram['order']:
                raise Checkpoint('Firmware changed BootOrder during create-only; original order is journaled')
            nvram = after
            if resolve_entry(nvram, esp['partuuid'], path, is_limine) is None:
                raise RuntimeError('Firmware entry creation did not read back')
    return nvram


def recovery_setup(tx, facts):
    config = Path('/etc/default/limine')
    if not config.exists():
        atomic(config, 'ESP_PATH="' + facts['esp']['target'] + '"\nENABLE_UKI=no\nENABLE_LIMINE_FALLBACK=no\nFIND_BOOTLOADERS=yes\n', 0o644)
    if not (Path(facts['esp']['target']) / 'limine.conf').is_file():
        command('/usr/bin/limine-install', '--no-efi-register', interactive=True)
    roots = [name for name, cfg in facts['snapper'].items() if cfg.get('SUBVOLUME') == '/']
    if len(roots) > 1:
        raise Checkpoint('Multiple Snapper root configurations; choose one explicitly')
    if not roots:
        if Path('/etc/snapper/configs/root').exists() or Path('/.snapshots').exists():
            raise Checkpoint('Existing snapshot layout has no matching Snapper root config; preserve and configure it explicitly')
        command('/usr/bin/snapper', '-c', 'root', 'create-config', '/', interactive=True)
    command('/usr/bin/systemctl', 'enable', '--now', 'limine-snapper-sync.service')
    command('/usr/bin/systemctl', 'enable', '--now', 'snapper-cleanup.timer')
    command('/usr/bin/limine-mkinitcpio', 'linux-lts', 'linux-cachyos-eac', interactive=True)
    if not (Path(facts['esp']['target']) / 'limine.conf').is_file():
        command('/usr/bin/limine-scan', interactive=True)
    if not (Path(facts['esp']['target']) / 'limine.conf').is_file():
        raise Checkpoint('Upstream tools did not generate a Limine configuration')
    command('/usr/bin/limine-snapper-sync', interactive=True)


def sign_lts(facts):
    """Use sbctl for conventional recovery images only, never EAC files."""
    esp = Path(facts['esp']['target'])
    images = [Path('/usr/lib/modules') / release / 'vmlinuz'
              for release, package in facts['kernels'].items() if package == 'linux-lts']
    images.append(esp / read('/etc/machine-id', '') / 'linux-lts/vmlinuz')
    for image in images:
        if not image.is_file():
            raise Checkpoint('LTS image missing: ' + str(image))
        protected(image)
        # --save tracks only stable LTS paths; snapshot copies belong to upstream
        # Limine retention, not an ever-growing sbctl file database.
        command('/usr/bin/sbctl', 'sign', '--save', str(image), timeout=120)
        command('/usr/bin/sbverify', '--cert', str(OWNER_CERT), str(image))
    command('/usr/bin/limine-snapper-sync', interactive=True)
    command('/usr/bin/limine-enroll-config', interactive=True)


def create_recovery_snapshot(tx, facts):
    if tx.data.get('recovery_snapshot'):
        return
    roots = [name for name, cfg in facts['snapper'].items() if cfg.get('SUBVOLUME') == '/']
    if len(roots) != 1:
        raise Checkpoint('Exactly one Snapper root configuration is required')
    identifier = command('/usr/bin/snapper', '-c', roots[0], 'create', '--type', 'single',
                         '--cleanup-algorithm', 'number', '--description', 'Desktop Foundation boot recovery',
                         '--print-number', timeout=120).stdout.strip()
    if not identifier.isdigit() or identifier == '0':
        raise Checkpoint('Recovery snapshot creation did not return a valid number')
    tx.data['recovery_snapshot'] = {'config': roots[0], 'number': identifier}
    tx.save()
    command('/usr/bin/limine-snapper-sync', interactive=True)


def install_hooks(tx):
    unit = Path('/etc/systemd/system/desktop-foundation-boot-refresh.service')
    hook = Path('/etc/pacman.d/hooks/zzzz-desktop-foundation-boot.hook')
    entries = {
        unit: '[Unit]\nDescription=Verify Desktop Foundation EAC and recovery boot files\nConditionPathExists=/var/lib/desktop-foundation/boot/journal.json\n\n[Service]\nType=oneshot\nExecStart=/usr/bin/python3 -I /usr/local/libexec/desktop-foundation-boot refresh\nTimeoutStartSec=300\nUMask=0077\nPrivateTmp=yes\n',
        hook: '[Trigger]\nOperation=Install\nOperation=Upgrade\nType=Package\nTarget=linux*\nTarget=limine*\nTarget=systemd-boot-cachyos-signed\nTarget=cachyos-eac-signing-key\nTarget=sbctl\n\n[Action]\nDescription=Scheduling EAC and Limine recovery verification...\nWhen=PostTransaction\nExec=/usr/bin/systemctl start --no-block desktop-foundation-boot-refresh.service\n',
    }
    for path, content in entries.items():
        tx.backup(path)
        previous = tx.data.setdefault('managed_hooks', {}).get(str(path))
        if path.exists() and (not previous or digest(path) != previous):
            raise Checkpoint('Existing update integration requires review: ' + str(path))
        atomic(path, content, 0o644)
        tx.data['managed_hooks'][str(path)] = digest(path)
        tx.save()
    command('/usr/bin/systemctl', 'daemon-reload')


def recovery_verify(facts):
    root = [name for name, cfg in facts['snapper'].items() if cfg.get('SUBVOLUME') == '/']
    if len(root) != 1:
        raise Checkpoint('Exactly one Snapper root configuration is required')
    command('/usr/bin/snapper', '-c', root[0], 'list')
    command('/usr/bin/systemctl', 'is-active', '--quiet', 'limine-snapper-sync.service')
    command('/usr/bin/systemctl', 'is-enabled', '--quiet', 'limine-snapper-sync.service')
    if not any('snap-pac' in p.name for p in Path('/usr/share/libalpm/hooks').glob('*')):
        raise Checkpoint('snap-pac package-manager snapshot hooks missing')
    esp = Path(facts['esp']['target'])
    text = read(esp / 'limine.conf', '')
    if 'linux-lts' not in text.lower():
        raise Checkpoint('Limine LTS recovery entry missing')
    loaders = [p for p in (esp / 'EFI').glob('**/*') if p.is_file() and p.suffix.lower() == '.efi' and 'limine' in p.name.lower()]
    if not loaders:
        raise Checkpoint('Independent Limine EFI executable missing')
    for path in loaders:
        command('/usr/bin/sbverify', '--cert', str(OWNER_CERT), str(path))
    kernel = esp / read('/etc/machine-id', '') / 'linux-lts/vmlinuz'
    command('/usr/bin/sbverify', '--cert', str(OWNER_CERT), str(kernel))
    # Snapshot copies are separate ESP assets. Verify referenced files, not just
    # the Btrfs snapshot directories; root snapshots do not roll the ESP back.
    missing = []
    for relative in re.findall(r'boot\(\):([^\s#]+)', text):
        relative = relative.split('#', 1)[0]
        target = esp / relative.lstrip('/')
        if '..' in Path(relative).parts or not target.is_file():
            missing.append(relative)
        if target.is_file() and target.name.startswith('vmlinuz'):
            upstream = command('/usr/bin/sbverify', '--cert', str(CA), str(target), required=False)
            if upstream.returncode:
                command('/usr/bin/sbverify', '--cert', str(OWNER_CERT), str(target))
    if missing:
        raise Checkpoint('Limine references missing ESP recovery assets: ' + ', '.join(missing[:8]))


def run_upstream(tx, facts, recovered_keys):
    verify_upstream()
    enrollment_needed = not (facts['eac_ca_enrolled'] is True and facts['keys']['db']['local_enrolled'] is True)
    if enrollment_needed and (facts['encryption']['tpm_bound'] or facts['encryption']['bitlocker_possible']):
        if not recovered_keys and not tx.data.get('recovery_keys_confirmed'):
            tx.checkpoint('recovery-keys', 'Confirm possession of BitLocker/TPM-bound LUKS recovery credentials with --recovery-keys-confirmed before enrollment')
        tx.data['recovery_keys_confirmed'] = True
        tx.save()
    # Upstream compares DER identities before appending db. Never call sbctl's
    # non-idempotent partial append independently or use -y for enrollment.
    command('/usr/bin/eac-setup', interactive=True, boot_manager=True)
    after = scan()
    if facts['setup_mode'] is not True and any(facts['keys'][name]['fingerprints'] != after['keys'][name]['fingerprints'] for name in ('PK', 'KEK')):
        tx.checkpoint('certificate-change', 'PK/KEK changed unexpectedly during db enrollment; stop and inspect firmware state')
    if after['eac_ca_enrolled'] is not True or after['keys']['db']['local_enrolled'] is not True:
        tx.checkpoint('enrollment', 'Firmware certificate readback did not verify; do not repeat enrollment blindly')
    return after


def verify_running_image(facts):
    if facts.get('kernels', {}).get(facts['kernel']) != 'linux-cachyos-eac':
        raise Checkpoint('Running EAC kernel differs from installed kernel; a fresh boot is required')
    kernel, _, _ = signatures(facts)
    source = Path('/usr/lib/modules') / facts['kernel'] / 'vmlinuz'
    if not source.is_file() or digest(source) != digest(kernel):
        raise Checkpoint('Selected ESP kernel does not match the running installed EAC image')


def verified_boot(facts):
    n = facts['nvram']
    if not n or not facts['esp']:
        raise Checkpoint('Firmware entry and ESP identity unavailable')
    eac = resolve_entry(n, facts['esp']['partuuid'], EAC_PATH)
    status = command('/usr/bin/bootctl', 'status', '--no-pager', required=False).stdout
    if not eac or not facts['kernel'].endswith('-cachyos-eac') or n['current'] != eac:
        raise Checkpoint('Boot files ready; boot the dedicated EAC firmware entry before verification')
    if normalize_efi(EAC_PATH) not in normalize_efi(status) or not re.search(r'Product:\s+systemd-boot\b', status) or not re.search(r'Current Entry:\s+cachyos-eac.conf\b', status):
        raise Checkpoint('Running bootloader/entry is not dedicated EAC systemd-boot')
    verify_running_image(facts)
    result = command('/usr/bin/eac-status', required=False, timeout=120)
    if result.returncode or '[FAIL]' in result.stdout or '[WARN]' in result.stdout or 'All checks passed.' not in result.stdout:
        raise Checkpoint('Authoritative eac-status did not fully pass:\n' + result.stdout[-5000:])
    validate_modules(facts)
    return result.stdout


def select_boot(tx, facts, method):
    if tx.data.get('attempt'):
        attempt = tx.data['attempt']
        if attempt['boot_id'] == facts['boot_id']:
            tx.checkpoint('boot-test', 'Boot selection already prepared; reboot manually, then resume')
        # A consumed BootNext is not evidence of success or failure. Verify
        # BootCurrent and actual executable/entry after a changed boot ID.
        expected = resolve_entry(facts['nvram'], facts['esp']['partuuid'], EAC_PATH)
        if facts['nvram']['current'] != expected:
            tx.checkpoint('boot-selection-failed', 'Firmware did not start the dedicated EAC entry. Use select-order once for compatibility, or restore-order; no automatic reboot loop')
        return
    n = ensure_entries(facts)
    eac = resolve_entry(n, facts['esp']['partuuid'], EAC_PATH)
    limine = resolve_entry(n, facts['esp']['partuuid'], limine=True)
    if n.get('next') and n['next'] != eac:
        tx.checkpoint('existing-bootnext', 'An unrelated BootNext is pending; preserve and complete that firmware action before the EAC boot test')
    tx.data['attempt'] = {'boot_id': facts['boot_id'], 'method': method, 'before': n, 'entry': n['entries'][eac]}
    tx.save()
    if method == 'next':
        command('/usr/bin/efibootmgr', '--bootnext', eac)
        after = parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)
        if after['next'] != eac:
            raise RuntimeError('BootNext did not read back')
    else:
        order = selected_order(n, eac, limine)
        tx.data['attempt']['applied_order'] = order
        tx.save()
        command('/usr/bin/efibootmgr', '--bootorder', ','.join(order))
        if parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)['order'] != order:
            raise RuntimeError('BootOrder did not read back')
    tx.checkpoint('boot-test', 'Boot selection prepared and read back. Reboot manually, then run the same installer/resume command')


def restore_order(tx):
    before = tx.data.get('attempt', {}).get('before') or tx.data.get('original_nvram')
    if not before:
        raise Checkpoint('No original firmware order in the transaction')
    now = parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)
    order = restored_order(before, now)
    if not order:
        raise Checkpoint('No original firmware entries remain; journal preserved')
    command('/usr/bin/efibootmgr', '--bootorder', ','.join(order))
    if parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)['order'] != order:
        raise RuntimeError('Original firmware order restore did not read back')
    # Do not remove an unrelated BootNext created since our diagnostic attempt.
    attempt = tx.data.get('attempt', {})
    expected = resolve_entry(now, tx.data['identity']['esp_partuuid'], EAC_PATH)
    if attempt and now['next'] == expected:
        previous = before.get('next')
        identity = before['entries'].get(previous)
        restored = [number for number, entry in now['entries'].items() if identity and entry['path'] == identity['path'] and entry['partuuid'] == identity['partuuid']]
        if len(restored) > 1:
            raise Checkpoint('Original BootNext identity is ambiguous; inspect journal')
        if restored:
            command('/usr/bin/efibootmgr', '--bootnext', restored[0])
        else:
            command('/usr/bin/efibootmgr', '--delete-bootnext')
        actual = parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)['next']
        if actual != (restored[0] if restored else None):
            raise Checkpoint('Original BootNext did not read back')
    tx.data.pop('attempt', None)
    tx.data['default'] = 'original'
    tx.save()


def resume(tx, args):
    f = scan()
    p = plan(f, args.allow_recovery_only_dkms or tx.data.get('allow_dkms', False))
    tx.prepare(p, args.allow_recovery_only_dkms)
    tx.data['timeout'] = args.timeout
    tx.save()
    firmware = [c['message'] for c in p['checks'] if c['category'] == 'firmware' and c['name'] != 'enrollment']
    if firmware:
        tx.checkpoint('firmware-prerequisites', '; '.join(firmware) + '; resume after configuring firmware')
    if 'packages' not in tx.data['completed'] or p['missing'] or p['driver_replacements'] or tx.data.get('pending') == 'packages':
        if args.no_packages and p['missing']:
            tx.checkpoint('packages', 'Required packages missing with --no-packages: ' + ', '.join(p['missing']))
        def packages():
            if not args.no_packages:
                # Conflict removals are reviewed in this one solver transaction;
                # never pacman -R a working DKMS driver before replacements exist.
                command('/usr/bin/pacman', '-Syu', '--needed', *p['packages'], interactive=True)
            after = scan()
            if any(name not in after['packages'] for name in p['packages']):
                raise Checkpoint('Package readback is incomplete; resume the package transaction')
            if any(name in after['packages'] for name in p['driver_replacements']):
                raise Checkpoint('Conflicting DKMS driver remains; package transaction not accepted')
            tx.data['prepared_packages'] = {name: after['packages'].get(name) for name in p['packages']}
        tx.phase('packages', packages)
        f = scan()
    p = plan(f, args.allow_recovery_only_dkms or tx.data.get('allow_dkms', False))
    if p['blocked']:
        tx.checkpoint('post-package-preflight', '; '.join(item['message'] for item in p['checks'] if item['category'] in ('unsupported', 'explicit-action')))
    if p['enrollment'] == 'setup-mode-required':
        vendor = f['board_vendor'].lower()
        hint = ('ASUS: review Secure Boot Key Management.' if 'asus' in vendor else 'Review the vendor Custom/Setup Mode procedure.') + ' Existing signing-key ownership is not established. Preserve the exported public variables and recovery credentials; do not clear PK/KEK blindly.'
        tx.checkpoint('firmware-setup', hint + ' Resume only after the deliberate firmware recovery/enrollment procedure. No automatic reboot.')
    verify_upstream()
    if 'recovery-preparation' not in tx.data['completed']:
        tx.phase('recovery-preparation', lambda: recovery_setup(tx, f))
        f = scan()
    # Reuse verified boot files. Rerun upstream on explicit repair, changed
    # package versions, missing files or incomplete certificate enrollment only.
    needs_upstream = 'upstream' not in tx.data['completed'] or args.action == 'repair'
    if f['eac_ca_enrolled'] is not True or f['keys']['db']['local_enrolled'] is not True:
        needs_upstream = True
    if not needs_upstream:
        try:
            signatures(f)
        except (RuntimeError, Checkpoint):
            needs_upstream = True
    if needs_upstream:
        tx.phase('upstream', lambda: run_upstream(tx, f, args.recovery_keys_confirmed))
        f = scan()
    tx.phase('lts-signing', lambda: sign_lts(f))
    tx.phase('recovery-snapshot', lambda: create_recovery_snapshot(tx, f))
    tx.phase('update-hooks', lambda: install_hooks(tx))
    tx.phase('boot-files', lambda: (validate_modules(f), boot_entries(f, args.timeout), ensure_entries(f)))
    command('/usr/bin/limine-enroll-config', interactive=True)
    tx.phase('recovery-files', lambda: recovery_verify(f))
    if f['secure_boot'] is not True:
        tx.checkpoint('firmware-enable', 'Signed EAC and recovery boot files must be verified before enforcement. Enable Secure Boot in firmware (ASUS: Boot → Secure Boot → OS Type → Windows UEFI Mode), reboot and resume')
    try:
        proof = verified_boot(f)
    except Checkpoint as error:
        if f['kernel'].endswith('-cachyos-eac') and resolve_entry(f['nvram'], f['esp']['partuuid'], EAC_PATH) == f['nvram']['current']:
            tx.checkpoint('eac-verification-failed', str(error))
        if tx.data.get('attempt'):
            select_boot(tx, f, tx.data['attempt']['method'])
            raise
        select_boot(tx, f, 'next')
        return
    tx.data['verified_boot'] = {'boot_id': f['boot_id'], 'kernel': f['kernel'], 'nvram': f['nvram'], 'eac_status': proof}
    tx.data['phase'] = 'verified-boot'
    tx.data['checkpoint'] = None
    tx.save()
    if tx.data.get('attempt', {}).get('method') == 'order':
        restore_order(tx)
    # Daily selection is a separate explicit command after this proof.
    print('Verified EAC boot passed. Use finalize for EAC daily default, or restore-order for a temporary diagnostic.')


def recovery_acceptance(facts, tx, kind=None):
    path = Path(facts['esp']['target']) / 'desktop-foundation/recovery-acceptance.json'
    if path.exists() or path.is_symlink():
        protected(path)
        value = json.loads(read(path, '{}'))
        if value.get('identity') != tx.data['identity']:
            raise Checkpoint('ESP recovery acceptance belongs to a different root/ESP')
    else:
        value = {'identity': tx.data['identity'], 'proofs': {}}
    proofs = value.get('proofs', {})
    if not isinstance(proofs, dict) or any(key not in ('lts', 'snapshot') or not isinstance(proof, dict)
            or not isinstance(proof.get('boot_id'), str) or not isinstance(proof.get('kernel'), str)
            for key, proof in proofs.items()):
        raise Checkpoint('Invalid recovery acceptance record; preserve and inspect')
    if kind:
        proofs[kind] = {'boot_id': facts['boot_id'], 'kernel': facts['kernel'], 'cmdline': facts['cmdline']}
        value['proofs'] = proofs
        atomic(path, json.dumps(value, indent=2) + '\n')
    tx.data.setdefault('recovery_acceptance', {}).update(proofs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['preflight', 'plan', 'status', 'resume', 'repair', 'verify', 'select-next', 'select-order', 'restore-order', 'finalize', 'refresh', 'accept-lts', 'accept-snapshot'])
    parser.add_argument('--desktop', action='store_true', help='Allow safe firmware checkpoints after desktop deployment')
    parser.add_argument('--summary', action='store_true')
    parser.add_argument('--no-packages', action='store_true')
    parser.add_argument('--recovery-keys-confirmed', action='store_true')
    parser.add_argument('--allow-recovery-only-dkms', action='store_true')
    parser.add_argument('--timeout', type=int, default=None, choices=range(0, 31))
    args = parser.parse_args()
    if args.action in ('preflight', 'plan'):
        saved = read(STATE / 'journal.json')
        consent = False
        if saved is not None:
            protected(STATE / 'journal.json')
            consent = json.loads(saved).get('allow_dkms') is True
        value = plan(scan(), args.allow_recovery_only_dkms or consent)
        if args.summary:
            for item in value['checks']:
                if item['category'] != 'ready':
                    print(item['category'] + ': ' + item['message'])
            print('Boot preflight: ' + ('needs action' if value['blocked'] else 'ready to prepare'))
        else:
            print(json.dumps(value, indent=2))
        return 2 if value['blocked'] else 0
    if args.action == 'status':
        journal = read(STATE / 'journal.json')
        print(json.dumps({'discovery': scan(), 'transaction': json.loads(journal) if journal is not None else None, 'journal_note': 'Run privileged status if the journal is not readable'}, indent=2))
        return 0
    with Transaction(wait=args.action == 'refresh') as tx:
        if args.timeout is None:
            args.timeout = tx.data.get('timeout', 0)
        if args.action in ('resume', 'repair'):
            resume(tx, args)
        elif not tx.data:
            raise Checkpoint('No managed boot transaction; run resume first')
        else:
            f = scan()
            root = f['backing_root'] if args.action == 'accept-snapshot' and f.get('backing_root') else f['root']
            if not f['esp'] or not root or {'esp_partuuid': f['esp']['partuuid'], 'root_uuid': root['uuid']} != tx.data['identity']:
                raise Checkpoint('Root/ESP identity changed')
            if args.action == 'restore-order':
                restore_order(tx)
            elif args.action in ('select-next', 'select-order'):
                signatures(f)
                recovery_verify(f)
                if args.action == 'select-order' and tx.data.get('phase') == 'boot-selection-failed':
                    tx.data.pop('attempt', None)  # explicit compatibility attempt
                    tx.save()
                select_boot(tx, f, args.action.split('-')[1])
            elif args.action in ('verify', 'finalize'):
                proof = verified_boot(f)
                recovery_verify(f)
                tx.data['verified_boot'] = {'boot_id': f['boot_id'], 'kernel': f['kernel'], 'eac_status': proof}
                tx.data['checkpoint'] = None
                tx.data['error'] = None
                if args.action == 'finalize':
                    n = ensure_entries(f)
                    eac = resolve_entry(n, f['esp']['partuuid'], EAC_PATH)
                    limine = resolve_entry(n, f['esp']['partuuid'], limine=True)
                    order = selected_order(n, eac, limine)
                    tx.data['pending_order'] = order
                    tx.save()
                    command('/usr/bin/efibootmgr', '--bootorder', ','.join(order))
                    if parse_nvram(command('/usr/bin/efibootmgr', '-v').stdout)['order'] != order:
                        raise RuntimeError('Daily BootOrder did not read back')
                    tx.data['default'] = 'eac'
                recovery_acceptance(f, tx)
                tx.data['installation_complete'] = tx.data.get('default') == 'eac'
                tx.data['recovery_verified'] = bool(tx.data.get('recovery_acceptance', {}).get('lts') and tx.data.get('recovery_acceptance', {}).get('snapshot'))
                tx.data['phase'] = 'installed-recovery-verified' if tx.data['installation_complete'] and tx.data['recovery_verified'] else 'installed-recovery-unverified' if tx.data['installation_complete'] else 'verified-boot'
                tx.save()
            elif args.action == 'refresh':
                if tx.data.get('pending') == 'packages':
                    tx.checkpoint('package-recovery-required', 'A package transaction was interrupted; run resume before boot-file refresh can succeed')
                # Package hooks do not enroll keys, select boot order or reboot.
                def refresh():
                    signatures(f)
                    validate_modules(f)
                    sign_lts(f)
                    boot_entries(f, args.timeout)
                    for path, limine in [(EAC_PATH, False), (r'\EFI\limine\limine_x64.efi', True)]:
                        if resolve_entry(f['nvram'], f['esp']['partuuid'], path, limine) is None:
                            raise Checkpoint('Managed firmware entry missing; run explicit repair')
                    recovery_verify(scan())
                tx.phase('refresh', refresh)
                tx.data['phase'] = 'boot-files-ready'
                tx.data['verified_boot'] = None
                tx.save()
            elif args.action.startswith('accept-'):
                n = f['nvram']
                current = n['entries'].get(n['current'], {})
                if 'limine' not in current.get('path', ''):
                    raise Checkpoint('Recovery acceptance requires booting through the independent Limine firmware entry')
                kind = args.action.split('-')[1]
                if kind == 'lts' and not f['kernel'].endswith('-lts'):
                    raise Checkpoint('LTS is not the running kernel')
                if kind == 'snapshot' and '.snapshots' not in f['cmdline']:
                    raise Checkpoint('A snapshot is not the running root')
                recovery_acceptance(f, tx, kind)
                tx.save()
        print(json.dumps({'phase': tx.data['phase'], 'default': tx.data.get('default'), 'checkpoint': tx.data.get('checkpoint')}, indent=2))
        return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except Checkpoint as error:
        print('Foundation boot checkpoint: ' + str(error), file=sys.stderr)
        raise SystemExit(75 if '--desktop' not in sys.argv or error.desktop_safe else 1)
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print('Foundation boot: ' + str(error) + '. Journal and recovery files retained.', file=sys.stderr)
        raise SystemExit(1)
