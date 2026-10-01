"""Persistent, deployment-owned Niri session units and activation overrides."""
from niri_wallpaper import quote


def targets(root, config, state, data):
    generated = state / 'session-units'
    generated.mkdir(parents=True, exist_ok=True)
    result = [(data / 'wallpapers/wallhaven-135w7w.png', root / 'wallpapers/wallhaven-135w7w.png')]
    for name, command in {
        'wallpaper': [str(root / 'scripts/niri-wallpaper-start')],
        'clipboard-persist': [str(root / 'scripts/clipboard-persist')],
    }.items():
        unit = 'desktop-foundation-' + name + '.service'
        source = generated / unit
        source.write_text('[Unit]\nDescription=Desktop foundation Niri ' + name + '\n'
                          'PartOf=niri.service graphical-session.target\nAfter=niri.service\n'
                          'ConditionEnvironment=NIRI_SOCKET\nStartLimitIntervalSec=60\nStartLimitBurst=3\n'
                          '[Service]\nExecStart=' + ' '.join(map(quote, command)) + '\n'
                          'Restart=on-failure\nRestartSec=1\nUMask=0077\n'
                          '[Install]\nWantedBy=niri.service\n')
        result.append((config / 'systemd/user' / unit, source))
        result.append((config / 'systemd/user/niri.service.wants' / unit, source))
    for name in ['gnome-keyring-pkcs11.desktop', 'gnome-keyring-secrets.desktop']:
        result.append((config / 'autostart' / name, root / 'session/keyring' / name))
    result.append((config / 'autostart/blueman.desktop', root / 'session/bluetooth/blueman.desktop'))
    result.append((data / 'dbus-1/services/org.freedesktop.secrets.service',
                   root / 'session/keyring/org.freedesktop.secrets.service'))
    return result
