"""Marketplace color-layer ownership; no Spotify account/user-data access."""
import configparser
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parent.parent
SCHEME='DesktopFoundation'


def paths():
    config=Path(os.environ.get('XDG_CONFIG_HOME',Path.home()/'.config'))/'spicetify'
    state=Path(os.environ.get('XDG_STATE_HOME',Path.home()/'.local/state'))/'desktop-foundation'
    return config,state/'spotify/theme-binding.json',state/'theme/current/spotify/color.ini'


def colors(p):
    roles={'text':'foreground','subtext':'muted','main':'background','sidebar':'background','player':'background',
           'card':'elevated','shadow':None,'selected-row':'selected','button':'accent','button-active':'accentStrong',
           'button-disabled':'border','tab-active':'selected','notification':'elevated','notification-error':'error',
           'misc':'muted','highlight':'elevated','highlight-elevated':'selected'}
    return '['+SCHEME+']\n'+''.join(f'{key} = {p[role][1:] if role else "000000"}\n' for key,role in roles.items())


def parse(text):
    c=configparser.ConfigParser(interpolation=None);c.read_string(text);return c


def setting(text,key,value):
    # Replace only one Setting key; keep all unrelated lines byte-for-byte.
    match=re.search(r'(?ms)^\[Setting\]\s*\n(.*?)(?=^\[|\Z)',text)
    if not match:raise RuntimeError('Spicetify Setting section missing')
    body=match.group(1)
    pattern=r'(?m)^'+re.escape(key)+r'\s*=.*$'
    if value is None:
        return text[:match.start(1)]+re.sub(pattern+'\n?', '', body)+text[match.end(1):]
    new=re.sub(pattern,key+' = '+value,body) if re.search(pattern,body) else body+key+' = '+value+'\n'
    return text[:match.start(1)]+new+text[match.end(1):]


def merge(text,generated):
    c=parse(text);g=parse(generated)
    if c.has_section(SCHEME):c.remove_section(SCHEME)
    c[SCHEME]=dict(g[SCHEME]);out=io.StringIO();c.write(out);return out.getvalue()


def install():
    from theme_pipeline import atomic
    config,journal,generated=paths();file=config/'config-xpui.ini';color=config/'Themes/marketplace/color.ini'
    if not file.exists():raise RuntimeError('Provision Spicetify/Marketplace first')
    text=file.read_text();c=parse(text)
    if c.get('Setting','current_theme',fallback='')!='marketplace':
        raise RuntimeError('Existing theme is not marketplace; refusing to replace an unrelated theme')
    if 'marketplace' not in c.get('AdditionalOptions','custom_apps',fallback='').split('|'):
        raise RuntimeError('Marketplace is not configured')
    if journal.exists():
        saved=json.loads(journal.read_text())
        for key,value in saved['owned'].items():
            accepted={value,saved['original'][key]} if saved.get('pending') else {value}
            if c.get('Setting',key,fallback=None) not in accepted:raise RuntimeError('Theme setting changed outside adapter: '+key)
    else:
        previous=parse(color.read_text() if color.exists() else '')
        if previous.has_section(SCHEME):raise RuntimeError('DesktopFoundation color scheme already exists without ownership')
        owned={'color_scheme':SCHEME,'replace_colors':'1'}
        saved={'original':{k:c.get('Setting',k,fallback=None) for k in owned},'owned':owned,'pending':True}
        atomic(journal,json.dumps(saved,indent=2)+'\n')
    semantic=generated.parent.parent/'semantic.json'
    palette=generated.read_text() if generated.exists() else colors(json.loads((semantic if semantic.exists() else ROOT/'theme/generated.json').read_text()))
    atomic(color,merge(color.read_text() if color.exists() else '',palette))
    for key,value in saved['owned'].items():text=setting(text,key,value)
    atomic(file,text)
    saved.pop('pending',None);atomic(journal,json.dumps(saved,indent=2)+'\n')
    print('Marketplace graphite color layer configured; existing theme CSS/apps/extensions preserved.')


def refresh():
    from theme_pipeline import atomic
    config,journal,generated=paths()
    if not journal.exists():return
    file=config/'config-xpui.ini'
    if not file.exists():print('Spotify absent; generated color layer retained.');return
    c=parse(file.read_text());saved=json.loads(journal.read_text())
    if c.get('Setting','current_theme',fallback='')!='marketplace' or any(c.get('Setting',k,fallback=None)!=v for k,v in saved['owned'].items()):
        raise RuntimeError('Spotify color settings changed outside adapter; refusing overwrite')
    color=config/'Themes/marketplace/color.ini'
    semantic=generated.parent.parent/'semantic.json'
    palette=generated.read_text() if generated.exists() else colors(json.loads(semantic.read_text()))
    atomic(color,merge(color.read_text(),palette))
    app=Path(c.get('Setting','spotify_path',fallback=''))/'Apps/xpui'
    executable=shutil.which('spicetify')
    if not executable or not (app/'spicetify-config.json').exists():
        print('Spotify patch not initialized; colors staged for post-install apply.');return
    subprocess.run([executable,'refresh','--no-restart'],check=True,capture_output=True,text=True)
    print('Spotify color assets refreshed; applies on next launch or native UI reload (no restart).')


def restore():
    from theme_pipeline import atomic
    config,journal,_=paths()
    if not journal.exists():return
    saved=json.loads(journal.read_text());file=config/'config-xpui.ini';text=file.read_text();c=parse(text)
    for key,value in saved['owned'].items():
        if c.get('Setting',key,fallback=None)!=value:raise RuntimeError('Theme setting changed outside adapter: '+key)
    for key,value in saved['original'].items():
        text=setting(text,key,value)
    color=config/'Themes/marketplace/color.ini';p=parse(color.read_text());p.remove_section(SCHEME);out=io.StringIO();p.write(out)
    atomic(color,out.getvalue());atomic(file,text);journal.unlink()
    executable=shutil.which('spicetify')
    if executable:subprocess.run([executable,'refresh','--no-restart'],check=True,capture_output=True)
    print('Only owned Spotify scheme/settings restored; Marketplace retained.')


if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['install','refresh','restore']);a=p.parse_args()
    try:globals()[a.action]()
    except (OSError,ValueError,RuntimeError,subprocess.SubprocessError) as error:raise SystemExit(str(error))
