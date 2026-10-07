"""Private native-preference fixtures; never contact the desktop session."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/debug/desktop-foundationctl'
SPY = r'''import json, os, pathlib, sys, time
args = sys.argv[1:]
name = pathlib.Path(sys.argv[0]).name
with pathlib.Path(os.environ['PREF_LOG']).open('a') as output:
    output.write(json.dumps([name, *args]) + '\n')
values = pathlib.Path(os.environ['PREF_VALUES'])
saved = json.loads(values.read_text())
if name == 'gsettings':
    assert args[1] == 'org.gnome.desktop.interface', args
    if args[0] == 'range':
        if os.environ.get('PREF_RANGE_FAIL'): sys.exit(3)
        print(os.environ.get('PREF_RANGE', "enum\n'blue'\n'orange'\n'slate'\n'purple'"))
        sys.exit(0)
    key = args[2]
    if args[0] == 'get': print(saved.get(key, "'default'")); sys.exit(0)
    assert args[0] == 'set', args
    value = args[3]
else:
    if args[0] == 'query': print(saved.get(args[2], '')); sys.exit(0)
    assert args[0] == 'default', args
    value, key = args[1:]
if key == os.environ.get('PREF_FAIL_KEY'): sys.exit(4)
if key == os.environ.get('PREF_STALL_KEY'): time.sleep(2)
saved[key] = value
values.write_text(json.dumps(saved))
if key == os.environ.get('PREF_BREAK_PUBLICATION'):
    journal = pathlib.Path(os.environ['PREF_JOURNAL'])
    journal.rename(journal.with_suffix('.saved'))
    journal.mkdir()
'''


class PreferenceFixture:
    def __init__(self, case):
        self.temp = tempfile.TemporaryDirectory(prefix='foundation-preferences-')
        case.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.state = self.base / 'state/desktop-foundation'
        self.journal = self.state / 'preferences.json'
        self.values_file = self.base / 'values.json'
        self.values_file.write_text(json.dumps({'color-scheme': "'default'", 'gtk-theme': "'Adwaita'", 'accent-color': "'blue'"}))
        self.log = self.base / 'calls.jsonl'
        self.bin = self.base / 'bin'; self.bin.mkdir()
        self.env = dict(PATH=str(self.bin), HOME=str(self.base), LANG='C', LC_ALL='C',
                        XDG_STATE_HOME=str(self.base / 'state'), XDG_CONFIG_HOME=str(self.base / 'config'),
                        XDG_DATA_HOME=str(self.base / 'data'), XDG_CACHE_HOME=str(self.base / 'cache'),
                        XDG_RUNTIME_DIR=str(self.base / 'runtime'),
                        PREF_LOG=str(self.log), PREF_VALUES=str(self.values_file), PREF_JOURNAL=str(self.journal))
        for name in ('gsettings', 'xdg-mime'):
            executable = self.bin / name
            executable.write_text('#!' + sys.executable + '\n' + SPY)
            executable.chmod(0o755)
        self.checkout = self.base / 'checkout'
        wrapper = self.checkout / 'scripts/foundation'; wrapper.parent.mkdir(parents=True)
        wrapper.write_text('#!' + sys.executable + '\nimport os,sys\nos.execv(' + repr(str(BINARY)) + ', [' + repr(str(BINARY)) + ', "--root", ' + repr(str(ROOT)) + ', *sys.argv[1:]])\n')
        wrapper.chmod(0o755)

    def values(self, **changes):
        data = json.loads(self.values_file.read_text())
        if changes:
            data.update(changes); self.values_file.write_text(json.dumps(data))
        return data

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def invoke(self, *arguments, check=True, lock=None):
        command = [str(BINARY), '--root', str(ROOT), 'preferences', '--state', str(self.state), *arguments]
        descriptors = ()
        if lock is not None:
            descriptors = (lock.fileno(),)
            command += ['--lock-fd', str(lock.fileno())]
        result = subprocess.run(command, env=self.env, capture_output=True, text=True, timeout=15, pass_fds=descriptors)
        if check and result.returncode:
            raise AssertionError(result.stderr)
        return result

    def install(self):
        self.invoke('set', 'gsettings', 'color-scheme', "'prefer-dark'")
        self.invoke('set', 'gsettings', 'gtk-theme', "'adw-gtk3-dark'")

    def saved(self):
        return json.loads(self.journal.read_text())

    def palette(self, color='#ffb599'):
        palette = self.state / 'theme/current/semantic.json'
        palette.parent.mkdir(parents=True, exist_ok=True)
        palette.write_text(json.dumps({'accent': color}))
