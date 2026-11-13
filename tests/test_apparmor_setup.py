"""Read-only boot capability detection; no real boot paths or privilege calls."""
import importlib.machinery
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
loader = importlib.machinery.SourceFileLoader('apparmor_setup_fixture', str(SCRIPTS / 'apparmor-setup'))
spec = importlib.util.spec_from_loader(loader.name, loader)
setup = importlib.util.module_from_spec(spec)
loader.exec_module(setup)


class AppArmorDetection(unittest.TestCase):
    def test_active_kernel_needs_no_grub_or_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            enabled = root / 'sys/module/apparmor/parameters/enabled'
            enabled.parent.mkdir(parents=True)
            enabled.write_text('Y\n')
            with patch.object(setup, 'Path', side_effect=lambda p: root / p.lstrip('/')), \
                    patch.object(setup, 'installed_lsms', side_effect=AssertionError('not needed')), \
                    patch.object(setup.os, 'geteuid', side_effect=AssertionError('not privileged')), \
                    patch.object(sys, 'argv', ['apparmor-setup', 'check']):
                setup.main()

    def test_check_as_normal_user_is_read_only_and_validates_lsm(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name, text in [('sys/module/apparmor/.keep', ''),
                               ('usr/bin/grub-mkconfig', 'default/grub.d/'),
                               ('boot/grub/grub.cfg', 'existing')]:
                p = root / name
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
            before = sorted(str(p.relative_to(root)) for p in root.rglob('*'))
            with patch.object(setup, 'Path', side_effect=lambda p: root / p.lstrip('/')), \
                    patch.object(setup, 'installed_lsms', return_value='landlock,apparmor,bpf') as lsms, \
                    patch.object(setup.os, 'geteuid', side_effect=AssertionError('no privilege check')), \
                    patch.object(setup.subprocess, 'run', side_effect=AssertionError('no subprocess')), \
                    patch.object(sys, 'argv', ['apparmor-setup', 'check']):
                setup.main()
                lsms.assert_called_once()
            self.assertEqual(before, sorted(str(p.relative_to(root)) for p in root.rglob('*')))
            self.assertEqual((root / 'boot/grub/grub.cfg').read_text(), 'existing')

    def test_unsupported_boot_and_missing_kernel_metadata_never_write(self):
        for failure in ['kernel', 'generator', 'mount', 'lsm']:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                if failure != 'kernel':
                    (root / 'sys/module/apparmor').mkdir(parents=True)
                if failure not in ['kernel', 'generator']:
                    p = root / 'usr/bin/grub-mkconfig'
                    p.parent.mkdir(parents=True)
                    p.write_text('default/grub.d/')
                if failure == 'lsm':
                    p = root / 'boot/grub/grub.cfg'
                    p.parent.mkdir(parents=True)
                    p.write_text('existing')
                with patch.object(setup, 'Path', side_effect=lambda p: root / p.lstrip('/')), \
                        patch.object(setup, 'STATE', root / 'state'), \
                        patch.object(setup, 'installed_lsms', side_effect=ValueError('kernel metadata unavailable')), \
                        patch.object(setup.os, 'geteuid', return_value=0), \
                        patch.object(setup.subprocess, 'run', side_effect=AssertionError('no subprocess')), \
                        patch.object(sys, 'argv', ['apparmor-setup', 'install']):
                    with self.assertRaises((RuntimeError, ValueError)):
                        setup.main()
                self.assertFalse((root / 'state').exists())
