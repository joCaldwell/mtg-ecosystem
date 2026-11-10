import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('cache', Path(__file__).with_name('engine-cache.py'))
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


class CacheTests(unittest.TestCase):
    def test_override_needs_no_git_or_rust_and_creates_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'uncreated'
            with patch.object(cache.subprocess, 'check_output', side_effect=AssertionError):
                self.assertEqual(dest, cache.cache_path(environ={'CARGO_TARGET_DIR': str(dest)}))
            self.assertFalse(dest.exists())

    def test_shared_repository_and_toolchain_qualification(self):
        with patch.object(cache.subprocess, 'check_output', side_effect=['/repo/.git\n', 'rustc A host X', '/repo/.git\n', 'rustc A host X', '/repo/.git\n', 'rustc B host X']):
            first = cache.cache_path(Path('/worktree-a'), {'XDG_CACHE_HOME': '/cache'})
            self.assertEqual(first, cache.cache_path(Path('/worktree-b'), {'XDG_CACHE_HOME': '/cache'}))
            self.assertNotEqual(first, cache.cache_path(Path('/worktree-b'), {'XDG_CACHE_HOME': '/cache'}))

    def test_budget_failure_preserves_executable(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / 'mtg-sim'
            binary.write_bytes(b'preserved executable')
            env = {'CARGO_TARGET_DIR': tmp, 'ENGINE_CACHE_MAX_GIB': '0', 'ENGINE_CACHE_MIN_FREE_GIB': '0'}
            self.assertTrue(cache.status(environ=env)['errors'])
            self.assertEqual(binary.read_bytes(), b'preserved executable')
            env['ENGINE_CACHE_MAX_GIB'] = 'nan'
            with self.assertRaises(ValueError):
                cache.status(environ=env)


if __name__ == '__main__':
    unittest.main()
