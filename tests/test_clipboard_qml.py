"""Opt-in Qt service integration; no surface and no real clipboard operation."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'native/foundation/target/release/desktop-foundationctl'


class ClipboardService(unittest.TestCase):
    @unittest.skipUnless(os.environ.get('DF_TEST_NIRI_IPC') == '1' and shutil.which('quickshell') and os.environ.get('WAYLAND_DISPLAY'), 'opt-in isolated Qt runtime: DF_TEST_NIRI_IPC=1')
    def test_default_command_file_watch_and_errors_use_private_native_history(self):
        with tempfile.TemporaryDirectory(prefix='foundation-clipboard-qml-') as directory:
            base = Path(directory); shell = base / 'shell'; shell.mkdir()
            state = base / 'history'
            (base / 'native').symlink_to(ROOT / 'native', target_is_directory=True)
            (shell / 'services').symlink_to(ROOT / 'shell/services', target_is_directory=True)
            env = dict(os.environ, DF_CLIPBOARD_STATE=str(state), DF_CLIPBOARD_WORKER='',
                       XDG_CACHE_HOME=str(base / 'cache'), XDG_STATE_HOME=str(base / 'state'), CLIPBOARD_STATE='data')
            subprocess.run([str(BINARY), 'clipboard', '--state', str(state), 'store', 'text'], input=b'private fixture', check=True, env=env)
            (shell / 'shell.qml').write_text('''import Quickshell
import Quickshell.Io
import "services"
ShellRoot {
    ClipboardHistory { id: history }
    IpcHandler {
        target: "clipboardfixture"
        function status(): string { return JSON.stringify({entries:history.entries, loading:history.loading, busy:history.busy, error:history.error, command:history.backendCommand}); }
        function clear(): void { history.request("clear", ""); }
        function invalid(): void { history.request("delete", "../escape"); }
    }
}
''')
            with (base / 'log').open('w') as log:
                process = subprocess.Popen(['quickshell', '--path', str(shell)], env=env, stdout=log, stderr=log)
                def ipc(method):
                    return subprocess.run(['quickshell', 'ipc', '--pid', str(process.pid), 'call', 'clipboardfixture', method], capture_output=True, text=True, timeout=5)
                def wait(predicate):
                    deadline = time.monotonic() + 6
                    while time.monotonic() < deadline:
                        try:
                            status = json.loads(ipc('status').stdout)
                            if predicate(status): return status
                        except (ValueError, KeyError): pass
                        time.sleep(.05)
                    raise AssertionError('Private Qt clipboard fixture failed: ' + (base / 'log').read_text())
                try:
                    initial = wait(lambda s: not s['loading'] and len(s['entries']) == 1)
                    self.assertEqual(initial['command'], [str(shell / '../native/foundation/target/release/desktop-foundationctl'), '--root', str(shell / '..'), 'clipboard'])
                    ipc('clear'); wait(lambda s: not s['busy'] and not s['entries'] and not s['error'])
                    ipc('invalid'); wait(lambda s: not s['busy'] and 'Could not delete' in s['error'])
                    (state / 'index.json').write_text('{}')
                    wait(lambda s: s['error'] == 'History could not be read.')
                    self.assertIn('Invalid history ID', (base / 'log').read_text())
                finally:
                    process.terminate(); process.wait(timeout=5)
