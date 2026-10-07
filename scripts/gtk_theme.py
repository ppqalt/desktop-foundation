"""Compatibility bridge to native GNOME accent policy and preference ownership."""


def accent(palette):
    from theme_pipeline import native_theme
    return native_theme('gtk-accent', material=palette)['accent']


def refresh():
    from preferences import native
    result = native('gtk-refresh')
    status = result['status']
    if status == 'unavailable':
        print('GTK native accent unavailable; dark neutral surfaces retained.')
    elif status == 'unsupported':
        print('GTK accent enum unsupported; dark neutral surfaces retained.')
    elif status == 'applied':
        print('Native GTK accent: ' + result['accent'] + ' (applications/portals must support the setting).')
