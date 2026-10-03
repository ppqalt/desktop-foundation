"""Deployment-owned session units: no unit generation during login."""
from niri_wallpaper import quote


def definitions(root, state):
    executable = state / 'bin/mako'
    # Use the same installed/native executable preference as the wallpaper launcher.
    import shutil
    executable = shutil.which('mako') or executable
    common = ('[Unit]\nPartOf=desktop-foundation-session.target graphical-session.target\n'
              'ConditionEnvironment=WAYLAND_DISPLAY\nStartLimitIntervalSec=60\nStartLimitBurst=3\n')
    units = {
        'desktop-foundation-session.target': (
            '[Unit]\nDescription=Desktop foundation interactive session\n'
            'PartOf=graphical-session.target\n'
            'Wants=desktop-foundation-shell.service desktop-foundation-notifications.service '
            'desktop-foundation-clipboard-init.service desktop-foundation-clipboard@text.service '
            'desktop-foundation-clipboard@image.service hyprpolkitagent.service\n'),
        'desktop-foundation-shell.service': common + '[Service]\nType=exec\nExecStart=' + quote(root / 'scripts/run-shell') + '\nRestart=on-failure\nRestartSec=1\n',
        'desktop-foundation-clipboard-init.service': common + '[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=' + quote(root / 'scripts/foundation') + ' clipboard init\nUMask=0077\n',
        'desktop-foundation-clipboard@.service': common +
            'Requires=desktop-foundation-clipboard-init.service\nAfter=desktop-foundation-clipboard-init.service\n'
            '[Service]\nType=exec\nExecStart=' + quote(root / 'scripts/clipboard-watch') + ' %i\nRestart=on-failure\nRestartSec=1\nUMask=0077\n',
        'desktop-foundation-notifications.service': common + 'ConditionEnvironment=NIRI_SOCKET\n'
            '[Service]\nType=dbus\nBusName=org.freedesktop.Notifications\nExecCondition=' + quote(root / 'scripts/notification-owner-check') + '\n'
            'ExecStart=' + quote(executable) + ' --config ' + quote(state / 'theme/current/notifications.conf') + '\nRestart=on-failure\nRestartSec=1\n',
    }
    for name, command in {
        'wallpaper': [root / 'scripts/niri-wallpaper-start'],
        'clipboard-persist': [root / 'scripts/clipboard-persist'],
    }.items():
        units['desktop-foundation-' + name + '.service'] = (
            '[Unit]\nDescription=Desktop foundation Niri ' + name + '\n'
            'PartOf=niri.service graphical-session.target\nAfter=niri.service\n'
            'ConditionEnvironment=NIRI_SOCKET\nStartLimitIntervalSec=60\nStartLimitBurst=3\n'
            '[Service]\nExecStart=' + ' '.join(map(quote, command)) + '\n'
            'Restart=on-failure\nRestartSec=1\nUMask=0077\n'
            '[Install]\nWantedBy=niri.service\n')
    return units


def targets(root, config, state, data, write=True, niri=True):
    generated = state / 'session-units'
    if write:
        generated.mkdir(parents=True, exist_ok=True)
    result = [(data / 'wallpapers/wallhaven-135w7w.png', root / 'wallpapers/wallhaven-135w7w.png')] if niri else []
    for unit, content in definitions(root, state).items():
        if not niri and unit in {'desktop-foundation-wallpaper.service', 'desktop-foundation-clipboard-persist.service'}:
            continue
        source = generated / unit
        if write and (not source.exists() or source.read_text() != content):
            temporary = source.with_suffix('.tmp')
            temporary.write_text(content)
            temporary.replace(source)
        result.append((config / 'systemd/user' / unit, source))
        if unit in {'desktop-foundation-wallpaper.service', 'desktop-foundation-clipboard-persist.service'}:
            result.append((config / 'systemd/user/niri.service.wants' / unit, source))
    for name in ['gnome-keyring-pkcs11.desktop', 'gnome-keyring-secrets.desktop']:
        result.append((config / 'autostart' / name, root / 'session/keyring' / name))
    if niri:
        # Mako's packaged D-Bus activation names mako.service. Both that unit and
        # our Type=dbus unit cannot independently register the same BusName.
        # Journal an alias so activation and session startup resolve to one unit.
        result.append((config / 'systemd/user/mako.service',
                       config / 'systemd/user/desktop-foundation-notifications.service'))
    result.append((config / 'autostart/blueman.desktop', root / 'session/bluetooth/blueman.desktop'))
    result.append((config / 'autostart/arch-update-tray.desktop', root / 'session/autostart/arch-update-tray.desktop'))
    result.append((data / 'dbus-1/services/org.freedesktop.secrets.service',
                   root / 'session/keyring/org.freedesktop.secrets.service'))
    return result
