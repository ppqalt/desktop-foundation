"""Deployment-owned entry for an existing Odysseus installation."""
import json
from pathlib import Path
import subprocess
import tomllib

from niri_wallpaper import quote


def targets(root, config, state, data, write=True):
    path = data / 'applications/desktop-foundation-odysseus.desktop'
    source = state / 'launchers/odysseus.desktop'
    if write:
        output = subprocess.run([str(root / 'scripts/foundation'), 'local-ai', 'plan'],
                                capture_output=True, text=True, check=True, timeout=10)
        configured = json.loads(output.stdout)['configured']
    else:
        # Before the installer builds Rust, its dry-run only inspects whether
        # a configured Compose file exists. Deployment uses the native plan.
        settings = tomllib.loads((root / 'config/local-ai.toml').read_text())
        override = config / 'desktop-foundation/local-ai.toml'
        if override.is_file():
            settings.update(tomllib.loads(override.read_text()))
        directory = Path(settings['compose_directory']).expanduser()
        configured = directory.is_absolute() and any((directory / name).is_file()
            for name in ['compose.yaml', 'compose.yml', 'docker-compose.yaml', 'docker-compose.yml'])
    manifest = state / 'manifest.json'
    recorded = any(entry['path'] == str(path)
                   for entry in json.loads(manifest.read_text()).get('entries', [])) if manifest.exists() else False
    if not configured and not recorded:
        return []
    if write:
        content = ('[Desktop Entry]\nType=Application\nName=Odysseus\n'
                   'Comment=Open local AI\nExec=' + quote(root / 'scripts/local-ai') + ' start\n'
                   'Icon=applications-science\nTerminal=false\nCategories=Utility;\n'
                   'Keywords=Ollama;Odysseus;Odusseys;local;AI;\n'
                   + ('Hidden=true\n' if not configured else ''))
        from theme_pipeline import atomic
        atomic(source, content)
    return [(path, source)]
