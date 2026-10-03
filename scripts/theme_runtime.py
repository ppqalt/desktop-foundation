"""Immutable wallpaper/theme bundles, atomic current pointer, retained rollback."""
import hashlib
import fcntl
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
import tomllib
import uuid

ROOT=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(ROOT))
from theme.adapters import render


def state():
    return Path(os.environ.get('XDG_STATE_HOME',Path.home()/'.local/state'))/'desktop-foundation/theme'


def config(root=ROOT):
    active=state()/'current/wallpaper.toml'
    path=active if active.is_file() else root/'compositor/niri/wallpaper.toml'
    c=tomllib.loads(path.read_text())
    image=Path(c['image']).expanduser()
    if not image.is_file() and path!=active:
        image=root/'wallpapers'/image.name
    c['image']=str(image)
    return c


def current():
    link=state()/'current'
    return link.resolve(strict=True) if link.exists() else None


def link(name,target):
    home=state();home.mkdir(parents=True,exist_ok=True)
    temp=home/('.'+name+'-'+uuid.uuid4().hex)
    temp.symlink_to(target)
    os.replace(temp,home/name)
    fd=os.open(home,os.O_DIRECTORY)
    try:os.fsync(fd)
    finally:os.close(fd)


def profile():
    p=state()/'profile.json'
    return json.loads(p.read_text())['profile'] if p.exists() else 'default'


def stage(p,image,mode='fill',reset=False,selected_profile=None):
    from PIL import Image
    from niri_wallpaper import prepare_backdrop
    image=Path(image).expanduser().resolve(strict=True)
    with Image.open(image) as check:check.verify()
    if mode not in {'fill','fit','center','tile'}:raise ValueError('Invalid scaling mode')
    home=state();revisions=home/'revisions';revisions.mkdir(parents=True,exist_ok=True)
    target=revisions/(str(time.time_ns())+'-'+uuid.uuid4().hex[:8])
    target.mkdir()
    try:
        copied=target/('wallpaper'+image.suffix.lower());shutil.copyfile(image,copied)
        backdrop=prepare_backdrop(copied,mode)
        shutil.copyfile(backdrop,target/'overview.png')
        render(ROOT,target,p,selected_profile or profile(),reset)
        (target/'wallpaper.toml').write_text('image = '+json.dumps(str(copied))+'\nmode = '+json.dumps(mode)+'\n')
        (target/'backdrop.json').write_text(json.dumps({'image':(target/'overview.png').as_uri(),'mode':mode})+'\n')
        metadata={'root':str(ROOT),'input':str(image),'wallpaperHash':hashlib.sha256(image.read_bytes()).hexdigest(),
                  'profile':selected_profile or profile(),'reset':reset}
        (target/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
        subprocess.run(['niri','validate','-c',str(target/'niri.kdl')],check=True,capture_output=True)
        subprocess.run(['fish','--no-config','-n',str(target/'terminal/fish/theme.fish')],check=True,capture_output=True)
        # All files are complete before the pointer can name this bundle.
        for file in target.rglob('*'):
            if file.is_file():
                with file.open('rb') as stream:os.fsync(stream.fileno())
        return target
    except Exception:
        shutil.rmtree(target)
        raise


def reload(wallpaper=False):
    if os.environ.get('NIRI_SOCKET'):
        if wallpaper:
            subprocess.run(['systemctl','--user','restart','desktop-foundation-wallpaper.service'],check=True)
        subprocess.run(['niri','msg','action','load-config-file'],check=True)
        result=subprocess.run([str(ROOT/'scripts/shell-ipc'),'reloadTheme'],capture_output=True,text=True)
        if result.returncode:raise RuntimeError('Shell theme reload failed: '+result.stderr.strip())
    if shutil.which('makoctl'):
        result=subprocess.run(['makoctl','reload'],capture_output=True)
        if result.returncode:print('Mako absent; rendered colors apply on next launch.')
    for proc in Path('/proc').iterdir():
        if proc.name.isdigit():
            try:
                if proc.stat().st_uid==os.getuid() and (proc/'comm').read_text().strip()=='kitty':os.kill(int(proc.name),signal.SIGUSR1)
            except OSError:pass
    from spotify_theme import refresh as spotify_refresh
    spotify_refresh()
    if (state()/'brave-devtools.json').exists():
        subprocess.run([str(ROOT/'scripts/brave-theme-reload')],check=True)
    else:
        print('Brave theme prepared: '+str(state()/'brave')+' (native refresh required; DevTools adapter not opted in).')


def publish(target,wallpaper=False,live=True):
    from theme_pipeline import atomic
    old=current()
    if old and json.loads((old/'metadata.json').read_text())['root']!=str(ROOT):
        raise RuntimeError('Another checkout owns active theme state')
    link('current',target)
    try:
        # Physical stable directory: Chromium can canonicalize symlink paths.
        # No browser profile files are read or edited. Browser import is manual.
        atomic(state()/'brave/manifest.json',(target/'brave/manifest.json').read_text())
        if live:reload(wallpaper)
    except Exception:
        if old:
            link('current',old)
            atomic(state()/'brave/manifest.json',(old/'brave/manifest.json').read_text())
            try:reload(wallpaper)
            except Exception as error:print('Rollback published; reload needs attention: '+str(error),file=sys.stderr)
        else:
            (state()/'current').unlink(missing_ok=True)
            (state()/'brave/manifest.json').unlink(missing_ok=True)
        raise
    if old:link('previous',old)


def ensure(selected_profile='default'):
    home=state().parent;home.mkdir(parents=True,exist_ok=True)
    with (home/'theme.lock').open('w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        _ensure(selected_profile)


def _ensure(selected_profile):
    from theme_pipeline import atomic,read
    home=state();home.mkdir(parents=True,exist_ok=True)
    previous=current()
    if previous:
        metadata=read(previous/'metadata.json')
        if metadata['root']!=str(ROOT):raise RuntimeError('Different checkout owns runtime theme')
        # Re-render at deliberate deployment so updated repository templates
        # propagate while retaining the selected wallpaper and semantic palette.
        c=config();target=stage(read(previous/'semantic.json'),c['image'],c['mode'],metadata.get('reset',False),selected_profile)
        publish(target,live=False)
    else:
        c=config();target=stage(read(ROOT/'theme/generated.json'),c['image'],c['mode'],selected_profile=selected_profile)
        publish(target,live=False)
    atomic(home/'profile.json',json.dumps({'profile':selected_profile})+'\n')


def apply(p,reset=False,image=None,wallpaper=False,mode=None):
    c=config();image=Path(image or c['image']).expanduser().resolve(strict=True)
    old=current()
    digest=hashlib.sha256(image.read_bytes()).hexdigest()
    desired_mode=mode or c.get('mode','fill')
    if old and {k:v for k,v in json.loads((old/'semantic.json').read_text()).items() if k!='source'}=={k:v for k,v in p.items() if k!='source'} and json.loads((old/'metadata.json').read_text())['wallpaperHash']==digest and desired_mode==c['mode']:
        print('Already active; no publication or reload needed.');return
    target=stage(p,image,desired_mode,reset)
    publish(target,wallpaper)


def rollback():
    target=state()/'previous'
    if not target.exists():raise RuntimeError('No previous runtime theme exists')
    publish(target.resolve(),wallpaper=True)
