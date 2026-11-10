import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('storage', Path(__file__).with_name('engine-storage.py'))
storage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(storage)


class StorageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / 'repo'
        self.root.mkdir()
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.cache = self.root / 'target/debug/incremental'
        self.cache.mkdir(parents=True)
        (self.cache / 'cache').write_bytes(b'x' * 8192)
        self.binary = self.root / 'target/debug/mtg-sim'
        self.binary.write_bytes(b'preserve executable')
        self.lock = Path(self.temp.name) / 'cargo.lock'

    def tearDown(self):
        self.temp.cleanup()

    def prune(self, apply=False):
        return storage.prune_incremental(self.root, 0, apply, self.lock, allow_uninspectable=True)

    def test_dry_run_and_budget_then_apply_preserves_executable(self):
        self.assertEqual(storage.incremental_plan(self.root, 1024**2), [])
        report = self.prune()
        self.assertGreater(report['candidate_bytes'], 0)
        self.assertTrue(self.cache.exists())
        report = self.prune(True)
        self.assertGreater(report['reclaimed_bytes'], 0)
        self.assertFalse(self.cache.exists())
        self.assertEqual(self.binary.read_bytes(), b'preserve executable')

    def test_unregistered_nested_path_rejected(self):
        with self.assertRaises(ValueError):
            storage.prune_incremental(self.root / 'target', 0, True, self.lock)
        self.assertTrue(self.cache.exists())

    def test_tracked_cache_refused(self):
        subprocess.run(['git', '-C', str(self.root), 'add', 'target'], check=True)
        with self.assertRaisesRegex(ValueError, 'Tracked'):
            self.prune(True)
        self.assertTrue(self.cache.exists())

    def test_symlink_inside_cache_refused(self):
        outside = Path(self.temp.name) / 'unique-source'
        outside.write_text('keep')
        (self.cache / 'link').symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            self.prune(True)
        self.assertEqual(outside.read_text(), 'keep')

    def test_symlink_target_and_ancestor_refused(self):
        link = Path(self.temp.name) / 'linked'
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            storage.prune_incremental(link, 0, True, self.lock)
        self.assertTrue(self.cache.exists())

    def test_busy_cargo_lock_refused(self):
        with storage.cargo_lock(self.lock):
            with self.assertRaisesRegex(RuntimeError, 'busy'):
                self.prune(True)
        self.assertTrue(self.cache.exists())

    def test_active_open_file_refused(self):
        # Activity need not have a cwd/executable in target: open fds count too.
        with (self.cache / 'cache').open() as handle:
            child = subprocess.Popen(['sleep', '30'], pass_fds=(handle.fileno(),))
            try:
                with self.assertRaisesRegex(RuntimeError, 'active'):
                    self.prune(True)
            finally:
                child.terminate()
                child.wait()
        self.assertTrue(self.cache.exists())

    def test_uninspectable_activity_requires_explicit_acknowledgement(self):
        with patch.object(storage, 'active_processes', return_value={'active': [], 'uninspectable': [42]}):
            with self.assertRaisesRegex(RuntimeError, 'Cannot inspect'):
                storage.prune_incremental(self.root, 0, True, self.lock)
        self.assertTrue(self.cache.exists())

    def test_target_symlink_refused(self):
        target = self.root / 'target'
        outside = Path(self.temp.name) / 'outside'
        target.rename(outside)
        target.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            self.prune(True)
        self.assertTrue((outside / 'debug/incremental/cache').exists())

    def test_inventory_does_not_follow_links(self):
        outside = Path(self.temp.name) / 'large'
        outside.write_bytes(b'x' * 16384)
        before = storage.usage(self.cache)
        (self.cache / 'link').symlink_to(outside)
        self.assertLess(storage.usage(self.cache) - before, 16384)


if __name__ == '__main__':
    unittest.main()
