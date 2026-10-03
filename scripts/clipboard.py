#!/usr/bin/env python3
"""Bounded clipboard storage and actions; native wl-paste owns event watching."""
import argparse
import codecs
from contextlib import closing, contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time

STATE = Path(os.environ.get('DF_CLIPBOARD_STATE', str(Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state'))) / 'desktop-foundation/clipboard')))
MAX_BYTES = 8 * 1024 * 1024
MAX_TOTAL = 32 * 1024 * 1024
MAX_ITEMS = 100


def atomic(path, data):
    temporary = path.with_suffix(path.suffix + '.tmp')
    with temporary.open('wb') as stream:
        os.chmod(temporary, 0o600)
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)


@contextmanager
def database(*, export_after=True):
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(STATE, 0o700)
    with (STATE / 'lock').open('a') as lock:
        os.chmod(STATE / 'lock', 0o600)
        fcntl.flock(lock, fcntl.LOCK_EX)
        with closing(sqlite3.connect(STATE / 'history.sqlite')) as db, db:
            os.chmod(STATE / 'history.sqlite', 0o600)
            db.execute('PRAGMA secure_delete=ON')
            db.execute('CREATE TABLE IF NOT EXISTS clips (id TEXT PRIMARY KEY, mime TEXT, payload BLOB, updated REAL)')
            yield db
            db.commit()
            if export_after:
                export(db)


def export(db):
    entries = []
    images = set()
    # The UI needs metadata and bounded text previews, not every image BLOB.
    # Four bytes per code point covers 2048 UTF-8 characters. Incremental
    # decoding retains strict validation while allowing a cut trailing sequence.
    query = """SELECT id, mime, length(payload), updated,
                      CASE WHEN mime LIKE 'image/%' THEN NULL
                           ELSE substr(payload, 1, 8192) END
               FROM clips ORDER BY updated DESC"""
    for identity, mime, size, updated, prefix in db.execute(query):
        image = mime.startswith('image/')
        filename = STATE / (identity + '.' + mime.split('/')[1])
        if image:
            images.add(filename.name)
            if not filename.exists():
                payload = db.execute('SELECT payload FROM clips WHERE id=?', (identity,)).fetchone()[0]
                atomic(filename, payload)
        preview = 'Copied image' if image else codecs.getincrementaldecoder('utf-8')().decode(prefix, final=size <= 8192)[:2048]
        entries.append({'id': identity, 'mime': mime, 'preview': preview, 'size': size,
                        'updated': updated, 'image': filename.as_uri() if image else ''})
    atomic(STATE / 'index.json', json.dumps(entries, ensure_ascii=True).encode())
    for file in STATE.iterdir():
        if file.suffix in {'.png', '.jpeg', '.gif', '.webp'} and file.name not in images:
            file.unlink()


def image_mime(data):
    if data.startswith(b'\x89PNG\r\n\x1a\n'):
        return 'image/png'
    if data.startswith(b'\xff\xd8\xff'):
        return 'image/jpeg'
    if data.startswith((b'GIF87a', b'GIF89a')):
        return 'image/gif'
    if data.startswith(b'RIFF') and data[8:12] == b'WEBP':
        return 'image/webp'
    return None


def store(kind, data):
    if os.environ.get('CLIPBOARD_STATE', 'data') != 'data' or not data or len(data) > MAX_BYTES:
        return
    mime = image_mime(data) if kind == 'image' else 'text/plain;charset=utf-8'
    if not mime:
        return
    if kind == 'text':
        try:
            data.decode('utf-8')
        except UnicodeDecodeError:
            return
    identity = hashlib.sha256(mime.encode() + b'\0' + data).hexdigest()
    with database() as db:
        db.execute('INSERT OR REPLACE INTO clips VALUES (?,?,?,?)', (identity, mime, data, time.time()))
        rows = db.execute('SELECT id,length(payload) FROM clips ORDER BY updated DESC').fetchall()
        total = 0
        for index, (key, size) in enumerate(rows):
            total += size
            if index >= MAX_ITEMS or total > MAX_TOTAL:
                db.execute('DELETE FROM clips WHERE id=?', (key,))


def main():
    global STATE
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', type=Path)
    parser.add_argument('action', choices=['init', 'store', 'copy', 'delete', 'clear'])
    parser.add_argument('value', nargs='?')
    args = parser.parse_args()
    if args.state:
        STATE = args.state.resolve()
    if args.action == 'store':
        if args.value not in {'text', 'image'}:
            parser.error('store requires text or image')
        store(args.value, sys.stdin.buffer.read(MAX_BYTES + 1))
        return
    payload = None
    with database(export_after=args.action != 'copy') as db:
        if args.action == 'clear':
            db.execute('DELETE FROM clips')
        elif args.action in {'copy', 'delete'}:
            if not args.value or len(args.value) != 64 or any(c not in '0123456789abcdef' for c in args.value):
                parser.error('Invalid history ID')
            row = db.execute('SELECT mime,payload FROM clips WHERE id=?', (args.value,)).fetchone()
            if not row:
                raise SystemExit('History item no longer exists')
            if args.action == 'delete':
                db.execute('DELETE FROM clips WHERE id=?', (args.value,))
            else:
                payload = row
    if payload:
        mime, data = payload
        subprocess.run(['wl-copy', '--type', mime], input=data, check=True, timeout=5)


if __name__ == '__main__':
    main()
