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
        self.lock = Path(self.temp.name) / 'cargo.lock'

    def tearDown(self):
        self.temp.cleanup()

    def test_symlink_ancestor_and_nested_symlink_refused(self):
        link = Path(self.temp.name) / 'linked'
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            storage.no_symlinks(link / 'target')
        outside = Path(self.temp.name) / 'unique-source'
        outside.write_text('keep')
        (self.cache / 'link').symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            storage.no_symlinks(self.root / 'target', recursive=True)
        self.assertEqual(outside.read_text(), 'keep')

    def test_busy_cargo_lock_refused_and_default_lock_is_the_worktree_target_lock(self):
        with storage.cargo_lock(self.lock):
            with self.assertRaisesRegex(RuntimeError, 'busy'):
                with storage.cargo_lock(self.lock):
                    pass
        with patch.dict(os.environ, {'MTG_ENGINE_LOCK_DIR': str(Path(self.temp.name) / 'locks')}):
            self.assertEqual(Path(self.temp.name) / 'locks', storage.worktree_lock(self.root).parent)

    def test_open_file_counts_as_activity(self):
        # Activity need not have a cwd/executable in target: open fds count too.
        with (self.cache / 'cache').open() as handle:
            child = subprocess.Popen(['sleep', '30'], pass_fds=(handle.fileno(),))
            try:
                self.assertIn(child.pid, storage.active_processes(self.root / 'target')['active'])
            finally:
                child.terminate()
                child.wait()

    def test_inventory_does_not_follow_links(self):
        outside = Path(self.temp.name) / 'large'
        outside.write_bytes(b'x' * 16384)
        before = storage.usage(self.cache)
        (self.cache / 'link').symlink_to(outside)
        self.assertLess(storage.usage(self.cache) - before, 16384)
        rows = storage.inventory(self.root)['rows']
        self.assertIn(str(self.root / 'target'), [row['path'] for row in rows])


if __name__ == '__main__':
    unittest.main()
