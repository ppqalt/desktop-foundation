"""Pure btop 1.4.7 theme output shared by either future monitor host."""
import re


def colors(palette):
    # Keys follow btop's Default_theme. Complete output avoids inheriting its
    # fixed red/green/blue colors when the wallpaper palette changes.
    roles = {
        'main_bg': 'background',
        'main_fg': 'foreground',
        'title': 'foreground',
        'hi_fg': 'accent',
        'selected_bg': 'selected',
        'selected_fg': 'foreground',
        'inactive_fg': 'muted',
        'graph_text': 'muted',
        'meter_bg': 'border',
        'proc_misc': 'accent',
        'cpu_box': 'border',
        'mem_box': 'border',
        'net_box': 'border',
        'proc_box': 'border',
        'div_line': 'border',
        'proc_pause_bg': 'selected',
        'proc_follow_bg': 'selected',
        'proc_banner_bg': 'elevated',
        'proc_banner_fg': 'foreground',
        'followed_bg': 'selected',
        'followed_fg': 'foreground',
    }
    for graph in ('temp', 'cpu', 'free', 'cached', 'available', 'used',
                  'download', 'upload', 'process'):
        roles[graph + '_start'] = 'border'
        roles[graph + '_mid'] = 'accent'
        roles[graph + '_end'] = (
            'error' if graph in ('temp', 'cpu', 'used', 'process')
            else 'accentStrong')
    lines = ['# Desktop Foundation; generated from semantic palette roles.']
    for key, role in roles.items():
        value = palette[role]
        if not isinstance(value, str) or not re.fullmatch(r'#[0-9a-fA-F]{6}', value):
            raise ValueError('Invalid btop palette color: ' + role)
        lines.append(f'theme[{key}]="{value}"')
    return '\n'.join(lines) + '\n'
