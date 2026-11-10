import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('worktrees', Path(__file__).with_name('engine-worktrees.py'))
w = importlib.util.module_from_spec(spec)
spec.loader.exec_module(w)


class RetirementTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / 'repo'
        self.remote = Path(self.tmp.name) / 'remote.git'
        self.tree = Path(self.tmp.name) / 'task space\nline'
        self.lock = Path(self.tmp.name) / 'lock'
        self.run_git('init', '-q', '-b', 'integration', str(self.root), root=False)
        self.run_git('config', 'user.email', 'test@example.invalid')
        self.run_git('config', 'user.name', 'Test')
        (self.root / '.gitignore').write_text('generated/\n')
        (self.root / 'source').write_text('source')
        self.run_git('add', '.')
        self.run_git('commit', '-qm', 'baseline')
        self.run_git('init', '--bare', '-q', str(self.remote), root=False)
        self.run_git('remote', 'add', 'origin', str(self.remote))
        self.run_git('push', '-q', 'origin', 'integration')
        self.run_git('worktree', 'add', '-qb', 'task', str(self.tree))
        self.head = w.git(self.root, 'rev-parse', 'HEAD').decode().strip()
        self.activity = patch.object(w.storage, 'active_processes', return_value={'active': [], 'uninspectable': []})
        self.activity.start()
        self.addCleanup(self.activity.stop)

    def run_git(self, *args, root=True):
        subprocess.run(['git', *(['-C', str(self.root)] if root else []), *args], check=True, capture_output=True)

    def retire(self, **kw):
        return w.retire(self.root, self.tree, 'integration', remote_branch='integration', lock=self.lock, **kw)

    def apply(self, **kw):
        return self.retire(apply=True, expected_head=self.head, inactive=True, **kw)

    def test_dry_run_then_remove_preserves_branch_and_published_source(self):
        self.assertEqual(self.retire()['blockers'], [])
        self.assertTrue(self.tree.is_dir())
        self.apply()
        self.assertFalse(self.tree.exists())
        self.assertEqual(w.git(self.root, 'rev-parse', 'task').decode().strip(), self.head)
        self.assertIn(self.head.encode(), w.git(self.root, 'ls-remote', 'origin', 'refs/heads/integration'))

    def test_modified_untracked_and_ignored_each_block_removal(self):
        for name in ('source', 'untracked', 'generated/evidence'):
            with self.subTest(name=name):
                p = self.tree / name
                p.parent.mkdir(exist_ok=True)
                old = p.read_bytes() if p.exists() else None
                p.write_text('preserve')
                with self.assertRaisesRegex(ValueError, 'blocked'):
                    self.apply()
                self.assertTrue(p.exists())
                if old is not None: p.write_bytes(old)
                else: p.unlink()
                if name.startswith('generated/'): p.parent.rmdir()

    def test_unique_commit_is_preserved(self):
        (self.tree / 'source').write_text('new')
        subprocess.run(['git', '-C', str(self.tree), 'commit', '-qam', 'unique'], check=True)
        self.head = w.git(self.tree, 'rev-parse', 'HEAD').decode().strip()
        with self.assertRaisesRegex(ValueError, 'unique commits'):
            self.apply()

    def test_stale_remote_proof_blocks(self):
        self.run_git('push', '-q', 'origin', ':integration')
        with self.assertRaisesRegex(ValueError, 'live published'):
            self.apply()

    def test_expected_identity_and_owner_ack_required(self):
        with self.assertRaisesRegex(ValueError, 'expect-head'):
            self.retire(apply=True, expected_head='0'*40, inactive=True)
        with self.assertRaisesRegex(ValueError, 'inactive'):
            self.retire(apply=True, expected_head=self.head)

    def test_active_and_uninspectable_processes(self):
        with patch.object(w.storage, 'active_processes', return_value={'active': [1], 'uninspectable': []}):
            with self.assertRaisesRegex(ValueError, 'active process'):
                self.apply()
        with patch.object(w.storage, 'active_processes', return_value={'active': [], 'uninspectable': [2]}):
            with self.assertRaisesRegex(ValueError, 'Uninspectable'):
                self.apply()
            self.apply(allow_uninspectable=True)

    def test_documented_reference_blocks(self):
        (self.root / 'docs').mkdir()
        (self.root / 'docs/proof.md').write_text(str(self.tree))
        self.run_git('add', 'docs')
        with self.assertRaisesRegex(ValueError, 'documented'):
            self.apply()

    def test_primary_locked_and_unregistered_paths_block(self):
        with self.assertRaisesRegex(ValueError, 'primary'):
            w.retire(self.root, self.root, 'integration', remote_branch='integration', lock=self.lock,
                     apply=True, expected_head=self.head, inactive=True)
        self.run_git('worktree', 'lock', str(self.tree))
        with self.assertRaisesRegex(ValueError, 'locked'):
            self.apply()
        with self.assertRaisesRegex(ValueError, 'registered'):
            w.plan(self.root, self.tree / 'nested', 'integration')

    def test_symlink_root_refused(self):
        alias = Path(self.tmp.name) / 'alias'
        alias.symlink_to(self.tree)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            w.plan(self.root, alias, 'integration')


if __name__ == '__main__':
    unittest.main()
