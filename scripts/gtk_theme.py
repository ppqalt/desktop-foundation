"""Native discrete GNOME/libadwaita accent; no CSS overrides or resident worker."""
import colorsys
import json
from pathlib import Path
import subprocess

ANCHORS = {'blue': '#3584e4', 'teal': '#2190a4', 'green': '#3a944a', 'yellow': '#c88800',
           'orange': '#ed5b00', 'red': '#e62d42', 'pink': '#d56199', 'purple': '#9141ac'}


def accent(palette):
    def hls(color):
        return colorsys.rgb_to_hls(*(int(color[i:i+2], 16) / 255 for i in (1, 3, 5)))
    hue, _, saturation = hls(palette['accent'])
    if saturation < .12:
        return 'slate'
    return min(ANCHORS, key=lambda key: min(abs(hls(ANCHORS[key])[0] - hue), 1 - abs(hls(ANCHORS[key])[0] - hue)))


def refresh():
    from theme_runtime import current
    from preferences import manage_setting
    revision = current()
    if revision is None:
        return
    try:
        supported = subprocess.check_output(['gsettings', 'range', 'org.gnome.desktop.interface', 'accent-color'], text=True, stderr=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError):
        print('GTK native accent unavailable; dark neutral surfaces retained.'); return
    value = accent(json.loads((revision / 'semantic.json').read_text()))
    if "'" + value + "'" not in supported:
        print('GTK accent enum unsupported; dark neutral surfaces retained.'); return
    manage_setting(['gsettings', 'accent-color'], "'" + value + "'")
    print('Native GTK accent: ' + value + ' (applications/portals must support the setting).')
