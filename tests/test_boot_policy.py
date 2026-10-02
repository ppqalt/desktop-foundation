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
