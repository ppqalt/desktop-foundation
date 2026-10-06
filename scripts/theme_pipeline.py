"""One-shot Matugen -> graphite semantics -> validated, reversible render outputs."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
VERSION = 'graphite-v1'


def file_hash(path):
    """Bound hashing memory regardless of original wallpaper size."""
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(path):
    return json.loads(path.read_text())


def native_theme(*arguments, material=None):
    """Run the finite Rust palette backend without applying any runtime outputs."""
    command = [str(ROOT/'scripts/foundation'), 'theme', *arguments]
    try:
        process = subprocess.run(command, input=json.dumps(material) if material is not None else None,
                                 capture_output=True, text=True, check=True, timeout=75)
    except subprocess.CalledProcessError as error:
        raise RuntimeError(error.stderr.strip() or 'Native palette generation failed; current theme was left intact') from error
    except subprocess.TimeoutExpired as error:
        raise RuntimeError('Native palette generation timed out; current theme was left intact') from error
    output = process.stdout
    try:
        result = json.loads(output)
    except json.JSONDecodeError as error:
        raise RuntimeError('Native palette backend returned invalid JSON') from error
    if not isinstance(result, dict):
        raise RuntimeError('Native palette backend returned an invalid response')
    return result


def derive(material, source, digest):
    """Compatibility entry point; all semantic color policy now lives in Rust."""
    return native_theme('derive', '--source', str(source), '--hash', str(digest), material=material)


def generate(image):
    result = native_theme('generate', str(Path(image).expanduser()))
    if not isinstance(result.get('palette'), dict) or not isinstance(result.get('cached'), bool):
        raise RuntimeError('Native palette backend returned an invalid generation response')
    return result['palette'], result['cached']


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
