"""Boot policy derivation. No writes or subprocesses on import."""
import gzip
import re
from pathlib import Path


def kernel_lsms(configs):
    lists = []
    for config in configs:
        if not re.search(r'^CONFIG_SECURITY_APPARMOR=y$', config, re.M):
            raise ValueError('Every installed boot kernel must support AppArmor')
        match = re.search(r'^CONFIG_LSM="([a-z_,]+)"$', config, re.M)
        if not match:
            raise ValueError('Kernel LSM configuration unavailable')
        names = match[1].split(',')
        if any(n in names for n in ('selinux', 'smack', 'tomoyo')):
            raise ValueError('Existing major LSM requires an explicit administrator decision')
        if names not in lists:
            lists.append(names)
    if not lists:
        raise ValueError('No installed kernel configurations found')
    if len(lists) != 1:
        raise ValueError('Installed kernels have different LSM defaults; review before modifying boot')
    names = [n for n in lists[0] if n not in ('apparmor', 'capability')]
    names.insert(names.index('bpf') if 'bpf' in names else len(names), 'apparmor')
    return ','.join(names)


def installed_lsms():
    configs = []
    for modules in Path('/usr/lib/modules').iterdir():
        if not (modules / 'pkgbase').is_file():
            continue
        candidates = [modules / 'build/.config', Path('/boot') / ('config-' + modules.name)]
        candidate = next((p for p in candidates if p.is_file()), None)
        if candidate:
            configs.append(candidate.read_text())
        elif modules.name == __import__('os').uname().release and Path('/proc/config.gz').exists():
            with gzip.open('/proc/config.gz', 'rt') as handle:
                configs.append(handle.read())
        else:
            raise ValueError('Kernel config missing for ' + modules.name + '; install matching headers or provide /boot/config-' + modules.name)
    return kernel_lsms(configs)


def fallback_preset(text):
    if not re.search(r'^PRESETS=\([\'\"]default[\'\"](?: [\'\"]fallback[\'\"])?\)', text, re.M):
        raise ValueError('Custom preset list: refusing to replace it')
    text = re.sub(r'^PRESETS=.*$', "PRESETS=('default' 'fallback')", text, flags=re.M)
    for key in ('fallback_image', 'fallback_options'):
        text, count = re.subn(r'^#?(' + key + r'=.*)$', r'\1', text, flags=re.M)
        if count != 1:
            raise ValueError('Missing or ambiguous ' + key)
    if not re.search(r'^fallback_options=[\'\"]-S autodetect[\'\"]$', text, re.M):
        raise ValueError('Custom fallback options need review')
    return text


def remove_plymouth(text):
    matches = list(re.finditer(r'^HOOKS=\(([^\n]*)\)$', text, re.M))
    if len(matches) != 1:
        raise ValueError('Custom mkinitcpio HOOKS syntax needs review')
    match = matches[0]
    hooks = match[1].split()
    if any(not re.fullmatch(r'[a-zA-Z0-9_-]+', h) for h in hooks):
        raise ValueError('Quoted or computed hooks need review')
    return text[:match.start()] + 'HOOKS=(' + ' '.join(h for h in hooks if h != 'plymouth') + ')' + text[match.end():]
