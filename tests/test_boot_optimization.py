import importlib.machinery
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
loader = importlib.machinery.SourceFileLoader('boot_optimize', str(ROOT / 'scripts/boot-optimize'))
spec = importlib.util.spec_from_loader(loader.name, loader)
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)
sys.path.insert(0, str(ROOT / 'scripts'))
report_loader = importlib.machinery.SourceFileLoader('boot_report', str(ROOT / 'scripts/boot-report.py'))
report_spec = importlib.util.spec_from_loader(report_loader.name, report_loader)
report = importlib.util.module_from_spec(report_spec)
report_loader.exec_module(report)


class BootOptimizationTests(unittest.TestCase):
    def test_stage_times_parse_minutes_and_subseconds(self):
        self.assertAlmostEqual(report.seconds('1min 2.345s'), 62.345)
        self.assertAlmostEqual(report.seconds('384ms'), .384)

    def test_timeout_preserves_unrelated_policy(self):
        before = 'GRUB_TIMEOUT=\'5\'\nGRUB_DEFAULT=0\nGRUB_CMDLINE_LINUX="apparmor=1 console=tty8"\n'
        after = module.replace_timeout(before)
        self.assertEqual(after, before.replace("GRUB_TIMEOUT='5'", 'GRUB_TIMEOUT=1'))
        with self.assertRaises(ValueError):
            module.replace_timeout('GRUB_DEFAULT=0\n')

    def test_menu_rejects_lost_os_security_or_fallback(self):
        records = ('menuentry \'CachyOS\' {\nlinux /vmlinuz apparmor=1 console=tty8\n'
                   'initrd /fallback.img\n}\nmenuentry \'X-01\' {\nchainloader /external.efi\n}\n')
        before = 'set timeout=3\nset timeout_style=menu\n' + records
        after = before.replace('timeout=3', 'timeout=1')
        module.validate_menu(before, after)
        for damaged in (after.replace('apparmor=1 ', ''), after.replace('initrd /fallback.img\n', ''),
                        after.replace("menuentry 'X-01' {\n", ''), after.replace('timeout=1', 'timeout=0.25')):
            with self.assertRaises(ValueError):
                module.validate_menu(before, damaged)

    def test_failed_candidate_restores_files_and_removes_new_mask(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            base, grub, wine, state = [root / name for name in ('grub-default', 'grub.cfg', 'wine.conf', 'state')]
            base.write_text('GRUB_TIMEOUT=3\n')
            grub.write_text("set timeout=3\nmenuentry 'Recovery' {\ninitrd /fallback.img\n}\n")
            original = grub.read_text()

            def command(*args):
                if args[0] == 'grub-mkconfig':
                    Path(args[-1]).write_text('set timeout=1\nset timeout_style=menu\n')
                return ''

            with patch.object(module, 'STATE', state), patch.object(module, 'JOURNAL', state / 'manifest.json'), \
                    patch.object(module, 'GRUB', grub), patch.object(module, 'WINE', wine), \
                    patch.object(module, 'plan', return_value={base: 'GRUB_TIMEOUT=1\n', wine: None}), \
                    patch.object(module, 'run', side_effect=command), patch.object(module.os, 'geteuid', return_value=0), \
                    patch.object(sys, 'argv', ['boot-optimize', 'install']):
                with self.assertRaisesRegex(ValueError, 'lost/changed'):
                    module.main()
            self.assertEqual(base.read_text(), 'GRUB_TIMEOUT=3\n')
            self.assertEqual(grub.read_text(), original)
            self.assertFalse(wine.is_symlink())
            self.assertFalse(state.exists())
