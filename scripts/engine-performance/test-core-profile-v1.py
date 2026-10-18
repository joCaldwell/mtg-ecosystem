#!/usr/bin/env python3
"""Small fixtures only; never invokes gprofng/native/systemd."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
P=Path(__file__).absolute().parent
s=importlib.util.spec_from_file_location('profile_driver',P/'run-core-profile-v1.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m)

def authority(mode='ordinary_core'):
    return {'profiling':{'mode':mode,'family':None if mode=='ordinary_core' else 'kernel','case':None if mode=='ordinary_core' else 'dense','collection':'cpu','collection_flags':list(m.COLLECTIONS['cpu'])},'timeout_seconds':120,'cpu_ids':[0],'test_executable':{'path':'/private/pinned/native'},'engine_build_id':'a'*64}

class Ordinary:
    def legal_semantics(self,value):return []
    def catalog_load(self,value,fixture):return {'scope':'fixture'}

def native():
    return {'schema':'ordinary-core-profile-v1','status':'passed','error':None,'build_id':'a'*64,'attribution_function':m.ATTRIBUTION,'scope':'precloned inputs; total signal-bookend overhead included','reference_semantics':{'inputs':1024,'final_state_sha256':'b'*64,'journal_sha256':'c'*64},'catalog_load':{},'accepted_inputs':1024,'advance_calls':1024,'core_ns':1000}

class ProfileGuards(unittest.TestCase):
    def test_fixed_flags_and_paused_vs_whole_harness_modes(self):
        a=authority();cmd,entry=m.native_command(a,Path('/private/p'))
        self.assertEqual(entry,m.ORDINARY_ENTRY);self.assertIn('-y',cmd);self.assertEqual(cmd[cmd.index('-F')+1],'off');self.assertEqual(cmd[cmd.index('-a')+1],'off');self.assertEqual(cmd[cmd.index('-H')+1],'off')
        dense=authority('dense');cmd,entry=m.native_command(dense,Path('/private/p'))
        self.assertNotIn('-y',cmd);self.assertIn('dense_kernel_diagnostic_host_adapter',entry)
        for key,value in [('collection_flags',['-F','on']),('family','kernel'),('case','dense')]:
            bad=authority();bad['profiling'][key]=value
            with self.assertRaises(ValueError):m.mode_guard(bad)
    def test_cpu_and_heap_are_distinct_predeclared_captures(self):
        a=authority();m.mode_guard(a)
        heap=copy.deepcopy(a);heap['profiling'].update(collection='heap',collection_flags=m.COLLECTIONS['heap'])
        command,_=m.native_command(heap,Path('/private/p'))
        self.assertEqual(command[command.index('-H')+1],'on');self.assertEqual(command[command.index('-p')+1],'off')
        for value in [dict(collection='combined'),dict(collection='cpu',collection_flags=m.COLLECTIONS['heap']),dict(collection='heap',collection_flags=m.COLLECTIONS['cpu'])]:
            bad=copy.deepcopy(a);bad['profiling'].update(value)
            with self.assertRaises(ValueError):m.mode_guard(bad)


    def test_wrong_authority_pin_rejects_before_any_script_import(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'authority.json';p.write_text('{}')
            with patch.object(m,'load') as load:
                with self.assertRaises(ValueError):m.authority_guard(p,'0'*64)
                load.assert_not_called()
    def test_helper_rejects_wrong_hash_before_import_even_optimized(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);evil=root/'evil.py';marker=root/'side-effect';evil.write_text(f'from pathlib import Path\nPath({str(marker)!r}).touch()\n')
            code=f"import importlib.util;from pathlib import Path;s=importlib.util.spec_from_file_location('m',{str(P/'run-core-profile-v1.py')!r});m=importlib.util.module_from_spec(s);s.loader.exec_module(m);m.HERE=Path({tmp!r});m.load('evil.py',{{'host_scripts':{{{str(evil)!r}:'{'0'*64}'}}}})"
            p=subprocess.run([sys.executable,'-O','-c',code],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=3)
            self.assertNotEqual(p.returncode,0);self.assertFalse(marker.exists())
    def test_banner_binds_owned_pid_and_exact_experiment(self):
        exp=Path('/private/capture.er');prefix=f'Creating experiment directory {exp} (Process ID: 123) ...\n';payload='running 1 test\n'
        self.assertEqual(m.capture_text(prefix+payload,exp,123),payload)
        for value,pid in [(prefix,124),(prefix.replace('123','0123'),123),(prefix.replace('capture.er','other.er'),123),(prefix+prefix,123),('garbage\n'+prefix,123)]:
            with self.assertRaises(ValueError):m.capture_text(value,exp,pid)
    def test_ordinary_counts_are_phase_plus_bookends_not_all_rust_allocations(self):
        proof=m.validate_profile_native(native(),authority(),Ordinary(),{'fixture':{}})
        self.assertEqual(proof['inputs'],1024);self.assertIn('no guessed subtraction',proof['allocation_scope']);self.assertFalse(proof['reference_preparation_profiled'])
        for mutate in [lambda r:r.update(scope='pure core allocations'),lambda r:r.update(accepted_inputs=1023),lambda r:r.update(build_id='f'*64),lambda r:r.update(attribution_function='other'),lambda r:r.update(core_ns=True),lambda r:r.update(status='running')]:
            r=native();mutate(r)
            with self.assertRaises(ValueError):m.validate_profile_native(r,authority(),Ordinary(),{'fixture':{}})
    def test_raw_tool_warnings_and_coverage_limits_are_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            for kind in m.DISPLAYS:
                (root/(kind+'.stdout')).write_text('Heap allocations 7\n' if kind=='heapstat' else 'Function data\n')
                (root/(kind+'.stderr')).write_text('WARNING: interval unreliable\n' if kind=='header' else '')
            out=m.display_review(root,'heap')
            self.assertEqual(len(out['warnings']),1);self.assertFalse(out['complete_Rust_allocation_count_claim']);self.assertFalse(out['causal_ratio_or_acceptance_claim'])
            (root/'heapstat.stdout').write_text('No heap data')
            with self.assertRaises(ValueError):m.display_review(root,'heap')
    def test_empty_display_is_not_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            for kind in m.DISPLAYS:
                (root/(kind+'.stdout')).write_text('');(root/(kind+'.stderr')).write_text('')
            with self.assertRaises(ValueError):m.display_review(root,'heap')
    def test_outside_unlimited_child_mode_is_rejected(self):
        def text(path):
            if str(path)=='/proc/self/cgroup':return '0::/fixture.service\n'
            return {'memory.max':'max','memory.swap.max':'0','pids.max':'128'}[path.name]
        with patch.object(Path,'read_text',text),patch.object(m.os,'sched_getaffinity',return_value={0}):
            with self.assertRaises(ValueError):m.group_caps([0])
    def test_one_deadline_and_owned_phase_timeout_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);pidfile=root/'sleeper.pid'
            command=[sys.executable,'-c','import time;time.sleep(30)']
            with self.assertRaises(TimeoutError):m.phase(command,{'PATH':'/usr/bin:/bin'},root,root,'timeout',time.monotonic()+.04)
            pid=json.loads((root/'timeout-pid.json').read_text())['pid']
            self.assertFalse(Path('/proc',str(pid)).exists())
    def test_phase_reaps_success_and_retains_actual_accounting(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            row=m.phase([sys.executable,'-c','print("tiny fixture")'],{'PATH':'/usr/bin:/bin'},root,root,'tiny',time.monotonic()+2)
            self.assertEqual(row['exit_code'],0);self.assertGreater(row['wait4_maxrss_bytes'],0);self.assertFalse(Path('/proc',str(row['pid'])).exists())
    def test_duplicate_and_nonfinite_json_reject(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'metadata.json'
            for value in ['{"x":1,"x":2}','{"x":NaN}']:
                p.write_text(value)
                with self.assertRaises(ValueError):m.read(p)
    def test_tool_manifest_requires_all_explicit_pins(self):
        with self.assertRaises(ValueError):m.tool_guard({'profile_tools':{},'profile_aliases':{}})

if __name__=='__main__':unittest.main()
