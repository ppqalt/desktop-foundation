"""Regression checks for media keys and independent Transparency shortcuts."""
import ctypes
import ctypes.util
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BINDINGS = ROOT / "compositor/niri/bindings.kdl"


class NiriMediaTransparencyBindings(unittest.TestCase):
    def test_keys_dispatch_once_to_the_intended_action(self):
        text = BINDINGS.read_text()
        found = re.findall(r'^\s*"([^"]+)"\s+repeat=false\s*\{\s*spawn-sh\s+"(.*?)";\s*\}', text, re.MULTILINE)
        commands = {}
        for key, command in found:
            commands.setdefault(key, []).append(command.replace(r'\"', '"'))
        self.assertEqual(commands.get("Insert"), ['exec "$DF_FOUNDATION_ROOT/scripts/foundation" shell call nextMedia'])
        self.assertEqual(commands.get("Delete"), ['exec "$DF_FOUNDATION_ROOT/scripts/foundation" shell call previousMedia'])
        self.assertEqual(commands.get("Pause"), ['exec "$DF_FOUNDATION_ROOT/scripts/transparency" toggle'])
        self.assertEqual(commands.get("Shift+Pause"), ['exec "$DF_FOUNDATION_ROOT/scripts/transparency" monitor'])
        self.assertNotIn("Shift+Insert", commands)
        self.assertNotRegex(text, r'"Shift\+Insert"')

    @unittest.skipUnless(shutil.which("niri"), "Niri is unavailable")
    def test_niri_accepts_pause_and_shift_pause(self):
        result = subprocess.run(["niri", "validate", "-c", str(BINDINGS)], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    @unittest.skipUnless(shutil.which("xkbcli"), "xkbcli is unavailable")
    def test_finnish_pause_key_has_pause_keysym(self):
        result = subprocess.run(["xkbcli", "compile-keymap", "--layout", "fi"], text=True, capture_output=True, check=True)
        match = re.search(r'key <PAUS>\s*\{(.*?)\};', result.stdout, re.DOTALL)
        self.assertIsNotNone(match)
        self.assertRegex(match.group(1), r'\[\s*Pause\s*,\s*Break\s*\]')
        self.assertIn('PC_CONTROL_LEVEL2', match.group(1))


    def test_pause_keysym_remains_pause_with_shift(self):
        library = ctypes.util.find_library("xkbcommon")
        if not library:
            self.skipTest("libxkbcommon is unavailable")
        xkb = ctypes.CDLL(library)
        class Names(ctypes.Structure):
            _fields_ = [(name, ctypes.c_char_p) for name in ("rules", "model", "layout", "variant", "options")]
        xkb.xkb_context_new.argtypes = [ctypes.c_int]
        xkb.xkb_context_new.restype = ctypes.c_void_p
        xkb.xkb_keymap_new_from_names.argtypes = [ctypes.c_void_p, ctypes.POINTER(Names), ctypes.c_int]
        xkb.xkb_keymap_new_from_names.restype = ctypes.c_void_p
        xkb.xkb_state_new.argtypes = [ctypes.c_void_p]
        xkb.xkb_state_new.restype = ctypes.c_void_p
        xkb.xkb_state_key_get_one_sym.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        xkb.xkb_state_key_get_one_sym.restype = ctypes.c_uint32
        xkb.xkb_state_update_key.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int]
        xkb.xkb_state_unref.argtypes = [ctypes.c_void_p]
        xkb.xkb_keymap_unref.argtypes = [ctypes.c_void_p]
        xkb.xkb_context_unref.argtypes = [ctypes.c_void_p]
        context = xkb.xkb_context_new(0)
        self.assertTrue(context)
        keymap = xkb.xkb_keymap_new_from_names(context, ctypes.byref(Names(None, None, b"fi", None, None)), 0)
        self.assertTrue(keymap)
        state = xkb.xkb_state_new(keymap)
        self.assertTrue(state)
        try:
            # Linux KEY_PAUSE is 119; XKB keycodes add 8. Left Shift is 42 + 8.
            self.assertEqual(xkb.xkb_state_key_get_one_sym(state, 127), 0xff13)
            xkb.xkb_state_update_key(state, 50, 1)
            self.assertEqual(xkb.xkb_state_key_get_one_sym(state, 127), 0xff13)
        finally:
            xkb.xkb_state_unref(state)
            xkb.xkb_keymap_unref(keymap)
            xkb.xkb_context_unref(context)


if __name__ == "__main__":
    unittest.main()
