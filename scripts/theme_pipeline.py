"""One-shot Matugen -> graphite semantics -> validated, reversible render outputs."""
import argparse
import colorsys
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
VERSION = 'graphite-v1'


def file_hash(path):
    """Bound hashing memory regardless of original wallpaper size."""
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(path):
    return json.loads(path.read_text())


def rgb(color):
    if not re.fullmatch(r'#[0-9a-fA-F]{6}', color):
        raise ValueError('Invalid palette color')
    return tuple(int(color[i:i+2], 16) / 255 for i in (1, 3, 5))


def hexcolor(values):
    return '#' + ''.join(f'{round(max(0, min(1, v))*255):02x}' for v in values)


def mix(base, tint, amount, saturation=None):
    values = tuple(a*(1-amount)+b*amount for a,b in zip(rgb(base), rgb(tint)))
    if saturation is not None:
        h,l,s = colorsys.rgb_to_hls(*values)
        values = colorsys.hls_to_rgb(h,l,min(s,saturation))
    return hexcolor(values)


def luminance(color):
    values = [v/12.92 if v <= .04045 else ((v+.055)/1.055)**2.4 for v in rgb(color)]
    return sum(v*w for v,w in zip(values, (.2126,.7152,.0722)))


def contrast(a,b):
    x,y = sorted((luminance(a),luminance(b)))
    return (y+.05)/(x+.05)


def derive(material, source, digest):
    p = dict(read(ROOT/'theme/fallback/semantic.json'))
    primary,secondary = material['primary'],material['secondary']
    p.update(source=source,wallpaperHash=digest,policy=VERSION)
    p['background'] = mix('#171b22', primary,.06,.20)
    p['elevated'] = mix('#222832',primary,.10,.20)
    p['iconTile'] = mix('#172029',primary,.12,.22)
    p['hover'] = mix('#222b38',primary,.12,.22)
    p['selected'] = mix('#293649',primary,.17,.24)
    p['border'] = mix('#38414e',primary,.18,.22)
    p['selectionBorder'] = mix('#485a73',primary,.30,.25)
    p['windowActive'] = mix('#485669',primary,.28,.25)
    p['windowInactive'] = mix('#2b323d',primary,.10,.20)
    p['accent'] = primary
    p['accentStrong'] = mix(primary,secondary,.20)
    # Foreground/muted/error/shadows remain neutral/readable static roles.
    for key in ('accent','accentStrong'):
        while min(contrast(p[key],p[b]) for b in ('background','elevated','selected')) < 4.5:
            new = mix(p[key],'#ffffff',.10)
            if new == p[key]:
                raise ValueError('Could not satisfy accent contrast')
            p[key] = new
    for key in ('foreground','muted'):
        if min(contrast(p[key],p[b]) for b in ('background','elevated','selected')) < (7 if key=='foreground' else 3):
            raise ValueError('Text contrast constraint failed')
    return p


def generate(image):
    executable = shutil.which('matugen')
    if not executable:
        raise RuntimeError('matugen is missing; current theme was left intact')
    image = image.expanduser().resolve(strict=True)
    digest = file_hash(image)
    cache = Path(os.environ.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'desktop-foundation/themes'
    cache.mkdir(parents=True,exist_ok=True)
    target = cache / (digest+'-'+VERSION+'.json')
    if target.exists():
        p = read(target)
        for role in read(ROOT/'theme/fallback/semantic.json'):
            if role not in {'source','scrim'}: rgb(p[role])
        if p.get('policy') != VERSION:
            raise ValueError('Cached palette policy mismatch; current theme left intact')
        p['source'] = str(image.relative_to(ROOT)) if image.is_relative_to(ROOT) else str(image)
        return p,True
    # Isolate user templates/hooks; --dry-run ensures no Matugen mutations.
    with tempfile.TemporaryDirectory() as directory:
        config = Path(directory)/'config.toml'
        config.write_text('[config]\n[templates]\n')
        result = subprocess.run([executable,'--config',str(config),'--dry-run','--mode','dark',
                                 '--json','hex','--source-color-index','0','image',str(image)],
                                check=True,capture_output=True,text=True,timeout=60)
    colors = json.loads(result.stdout)['colors']
    # Matugen 4.x uses role -> mode -> color. Accept legacy mode -> role too.
    material = colors.get('dark') or {name: value['dark']['color'] for name,value in colors.items()}
    source = str(image.relative_to(ROOT)) if image.is_relative_to(ROOT) else str(image)
    p = derive(material,source,digest)
    atomic(target,json.dumps(p,indent=2)+'\n')
    return p,False


def atomic(path, content):
    path.parent.mkdir(parents=True,exist_ok=True)
    fd,name = tempfile.mkstemp(prefix='.'+path.name,dir=path.parent)
    try:
        with os.fdopen(fd,'w') as file:
            file.write(content)
            file.flush()
            os.fsync(file.fileno())
        os.chmod(name,0o644)
        os.replace(name,path)
        directory = os.open(path.parent, os.O_DIRECTORY)
        try: os.fsync(directory)
        finally: os.close(directory)
    finally:
        if os.path.exists(name): os.unlink(name)


def apply(p,reset=False):
    from theme_runtime import apply as runtime_apply
    runtime_apply(p,reset)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action',choices=['generate','apply','reset','wallpaper','rollback','promote'])
    parser.add_argument('image',nargs='?')
    parser.add_argument('--mode',choices=['fill','fit','center','tile'])
    args = parser.parse_args()
    started = time.perf_counter()
    state = Path(os.environ.get('XDG_STATE_HOME',Path.home()/'.local/state'))/'desktop-foundation'
    state.mkdir(parents=True,exist_ok=True)
    with (state/'theme.lock').open('w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        from theme_runtime import config,rollback,current,apply as runtime_apply
        if args.action=='rollback':
            rollback();return
        if args.action=='promote':
            chosen=read(current()/'semantic.json')
            chosen['source']='promoted-runtime-palette'
            atomic(ROOT/'theme/generated.json',json.dumps(chosen,indent=2)+'\n')
            print('Updated shipped palette deliberately; review Git diff before committing.');return
        if args.action=='reset':
            p=read(ROOT/'theme/fallback/semantic.json');hit=False
        else:
            if args.action=='wallpaper':
                if not args.image:raise ValueError('wallpaper-set requires an image path')
                from PIL import Image
                with Image.open(Path(args.image).expanduser()) as check:check.verify()
            configured=config()['image']
            p,hit=generate(Path(args.image or configured))
        if args.action=='generate': print(json.dumps(p,indent=2))
        elif args.action=='wallpaper':
            if not args.image:raise ValueError('wallpaper-set requires an image path')
            runtime_apply(p,image=args.image,wallpaper=True,mode=args.mode)
        else: apply(p,args.action=='reset')
    if args.action in {'apply', 'reset', 'wallpaper'}:
        # Retention acquires the same lock after publication has released it.
        from theme_runtime import maintain_cache
        maintain_cache()
    print(f'{args.action}: {time.perf_counter()-started:.3f}s; palette cache hit={hit}',file=__import__('sys').stderr)


if __name__=='__main__':
    try:
        main()
    except (OSError,ValueError,KeyError,RuntimeError,subprocess.SubprocessError) as error:
        raise SystemExit(f'Theme failed: {error}. No Matugen/template outputs were applied on generation failure.')
