"""Small, inert boot-planning fixtures. Never enroll, install or reboot."""
import hashlib
import importlib.util
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('boot_integration', Path(__file__).resolve().parents[1] / 'scripts/boot_integration.py')
b = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(b)
PART = '543b3d54-3878-47bf-ab50-ba0b0775c121'


def firmware(eac='0002'):
    return b.parse_nvram('BootCurrent: ' + eac + '\nBootOrder: 0000,0001,000B\n'
        'Boot0000* Limine\tHD(1,GPT,' + PART + ',0x1,0x2)/\\EFI\\limine\\limine_x64.efi\n'
        'Boot0001* Windows Boot Manager\tHD(1,GPT,' + PART + ',0x1,0x2)/File(\\EFI\\Microsoft\\Boot\\bootmgfw.efi)\n'
        'Boot' + eac + '* CachyOS EAC\tHD(1,GPT,' + PART + ',0x1,0x2)/\\EFI\\cachyos-eac\\systemd-bootx64.efi\n'
        'Boot000B* USB\tPciRoot(0)/USB(0,0)\n')


class BootPlanning(unittest.TestCase):
    def test_complete_certificate_identity_and_database_guid(self):
        der = b'fixture DER certificate'
        kind = bytes.fromhex('a159c0a5e494a74a87b5ab155c2bf072')
        record = bytes(16) + der
        data = bytes(4) + kind + struct.pack('<III', 28 + len(record), 0, len(record)) + record
        self.assertEqual(b.efi_certificates(data), [hashlib.sha256(der).hexdigest()])
        with tempfile.TemporaryDirectory() as tmp, patch.object(b, 'EFI', Path(tmp)):
            (Path(tmp) / ('db-' + b.IMAGE_SECURITY)).write_bytes(data)
            self.assertEqual(b.enrolled('db'), [hashlib.sha256(der).hexdigest()])
        for broken in (data[:-1], bytes(4) + kind + struct.pack('<III', 28, 0, 0)):
            with self.assertRaises(ValueError):
                b.efi_certificates(broken)

    def test_entry_identity_and_foreign_entries_survive(self):
        n = firmware()
        self.assertEqual(b.resolve_entry(n, PART, b.EAC_PATH), '0002')
        self.assertEqual(b.resolve_entry(n, PART, limine=True), '0000')
        self.assertIn('000B', n['entries'])
        self.assertEqual(b.selected_order(n, '0002', '0000'), ['0002', '0000', '0001', '000B'])
        new = firmware('0007')
        new['order'] = ['0007', '0000', '0001', '000B', '0020']
        self.assertEqual(b.restored_order(n, new), ['0000', '0001', '000B', '0007', '0020'])
        new['entries']['0008'] = new['entries']['0007'].copy()
        n['order'].insert(0, '0002')
        with self.assertRaises(b.Checkpoint):
            b.restored_order(n, new)

    def test_cmdline_keeps_root_encryption_and_removes_conflicting_lsms(self):
        before = 'BOOT_IMAGE=/old initrd=\\old root=UUID=test rootflags=subvol=/@ rd.luks.name=id=root apparmor=1 lsm=apparmor quiet'
        result = b.kernel_options(before)
        self.assertIn('rd.luks.name=id=root', result)
        self.assertIn('rootflags=subvol=/@', result)
        self.assertNotIn('apparmor', result)
        self.assertEqual(b.kernel_options(result), result)
        for option in b.EAC_OPTIONS:
            self.assertEqual(result.split().count(option), 1)
        with self.assertRaises(b.Checkpoint):
            b.kernel_options('root=UUID=test rootflags=subvol=/@/.snapshots/1/snapshot')

    def test_driver_replacement_covers_all_existing_kernel_consumers(self):
        additions, replacements, blockers = b.driver_plan({'nvidia-open-dkms': '1', 'xpadneo-dkms': '1'}, ['linux', 'linux-cachyos', 'linux-lts'], '8.6')
        self.assertEqual(replacements, ['nvidia-open-dkms'])
        for package in ['nvidia-open', 'nvidia-open-lts', 'linux-cachyos-nvidia-open', 'linux-cachyos-eac-nvidia-open']:
            self.assertIn(package, additions)
        self.assertTrue(any('xpadneo' in message for message in blockers))
        self.assertIn('linux-lts-headers', additions)
        self.assertIn('linux-cachyos-headers', additions)
        self.assertNotIn('linux-cachyos-eac-headers', additions)
        self.assertTrue(b.driver_plan({'nvidia-open-dkms': '1'}, ['linux-lts'], None)[2])
        self.assertTrue(b.driver_plan({'nvidia-open-dkms': '1'}, ['linux-lts'], '6.1\n8.6')[2])

    def test_eac_status_success_on_non_eac_boot_is_not_verification(self):
        facts = {'nvram': firmware(), 'esp': {'partuuid': PART}, 'kernel': '6.18.1-lts'}
        with patch.object(b, 'command', return_value=subprocess.CompletedProcess([], 0, 'All checks passed.', '')) as call:
            with self.assertRaises(b.Checkpoint):
                b.verified_boot(facts)
            self.assertFalse(any('/usr/bin/eac-status' in args[0] for args in call.call_args_list))

    def test_warnings_or_wrong_loader_never_claim_verified(self):
        facts = {'nvram': firmware(), 'esp': {'partuuid': PART}, 'kernel': '7.3.3-2-cachyos-eac'}
        status = 'Product: systemd-boot 262\nLoader: /boot/EFI/cachyos-eac/systemd-bootx64.efi\nCurrent Entry: cachyos-eac.conf\n'
        for output in ('All checks passed.\n[WARN] incomplete', '[FAIL] TPM', 'The boot files are ready.'):
            with patch.object(b, 'verify_running_image'), patch.object(b, 'command', side_effect=[subprocess.CompletedProcess([], 0, status, ''), subprocess.CompletedProcess([], 0, output, '')]) as call:
                with self.assertRaises(b.Checkpoint):
                    b.verified_boot(facts)
                self.assertEqual(call.call_count, 2)

    def test_existing_boot_attempt_never_queues_a_reboot_loop(self):
        class Transaction:
            data = {'attempt': {'boot_id': 'same', 'method': 'next'}}
            def checkpoint(self, name, text):
                raise b.Checkpoint(text)
        with patch.object(b, 'command', side_effect=AssertionError('No mutation on retry')):
            with self.assertRaises(b.Checkpoint):
                b.select_boot(Transaction(), {'boot_id': 'same'}, 'next')

    def test_existing_loader_preferences_are_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            esp = Path(tmp)
            target = esp / 'loader/loader.conf'
            target.parent.mkdir()
            target.write_text('editor no\nconsole-mode keep\ntimeout 9\ndefault old.conf\n')
            def write(path, text, mode):
                path.parent.mkdir(exist_ok=True, parents=True)
                path.write_text(text)
            with patch.object(b, 'signatures', return_value=(esp/'kernel', esp/'initrd', esp/'loader')), patch.object(b, 'atomic', side_effect=write):
                b.boot_entries({'esp': {'target': tmp}, 'cmdline': 'root=UUID=test'})
            self.assertIn('editor no', target.read_text())
            self.assertIn('console-mode keep', target.read_text())
            self.assertNotIn('old.conf', target.read_text())

    def test_another_operating_system_bootnext_is_preserved(self):
        class Transaction:
            data = {}
            def checkpoint(self, name, text):
                raise b.Checkpoint(text)
        n = firmware()
        n['next'] = '0001'
        with patch.object(b, 'ensure_entries', return_value=n), patch.object(b, 'command', side_effect=AssertionError('No firmware write')):
            with self.assertRaisesRegex(b.Checkpoint, 'unrelated BootNext'):
                b.select_boot(Transaction(), {'esp': {'partuuid': PART}, 'boot_id': 'test'}, 'next')

    def test_refresh_route_has_no_enrollment_or_firmware_mutation(self):
        identity = {'root_uuid': 'root', 'esp_partuuid': PART}
        class Transaction:
            data = {'identity': identity, 'phase': 'ready', 'timeout': 3}
            def __enter__(self): return self
            def __exit__(self, *args): return False
            def phase(self, name, operation): operation()
            def save(self): pass
        facts = {'root': {'uuid': 'root'}, 'esp': {'partuuid': PART}, 'nvram': firmware()}
        with patch.object(b.sys, 'argv', ['boot', 'refresh']), patch.object(b, 'Transaction', return_value=Transaction()), patch.object(b, 'scan', return_value=facts), \
                patch.object(b, 'signatures'), patch.object(b, 'validate_modules'), patch.object(b, 'sign_lts'), patch.object(b, 'boot_entries') as entries, \
                patch.object(b, 'recovery_verify'), patch.object(b, 'run_upstream', side_effect=AssertionError('No enrollment')), \
                patch.object(b, 'command', side_effect=AssertionError('No package or firmware calls')):
            self.assertEqual(b.main(), 0)
            self.assertEqual(entries.call_args.args[1], 3)


class BootJournal(unittest.TestCase):
    def test_failed_phase_is_durable_and_original_backups_never_rotate(self):
        with tempfile.TemporaryDirectory() as tmp:
            state = Path(tmp) / 'state'
            target = Path(tmp) / 'entry.conf'
            target.write_text('original')
            with patch.object(b, 'STATE', state), patch.object(b.os, 'geteuid', return_value=0), patch.object(b, 'protected'), patch.object(b, 'safe_parents'):
                tx = b.Transaction()
                tx.data = {'version': 1, 'phase': 'preflight'}
                tx.backup(target)
                target.write_text('changed')
                tx.backup(target)
                backup = Path(tx.data['files'][str(target)]['backup'])
                self.assertEqual(backup.read_text(), 'original')
                def fail():
                    self.assertEqual(__import__('json').loads(tx.journal_path.read_text())['pending'], 'fixture')
                    raise RuntimeError('interrupted')
                with self.assertRaises(RuntimeError):
                    tx.phase('fixture', fail)
                saved = __import__('json').loads(tx.journal_path.read_text())
                self.assertEqual(saved['pending'], 'fixture')
                self.assertEqual(saved['error'], 'interrupted')
                self.assertNotIn('fixture', saved.get('completed', []))
                with self.assertRaises(BlockingIOError):
                    b.Transaction()
                backup.write_text('corrupt')
                with self.assertRaises(RuntimeError):
                    tx.backup(target)
                tx.lock.close()

    def test_symlink_target_is_never_backed_up(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / 'link'
            target.symlink_to('/etc/passwd')
            tx = object.__new__(b.Transaction)
            tx.data = {}
            with self.assertRaises(b.Checkpoint):
                tx.backup(target)


if __name__ == '__main__':
    unittest.main()
