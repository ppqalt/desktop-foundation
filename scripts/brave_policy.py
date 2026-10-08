"""Theme transaction bridge to the Rust managed-browser adapter."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def request(*arguments):
    result = subprocess.run([str(ROOT / 'scripts/foundation'), 'browser-theme', *arguments],
                            check=True, capture_output=True, text=True, timeout=25)
    return json.loads(result.stdout)


def status():
    return request('status')


def enabled():
    return status().get('enabled', False)


def refresh(palette):
    value = request('apply', palette['elevated'])
    if value.get('enabled'):
        print('Brave managed theme updated; ' + ('live policy refresh requested.' if value.get('refreshed') else 'colors apply on next browser start.'))
        return True
    return False


def remove():
    return request('remove')
