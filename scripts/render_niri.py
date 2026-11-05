"""Compatibility API for the shared native Niri renderer."""
import json
from pathlib import Path
import subprocess


def render(root, profile, semantic=None):
    root = Path(root).resolve()
    command = [str(root / 'native/foundation/target/release/desktop-foundationctl'),
               '--root', str(root), 'render', 'niri', profile]
    if semantic is not None:
        command.append('--semantic-stdin')
    result = subprocess.run(command, input=json.dumps(semantic) if semantic is not None else None,
                            text=True, capture_output=True, check=True)
    return result.stdout
