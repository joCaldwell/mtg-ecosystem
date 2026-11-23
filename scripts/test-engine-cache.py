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

    def test_budget_failure_outside_a_cargo_cache_preserves_executable(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / 'mtg-sim'
            binary.write_bytes(b'preserved executable')
            env = {'CARGO_TARGET_DIR': tmp, 'ENGINE_CACHE_MAX_GIB': '0', 'ENGINE_CACHE_MIN_FREE_GIB': '0',
                   'MTG_ENGINE_LOCK_DIR': str(Path(tmp) / 'locks')}
            self.assertTrue(cache.status(environ=env)['errors'])
            # No CACHEDIR.TAG: not a Cargo cache this tool may evict.
            self.assertIn('not an evictable', cache.check(environ=env, locked=True)['errors'][-1])
            self.assertEqual(binary.read_bytes(), b'preserved executable')
            env['ENGINE_CACHE_MAX_GIB'] = 'nan'
            with self.assertRaises(ValueError):
                cache.status(environ=env)

    def cargo_target(self, tmp):
        root = Path(tmp)
        target = root / 'worktree/target/engine'
        (target / 'debug/incremental/mtg_core-1').mkdir(parents=True)
        (target / 'debug/incremental/mtg_core-1/s.bin').write_bytes(b'x' * 65536)
        (target / 'debug/deps').mkdir()
        (target / 'debug/deps/mtg-sim').write_bytes(b'x' * 65536)
        (target / 'release/incremental').mkdir(parents=True)
        (target / 'release/mtg-sim').write_bytes(b'x' * 65536)
        (target / 'x86_64-unknown-linux-gnu/release/deps').mkdir(parents=True)
        (target / 'x86_64-unknown-linux-gnu/release/incremental').mkdir()
        (target / 'CACHEDIR.TAG').write_text('Signature: 8a477f597d28d172789f06886806bc55\n')
        evidence = root / 'worktree/generated/receipt.json'
        evidence.parent.mkdir(parents=True)
        evidence.write_text('keep')
        env = {'CARGO_TARGET_DIR': str(target), 'ENGINE_CACHE_MIN_FREE_GIB': '0',
               'MTG_ENGINE_LOCK_DIR': str(root / 'locks')}
        return target, evidence, env

    def test_over_budget_check_evicts_in_stages_inside_the_target_only(self):
        storage = cache.storage_module()
        with tempfile.TemporaryDirectory() as tmp:
            target, evidence, env = self.cargo_target(tmp)
            total = storage.usage(target)
            incremental = storage.usage(target / 'debug/incremental')
            # Dropping incremental state alone fits: executables stay.
            env['ENGINE_CACHE_MAX_GIB'] = str((total - incremental // 2) / cache.GIB)
            report = cache.check(environ=env, locked=True)
            self.assertEqual([], report['errors'])
            self.assertEqual({'incremental'}, {row['stage'] for row in report['evicted']})
            self.assertFalse((target / 'debug/incremental').exists())
            self.assertFalse((target / 'x86_64-unknown-linux-gnu/release/incremental').exists())
            self.assertTrue((target / 'debug/deps/mtg-sim').exists())
            self.assertTrue((target / 'release/mtg-sim').exists())
            # Next, profiles the gate does not use; the gate profile survives.
            env['ENGINE_CACHE_MAX_GIB'] = str((storage.usage(target / 'debug') + 16384) / cache.GIB)
            report = cache.check(environ=env, locked=True)
            self.assertEqual([], report['errors'])
            self.assertIn('non-gate-profiles', {row['stage'] for row in report['evicted']})
            self.assertFalse((target / 'release').exists())
            self.assertTrue((target / 'debug/deps/mtg-sim').exists())
            # Finally the whole target resets; the cache tag and siblings remain.
            env['ENGINE_CACHE_MAX_GIB'] = '0'
            report = cache.check(environ=env, locked=True)
            self.assertEqual([], report['errors'])
            self.assertEqual(['CACHEDIR.TAG'], [entry.name for entry in target.iterdir()])
            self.assertEqual('keep', evidence.read_text())

    def test_busy_lock_or_symlinked_target_refuses_eviction(self):
        with tempfile.TemporaryDirectory() as tmp:
            target, _, env = self.cargo_target(tmp)
            env['ENGINE_CACHE_MAX_GIB'] = '0'
            lock = cache.lock_path(target, env)
            lock.parent.mkdir(parents=True)
            with lock.open('a') as handle:
                fcntl.flock(handle, fcntl.LOCK_EX)
                report = cache.check(environ=env)
                self.assertIn('lock is busy', report['errors'][-1])
            self.assertTrue((target / 'debug/incremental').exists())
            link = Path(tmp) / 'link'
            link.symlink_to(target, target_is_directory=True)
            report = cache.check(environ={**env, 'CARGO_TARGET_DIR': str(link)}, locked=True)
            self.assertIn('not an evictable', report['errors'][-1])
            self.assertTrue((target / 'debug/incremental').exists())
            # Without --locked, an idle lock is taken and eviction proceeds.
            self.assertEqual([], cache.check(environ=env)['errors'])
            self.assertFalse((target / 'debug').exists())


if __name__ == '__main__':
    unittest.main()
