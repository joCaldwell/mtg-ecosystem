import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).with_name('run-retention.py')
spec = importlib.util.spec_from_file_location('retention', SCRIPT)
retention = importlib.util.module_from_spec(spec)
spec.loader.exec_module(retention)


class RetentionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / 'repo'
        self.root.mkdir()
        self.store = Path(self.temp.name) / 'evidence'
        self.store.mkdir()
        self.scratch = self.root / 'generated/.retention-work'
        self.lock = Path(self.temp.name) / 'cargo.lock'

    def tearDown(self):
        self.temp.cleanup()

    def run_worker(self, script, **kwargs):
        return retention.supervise([sys.executable, '-c', script], self.root, self.store,
                                   reserve=0, interval=0.01, lock=self.lock, **kwargs)

    def test_success_discards_scratch_preserves_evidence(self):
        evidence = self.store / 'source'
        evidence.write_text('unique source')
        result = self.run_worker("import os,pathlib;pathlib.Path(os.environ['MTG_RETENTION_SCRATCH'],'cache').write_bytes(b'x'*65536)")
        self.assertEqual(result['status'], 'worker_completed')
        self.assertFalse(self.scratch.exists())
        self.assertEqual(evidence.read_text(), 'unique source')

    def test_inherited_target_and_artifact_paths_cannot_bypass_scratch(self):
        outside = self.root / 'shared-cache'
        with patch.dict(os.environ, {'CARGO_TARGET_DIR': str(outside), 'MTG_ENGINE_ARTIFACT_ROOT': str(outside)}):
            self.run_worker("import os,pathlib;root=pathlib.Path(os.environ['MTG_RETENTION_SCRATCH']);assert pathlib.Path(os.environ['CARGO_TARGET_DIR']).parent==root;assert pathlib.Path(os.environ['MTG_ENGINE_ARTIFACT_ROOT']).parent==root")
        self.assertFalse(outside.exists())
        self.assertFalse(self.scratch.exists())

    def test_failure_discards_scratch_preserves_partial_evidence(self):
        evidence = self.store / 'failed.log'
        with self.assertRaisesRegex(RuntimeError, 'exited 7'):
            self.run_worker(f"import pathlib,sys;pathlib.Path({str(evidence)!r}).write_text('original failure');sys.exit(7)")
        self.assertFalse(self.scratch.exists())
        self.assertEqual(evidence.read_text(), 'original failure')
        status = json.loads((self.root / 'generated/retention-storage-latest.json').read_text())
        self.assertEqual(status['status'], 'failed')

    def test_store_admission_refuses_before_worker(self):
        marker = self.root / 'ran'
        (self.store / 'old-evidence').write_bytes(b'x'*8192)
        with self.assertRaisesRegex(RuntimeError, 'Evidence store exceeds'):
            self.run_worker(f"import pathlib;pathlib.Path({str(marker)!r}).touch()", store_limit=1)
        self.assertFalse(marker.exists())
        self.assertFalse(self.scratch.exists())
        self.assertTrue((self.store / 'old-evidence').exists())

    def test_runtime_scratch_limit_stops_writer(self):
        with self.assertRaisesRegex(RuntimeError, 'scratch exceeds'):
            self.run_worker("import os,pathlib,time;pathlib.Path(os.environ['MTG_RETENTION_SCRATCH'],'large').write_bytes(b'x'*1048576);time.sleep(30)", scratch_limit=65536)
        self.assertFalse(self.scratch.exists())

    def test_runtime_store_limit_preserves_failed_output(self):
        output = self.store / 'new-evidence'
        with self.assertRaisesRegex(RuntimeError, 'Evidence store exceeds'):
            self.run_worker(f"import pathlib,time;pathlib.Path({str(output)!r}).write_bytes(b'x'*1048576);time.sleep(30)", store_limit=65536)
        self.assertEqual(output.stat().st_size, 1048576)
        self.assertFalse(self.scratch.exists())

    def test_spawn_failure_cleans_scratch(self):
        with self.assertRaises(FileNotFoundError):
            retention.supervise([str(self.root / 'missing-worker')], self.root, self.store,
                                reserve=0, lock=self.lock)
        self.assertFalse(self.scratch.exists())

    def test_busy_lock_refuses_before_creating_scratch(self):
        with retention.storage.cargo_lock(self.lock):
            with self.assertRaisesRegex(RuntimeError, 'busy'):
                self.run_worker('raise AssertionError()')
        self.assertFalse(self.scratch.exists())

    def test_free_space_reserve_refuses_before_worker(self):
        with patch.object(retention.shutil, 'disk_usage', return_value=type('Disk', (), {'free': 0})()):
            with self.assertRaisesRegex(RuntimeError, 'free bytes'):
                retention.budget_check(self.root, self.store, 100000, 100000, 1)

    def test_existing_scratch_blocks_and_preserves(self):
        self.scratch.mkdir(parents=True)
        (self.scratch / 'abandoned').write_text('inspect me')
        with self.assertRaises(FileExistsError):
            self.run_worker('raise AssertionError()')
        self.assertEqual((self.scratch / 'abandoned').read_text(), 'inspect me')

    def test_symlink_scratch_refused(self):
        self.scratch.parent.mkdir(parents=True)
        self.scratch.symlink_to(self.store, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            self.run_worker('raise AssertionError()')
        self.assertTrue(self.store.exists())

    def test_signal_stops_descendants_before_cleanup(self):
        ready = self.store / 'ready.json'
        worker = self.root / 'worker.py'
        worker.write_text("import subprocess,sys,os,json,time\nfrom pathlib import Path\n"
                          "child=subprocess.Popen([sys.executable,'-c','import time;time.sleep(30)'])\n"
                          f"Path({str(ready)!r}).write_text(json.dumps([os.getpid(),child.pid]))\n"
                          "time.sleep(30)\n")
        driver = self.root / 'driver.py'
        driver.write_text(f"import runpy,sys\nm=runpy.run_path({str(SCRIPT.resolve())!r})\n"
                          f"m['supervise']([sys.executable,{str(worker)!r}],{str(self.root)!r},{str(self.store)!r},reserve=0,interval=.01,lock=m['Path']({str(self.lock)!r}))\n")
        process = subprocess.Popen([sys.executable, str(driver)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(ready.exists(), 'worker started')
            pids = json.loads(ready.read_text())
            process.send_signal(signal.SIGTERM)
            self.assertNotEqual(process.wait(timeout=5), 0)
            self.assertFalse(self.scratch.exists())
            for pid in pids:
                status = Path(f'/proc/{pid}/status')
                if status.exists():
                    self.assertIn('State:\tZ', status.read_text(), 'descendant is not running')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


if __name__ == '__main__':
    unittest.main()
