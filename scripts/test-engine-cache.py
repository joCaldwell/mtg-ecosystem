import fcntl
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('cache', Path(__file__).with_name('engine-cache.py'))
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


def git(*args):
    subprocess.run(['git', *args], check=True, capture_output=True)


class CacheTests(unittest.TestCase):
    def test_override_needs_no_git_or_rust_and_creates_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'uncreated'
            locks = Path(tmp) / 'locks'
            env = {'CARGO_TARGET_DIR': str(dest), 'MTG_ENGINE_LOCK_DIR': str(locks)}
            with patch.object(cache.subprocess, 'check_output', side_effect=AssertionError):
                self.assertEqual(dest, cache.cache_path(environ=env))
                self.assertEqual(locks, cache.lock_path(dest, env).parent)
            self.assertFalse(dest.exists())
            self.assertFalse(locks.exists())

    def test_each_worktree_selects_its_own_target_without_rustc(self):
        # Only the worktree's top level is consulted; toolchains share Cargo's own
        # per-rustc metadata hashing inside one target.
        with patch.object(cache.subprocess, 'check_output', side_effect=['/worktree-a\n', '/worktree-b\n', '/worktree-a\n']) as calls:
            first = cache.cache_path(Path('/worktree-a/sub'), {})
            self.assertEqual(Path('/worktree-a/target/engine'), first)
            self.assertNotEqual(first, cache.cache_path(Path('/worktree-b'), {}))
            self.assertEqual(first, cache.cache_path(Path('/worktree-a'), {}))
        self.assertTrue(all(call.args[0][:2] == ['git', '-C'] for call in calls.call_args_list))

    def test_registered_worktrees_get_distinct_targets_and_locks(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, other = Path(tmp) / 'repo', Path(tmp) / 'other'
            git('init', '-q', str(root))
            git('-C', str(root), '-c', 'commit.gpgsign=false', '-c', 'user.name=t', '-c', 'user.email=t@example.invalid',
                'commit', '-q', '--allow-empty', '-m', 'fixture')
            git('-C', str(root), 'worktree', 'add', '-q', '--detach', str(other))
            env = {'MTG_ENGINE_LOCK_DIR': str(Path(tmp) / 'locks')}
            targets = [cache.cache_path(path, env) for path in (root, other)]
            self.assertEqual([path.resolve() / 'target/engine' for path in (root, other)], targets)
            locks = [cache.lock_path(target, env) for target in targets]
            self.assertNotEqual(*locks)
            # worktree_lock ignores an explicit target: it names the checkout's own lock.
            self.assertEqual(locks[1], cache.worktree_lock(other, {**env, 'CARGO_TARGET_DIR': str(targets[0])}))
            shared = {**env, 'CARGO_TARGET_DIR': str(Path(tmp) / 'shared')}
            self.assertEqual(*[cache.lock_path(cache.cache_path(path, shared), shared) for path in (root, other)])
            (targets[1] / 'debug').mkdir(parents=True)
            (targets[1] / 'debug/binary').write_bytes(b'x' * 8192)
            rows = cache.listing(root, {**env, 'XDG_CACHE_HOME': str(Path(tmp) / 'xdg')})['worktrees']
            self.assertEqual([str(targets[1])], [row['target_dir'] for row in rows])
            self.assertGreaterEqual(rows[0]['allocated_bytes'], 8192)
            self.assertEqual('absent', rows[0]['lock_state'])

    def test_symlink_alias_shares_lock_and_probe_reports_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            actual, link = Path(tmp) / 'actual', Path(tmp) / 'link'
            actual.mkdir()
            link.symlink_to(actual, target_is_directory=True)
            env = {'MTG_ENGINE_LOCK_DIR': str(Path(tmp) / 'locks')}
            lock = cache.lock_path(actual, env)
            self.assertEqual(lock, cache.lock_path(link, env))
            self.assertEqual('absent', cache.lock_state(lock))
            self.assertFalse(lock.exists(), 'probing must not create a lock file')
            lock.parent.mkdir()
            with lock.open('a') as handle:
                self.assertEqual('idle', cache.lock_state(lock))
                fcntl.flock(handle, fcntl.LOCK_EX)
                self.assertEqual('busy', cache.lock_state(lock))
            self.assertEqual('idle', cache.lock_state(lock))

    def test_override_symlink_counts_the_actual_cache(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            actual = root / 'actual'
            actual.mkdir()
            (actual / 'binary').write_bytes(b'x' * 8192)
            link = root / 'link'
            link.symlink_to(actual, target_is_directory=True)
            report = cache.status(environ={'CARGO_TARGET_DIR': str(link), 'ENGINE_CACHE_MIN_FREE_GIB': '0', 'MTG_ENGINE_LOCK_DIR': tmp})
            self.assertGreaterEqual(report['allocated_bytes'], 8192)
            self.assertEqual(report['lock'], str(cache.lock_path(actual, {'MTG_ENGINE_LOCK_DIR': tmp})))
            self.assertEqual(report['incremental'], 'profile default')
            self.assertTrue(link.is_symlink())

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
