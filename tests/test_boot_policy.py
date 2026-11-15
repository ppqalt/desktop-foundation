import sys
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from boot_policy import kernel_lsms, fallback_preset, remove_plymouth

class BootPolicyTests(unittest.TestCase):
    def test_preserves_kernel_lsms_and_rejects_unsupported_kernels(self):
        config = 'CONFIG_SECURITY_APPARMOR=y\nCONFIG_LSM="landlock,lockdown,yama,integrity,bpf"\n'
        self.assertEqual(kernel_lsms([config, config]), 'landlock,lockdown,yama,integrity,apparmor,bpf')
        with self.assertRaises(ValueError): kernel_lsms([config.replace('APPARMOR=y', 'APPARMOR=n')])
        with self.assertRaises(ValueError): kernel_lsms([config, config.replace('yama,', '')])
        with self.assertRaises(ValueError): kernel_lsms([config.replace('integrity', 'selinux')])
    def test_fallback_is_idempotent_and_preserves_image_names(self):
        text = 'PRESETS=(\'default\')\n#fallback_image="/boot/initramfs-other-fallback.img"\n#fallback_options="-S autodetect"\n'
        changed = fallback_preset(text)
        self.assertIn('/boot/initramfs-other-fallback.img', changed)
        self.assertEqual(fallback_preset(changed), changed)
        with self.assertRaises(ValueError): fallback_preset(text.replace("'default'", "'custom'"))
    def test_removes_only_plymouth_and_rejects_computed_hooks(self):
        self.assertEqual(remove_plymouth('HOOKS=(base systemd plymouth sd-encrypt filesystems)\n'), 'HOOKS=(base systemd sd-encrypt filesystems)\n')
        with self.assertRaises(ValueError): remove_plymouth('HOOKS=($CUSTOM plymouth)\n')

class PlymouthSetupTests(unittest.TestCase):
    def test_effective_policy_preserves_encryption_and_later_dropins(self):
        import runpy
        import subprocess
        module = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'scripts/plymouth-setup'))
        source = 'HOOKS=(base systemd kms keyboard sd-vconsole plymouth sd-encrypt filesystems)\n'
        policy = module['configuration'](source)
        script = source + 'HOOKS=(${HOOKS[@]/kms/})\nHOOKS+=(sd-btrfs-overlayfs)\n' + policy + 'printf "%s\\n" "${HOOKS[@]}"'
        hooks = subprocess.check_output(['bash', '-c', script], text=True).splitlines()
        self.assertEqual(hooks, ['base', 'systemd', 'keyboard', 'sd-vconsole', 'sd-encrypt', 'filesystems', 'sd-btrfs-overlayfs'])
        again = subprocess.check_output(['bash', '-c', script.replace(policy, policy + policy)], text=True)
        self.assertEqual(again.splitlines(), hooks)

    def test_busybox_unlock_is_replaced_not_removed(self):
        import runpy
        import subprocess
        module = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'scripts/plymouth-setup'))
        source = 'HOOKS=(base udev keyboard plymouth plymouth-encrypt filesystems)\n'
        output = subprocess.check_output(['bash', '-c', source + module['configuration'](source) + 'printf "%s\\n" "${HOOKS[@]}"'], text=True)
        self.assertEqual(output.splitlines(), ['base', 'udev', 'keyboard', 'encrypt', 'filesystems'])

    def test_image_verification_rejects_stale_plymouth_and_missing_console_agent(self):
        import importlib.machinery
        import importlib.util
        from unittest.mock import patch
        path = Path(__file__).resolve().parents[1] / 'scripts/plymouth-setup'
        loader = importlib.machinery.SourceFileLoader('plymouth_setup_test', str(path))
        spec = importlib.util.spec_from_loader(loader.name, loader)
        module = importlib.util.module_from_spec(spec)
        loader.exec_module(module)
        with patch.object(module, 'images', return_value=[Path('/boot/initramfs-linux.img')]):
            with patch.object(module, 'run', return_value='usr/bin/plymouth\n'):
                with self.assertRaisesRegex(ValueError, 'Plymouth remains'):
                    module.verify()
            with patch.object(module, 'run', return_value='usr/lib/systemd/systemd-cryptsetup\n'):
                with self.assertRaisesRegex(ValueError, 'console password agent'):
                    module.verify()
            with patch.object(module, 'run', return_value='usr/lib/systemd/systemd-cryptsetup\nusr/lib/systemd/system/systemd-ask-password-console.service\n'):
                module.verify()
