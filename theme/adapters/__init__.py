"""Semantic palette renderers and explicit application/reload classes."""
import json
import hashlib
import shutil
from pathlib import Path

MODES = {
    'quickshell': {'mode': 'LIVE', 'reload': 'foundation.reloadTheme IPC'},
    'niri': {'mode': 'RELOADABLE', 'reload': 'load-config-file'},
    'kitty': {'mode': 'RELOADABLE', 'reload': 'SIGUSR1'},
    'fish': {'mode': 'NEXT-LAUNCH', 'reload': 'or source theme.fish in existing shell'},
    'fastfetch': {'mode': 'NEXT-LAUNCH', 'reload': 'next one-shot invocation'},
    'mako': {'mode': 'RELOADABLE', 'reload': 'makoctl reload'},
    'overview': {'mode': 'LIVE', 'reload': 'foundation.reloadTheme IPC'},
    'gtk': {'mode': 'RELOADABLE', 'reload': 'supported native accent-color enum; dark neutral surfaces unchanged'},
    'spotify': {'mode': 'NEXT-LAUNCH', 'reload': 'spicetify refresh --no-restart; native UI reload optional'},
    'brave': {'mode': 'RELOADABLE', 'reload': 'opt-in approved CDP Extensions.loadUnpacked; native manual import fallback; no profile edits'},
}


def brave(p):
    def rgb(role):
        color=p[role];return [int(color[i:i+2],16) for i in (1,3,5)]
    mapping={
        'frame':'background','frame_inactive':'background','toolbar':'elevated',
        'toolbar_text':'foreground','tab_text':'foreground',
        'tab_background_text':'muted','tab_background_text_inactive':'muted',
        'background_tab':'background','background_tab_inactive':'background',
        'bookmark_text':'foreground','button_background':'selected',
        'toolbar_button_icon':'accent',
        'ntp_background':'background','ntp_text':'foreground','ntp_link':'accent',
        'omnibox_background':'background','omnibox_text':'foreground',
    }
    digest=hashlib.sha256(json.dumps(p,sort_keys=True).encode()).hexdigest()
    version='1.'+'.'.join(str(int(digest[i:i+4],16)) for i in (0,4,8))
    return {'manifest_version':3,'name':'Desktop Foundation Graphite',
            'version':version,'description':'Wallpaper-derived accents on graphite; no permissions or scripts.',
            'theme':{'colors':{key:rgb(role) for key,role in mapping.items()}}}


def render(root, target, p, profile, reset=False):
    from render_terminal import render as terminal_render
    from render_niri import render as niri_render
    for name in ('kitty','fish','fastfetch'):
        shutil.copytree(root/'terminal'/name,target/'terminal'/name,ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
    terminal=json.loads((root/'theme/fallback/terminal.json').read_text())
    if not reset:
        for key in ('background','foreground','muted','accent','error'):terminal[key]=p[key][1:]
        terminal['selection']=p['selected'][1:]
    (target/'terminal/palette.json').write_text(json.dumps(terminal,indent=2)+'\n')
    shutil.copy(root/'theme/fallback/fastfetch.json',target/'terminal/fastfetch/config.jsonc')
    terminal_render(target)
    text=(root/'theme/fallback/notifications.conf').read_text()
    if not reset:
        text=text.replace('#171b22f5',p['background']+'f5').replace('#b7bec9',p['foreground']).replace('#2b323d',p['border']).replace('#dce2ea',p['foreground'])
    (target/'notifications.conf').write_text(text)
    (target/'semantic.json').write_text(json.dumps(p,indent=2)+'\n')
    (target/'window-colors.lua').write_text('return { windowActive = '+json.dumps(p['windowActive'])+', windowInactive = '+json.dumps(p['windowInactive'])+' }\n')
    (target/'niri.kdl').write_text(niri_render(root,profile,p))
    (target/'brave').mkdir()
    (target/'brave/manifest.json').write_text(json.dumps(brave(p),indent=2)+'\n')
    from gtk_theme import accent
    (target/'gtk.json').write_text(json.dumps({'colorScheme':'prefer-dark','accent':accent(p)},indent=2)+'\n')
    from spotify_theme import colors
    (target/'spotify').mkdir()
    (target/'spotify/color.ini').write_text(colors(p))
    (target/'adapters.json').write_text(json.dumps(MODES,indent=2)+'\n')
