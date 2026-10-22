import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE = Path(__file__).absolute().parent
sys.path.insert(0,str(HERE))
import measurement_host as h
import host_inputs as inputs
spec = importlib.util.spec_from_file_location('fixed',HERE/'run-fixed-work.py')
f = importlib.util.module_from_spec(spec)
spec.loader.exec_module(f)
f.host=h


class SharedHost(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.root.chmod(0o700)
        self.cpus = h.physical_cpus()[:1]
        self.exe = Path(sys.executable).resolve()
        self.children = []

    def tearDown(self):
        for p in self.children:
            if p.poll() is None:
                os.killpg(p.pid,signal.SIGKILL)
                p.wait()
        self.tmp.cleanup()

    def fake_service(self,args,**kwargs):
        directory = Path(args[-1])
        # Real payload+real tiny child; only systemd/cgroup controls are fixtures.
        p = subprocess.Popen([str(self.exe),str(HERE/'measurement_host.py'),'--payload',str(directory)],**kwargs)
        self.children.append(p)
        return p

    def properties(self,_):
        return {'MainPID':str(self.children[-1].pid),'MemoryMax':str(h.MEMORY_BYTES),
                'MemorySwapMax':'0','TasksMax':str(h.TASKS),'RuntimeMaxUSec':self.runtime,'ControlGroup':'/fixture'}

    def counters(self,_):
        return {'memory_max':str(h.MEMORY_BYTES),'memory_swap_max':'0','pids_max':str(h.TASKS),
                'memory_peak_bytes':123456,'memory_current_bytes':1234,'memory_events':{'oom':0,'oom_kill':0,'max':0},
                'cpu_stat':{'user_usec':4,'system_usec':2},'cgroup_events':{'populated':1}}

    def cleanup(self,unit,launcher,group):
        os.killpg(launcher.pid,signal.SIGKILL)
        launcher.wait(timeout=3)
        return {'unit_empty':True,'launcher_reaped':True,'errors':[]}

    def run_fixture(self,code='pass',timeout=2,audit=None,popen=None,counters=None):
        self.runtime = str(__import__('math').ceil(timeout)+5)+'s'
        original_popen = subprocess.Popen
        def service(args,**kwargs):
            directory = Path(args[-1])
            p = original_popen([str(self.exe),str(HERE/'measurement_host.py'),'--payload',str(directory)],**kwargs)
            self.children.append(p)
            if popen:
                popen(p)
            return p
        with patch.object(h.subprocess,'Popen',service),patch.object(h,'_show',self.properties),patch.object(h,'_counters',counters or self.counters),patch.object(h,'_group',lambda _:self.root/'fakegroup'),patch.object(h,'_cleanup',self.cleanup),patch.object(h,'validate_limits',lambda *args:None),patch.object(h,'assert_only_payload',lambda *args:None):
            return h.run_bounded(command=[str(self.exe),'-c',code],cwd=self.root,env=dict(os.environ),directory=self.root/'attempt',label='tiny-test',cpus=self.cpus,timeout_seconds=timeout,expected_executable_sha256=h.sha(self.exe),audit=audit)

    def test_real_payload_child_usage_and_private_files(self):
        result = self.run_fixture('print("tiny fixture")',audit=lambda:{'checked':True})
        self.assertEqual(result['status'],'passed')
        self.assertEqual(result['workload']['exit_code'],0)
        self.assertGreaterEqual(result['workload']['user_seconds'],0)
        self.assertEqual(result['audit'],{'checked':True})
        self.assertEqual((self.root/'attempt').stat().st_mode & 0o777,0o700)
        for p in (self.root/'attempt').iterdir():
            self.assertEqual(p.stat().st_mode & 0o777,0o600)
        self.assertTrue(result['cleanup']['launcher_reaped'])

    def test_real_child_nonzero_preserves_failure_and_logs(self):
        result = self.run_fixture('print("failure retained");raise SystemExit(7)')
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['workload']['exit_code'],7)
        self.assertIn('failure retained',(self.root/'attempt/workload.stdout').read_text())
        self.assertEqual(h.read_json(self.root/'attempt/host-report.json')['status'],'failed')

    def test_external_timeout_kills_owned_real_payload_and_child(self):
        result = self.run_fixture('import time;time.sleep(10)',timeout=.15)
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['failure']['type'],'TimeoutError')
        self.assertTrue(all(p.poll() is not None for p in self.children))

    def test_signal_during_spawn_gap_is_caught_after_owned_popen(self):
        result = self.run_fixture(popen=lambda _:os.kill(os.getpid(),signal.SIGTERM))
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['received_signals'],[signal.SIGTERM])
        self.assertTrue(result['cleanup']['launcher_reaped'])

    def test_signal_during_final_cleanup_cannot_pass(self):
        original = self.cleanup
        def cancelled(unit,launcher,group):
            os.kill(os.getpid(),signal.SIGTERM)
            return original(unit,launcher,group)
        self.cleanup = cancelled
        result = self.run_fixture()
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['received_signals'],[signal.SIGTERM])
        self.assertTrue(result['cleanup']['launcher_reaped'])

    def test_final_audit_cannot_reset_or_exceed_deadline(self):
        def slow():
            time.sleep(.15)
            return {'status':'claimed-pass'}
        result = self.run_fixture(timeout=.12,audit=slow)
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['failure']['type'],'TimeoutError')

    def test_audit_exception_fails_closed(self):
        def bad():
            raise ValueError('changed raw trial')
        result = self.run_fixture(audit=bad)
        self.assertEqual(result['status'],'failed')
        self.assertIn('changed raw trial',result['failure']['reason'])

    def test_charged_memory_passing_does_not_override_child_rss_failure(self):
        original=h.read_json
        def tampered(path):
            value=original(path)
            if Path(path).name=='payload-result.json':
                value['ru_maxrss_bytes']=h.MEMORY_BYTES+1
            return value
        with patch.object(h,'read_json',tampered):
            result=self.run_fixture()
        self.assertEqual(result['status'],'failed')
        self.assertLess(result['kernel_final']['memory_peak_bytes'],h.MEMORY_BYTES)
        self.assertIn('RSS',result['failure']['reason'])

    def test_memory_cap_hit_never_passes(self):
        def oom(group):
            value = self.counters(group)
            value['memory_events']['max'] = 1
            return value
        self.assertEqual(self.run_fixture(counters=oom)['status'],'failed')

    def test_preflight_bad_executable_no_attempt_creation(self):
        with self.assertRaises(ValueError):
            h.run_bounded(command=[str(self.exe)],cwd=self.root,env={},directory=self.root/'new',label='bad',cpus=self.cpus,expected_executable_sha256='0'*64)
        self.assertFalse((self.root/'new').exists())

    def test_private_parent_and_symlink_rejected(self):
        self.root.chmod(0o755)
        with self.assertRaises(ValueError):
            h.real_path(self.root,private=True)
        self.root.chmod(0o700)
        (self.root/'link').symlink_to(self.exe)
        with self.assertRaises(ValueError):
            h.real_path(self.root/'link')

    def test_unexpected_descendants_reject(self):
        (self.root/'cgroup.procs').write_text('123\n456\n')
        with self.assertRaises(RuntimeError):
            h.assert_only_payload(self.root,123)
        (self.root/'cgroup.procs').write_text('123\n')
        h.assert_only_payload(self.root,123)

    def test_kernel_properties_and_real_affinity_checked(self):
        props = {'MainPID':str(os.getpid()),'MemoryMax':str(h.MEMORY_BYTES),'MemorySwapMax':'0','TasksMax':str(h.TASKS)}
        counters = self.counters(None)
        actual = sorted(os.sched_getaffinity(0))
        h.validate_limits(props,counters,os.getpid(),actual)
        for field in ('memory_max','memory_swap_max','pids_max'):
            bad = dict(counters)
            bad[field]='wrong'
            with self.assertRaises(RuntimeError):
                h.validate_limits(props,bad,os.getpid(),actual)
        with self.assertRaises(RuntimeError):
            h.validate_limits(props,counters,os.getpid(),[])

    def test_no_duplicate_json_or_nonfinite_constants(self):
        for raw in ('{"a":1,"a":2}','{"a":NaN}'):
            p = self.root/'bad.json'
            p.write_text(raw)
            with self.assertRaises(ValueError):
                h.read_json(p)


class StrictJsonReads(unittest.TestCase):
    # Exercise both readers with tiny caps; never allocate a pod-sized fixture.
    def readers(self):
        return [(h, h.read_json), (inputs, inputs.read)]

    def test_exact_cap_unicode_and_cap_plus_one(self):
        raw = '{"text":"雪☃é"}'.encode('utf-8')
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'game-00.json'
            for module, read in self.readers():
                with self.subTest(reader=module.__name__):
                    path.write_bytes(raw)
                    self.assertEqual(read(path, max_bytes=len(raw)), {'text': '雪☃é'})
                    path.write_bytes(raw + b' ')
                    with self.assertRaisesRegex(ValueError, f'bound {len(raw)} bytes: {path}'):
                        read(path, max_bytes=len(raw))

    def test_single_open_descriptor_check_and_bounded_read_catches_growth_before_parse(self):
        cap = 16
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'game-00.json'
            path.write_bytes(b'{}' + b' '*(cap-1))  # Actual size cap+1; fstat is stale.
            for module, read in self.readers():
                for grew in (False, True):
                    with self.subTest(reader=module.__name__, grew=grew):
                        requests = []
                        with path.open('rb') as source:
                            class Tracked:
                                def __enter__(self): return self
                                def __exit__(self, *args): pass
                                def fileno(self): return source.fileno()
                                def read(self, size):
                                    requests.append(size)
                                    return source.read(size)
                            with patch.object(Path, 'open', return_value=Tracked()) as opened, \
                                 patch.object(module.os, 'fstat', return_value=SimpleNamespace(st_size=cap if grew else cap+1)) as sized, \
                                 patch.object(module.json, 'loads', side_effect=AssertionError('must reject before parse')):
                                with self.assertRaisesRegex(ValueError, f'bound {cap} bytes: {path}'):
                                    read(path, max_bytes=cap)
                            opened.assert_called_once_with('rb')
                            sized.assert_called_once_with(source.fileno())
                            self.assertEqual(requests, [cap+1] if grew else [])

    def test_default_stays_128mib_readonly_sparse_oversize_rejects_before_read(self):
        default = 128*1024**2
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'authority.json'
            with path.open('wb') as source: source.truncate(default+1)
            path.chmod(0o400)
            for module, read in self.readers():
                with self.subTest(reader=module.__name__):
                    self.assertEqual(module.MAX_JSON_BYTES, default)
                    self.assertEqual(read.__defaults__, (default,))
                    with path.open('rb') as source:
                        class ReadForbidden:
                            def __enter__(self): return self
                            def __exit__(self, *args): pass
                            def fileno(self): return source.fileno()
                            def read(self, size): raise AssertionError('oversize sparse file read')
                        with patch.object(Path, 'open', return_value=ReadForbidden()), \
                             patch.object(module.json, 'loads', side_effect=AssertionError('oversize parsed')):
                            with self.assertRaisesRegex(ValueError, f'bound {default} bytes: {path}'):
                                read(path)

    def test_strict_json_rejects_malformed_duplicate_nonfinite_and_truncated(self):
        for module, read in self.readers():
            for raw in [b'{bad}', b'{"a":1,"a":2}', b'{"nested":{"a":1,"a":2}}',
                        b'{"a":NaN}', b'{"a":Infinity}', b'{"a":-Infinity}', b'{"a":', b'{}{}']:
                with self.subTest(reader=module.__name__, raw=raw), tempfile.TemporaryDirectory() as tmp:
                    path = Path(tmp)/'bad.json'; path.write_bytes(raw)
                    with self.assertRaises(ValueError): read(path, max_bytes=64)

    def test_invalid_caps_reject_and_input_path_rules_remain(self):
        for module, read in self.readers():
            for cap in [-1, True, 1.5, None]:
                with self.subTest(reader=module.__name__, cap=cap):
                    with self.assertRaisesRegex(ValueError, 'integer JSON byte bound'):
                        read(Path('/unused'), max_bytes=cap)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'game.json'; path.write_text('{}')
            alias = path.with_name('alias'); alias.symlink_to(path)
            with self.assertRaisesRegex(ValueError, 'symlink'): inputs.read(alias, max_bytes=64)
            with self.assertRaisesRegex(ValueError, 'absolute'): inputs.read(Path('relative.json'), max_bytes=64)


class RowProof(unittest.TestCase):
    def fixture(self,world_count=8):
        worlds = [{'world_index':w,'sampling_attempt':0,'assignment_digest':f'{w:064x}','proposal':{'fixed':w}} for w in range(world_count)]
        work = {'version':'foundations-fixed-search-work-v1','context_id':'ctx','public_job_seed':list(__import__('hashlib').sha256(b'commander-open-search-v1/job/0').digest()),'horizon':256,
                'worlds':worlds,'candidate_policy':{'truncated_at_limit':False,'candidates':[{'candidate_id':'pass','plan':{'action':'pass'}}]}}
        trials = [{'world_index':w,'assignment_digest':worlds[w]['assignment_digest'],'candidate_index':0,'candidate_id':'pass',
                   'counts':{'accepted_inputs':256,'advance_calls':300,'query_calls':400,'rule_steps':500},
                   'result':{'outcome':'horizon','state_sha256':f'{w+12:064x}',
                             'evaluations':[{'player':p,'public_status':'playing','utility':str(p-w)} for p in range(4)]}} for w in range(world_count)]
        semantics = {'work_sha256':f.digest(work),'world_build_calls':[[w,7] for w in range(world_count)],'trials':trials,'aggregates':f.aggregates(work,trials)}
        result = {'workers':1,'complete_useful':True,'worker_faults':[],'worker_measurements':[{'worker_index':0,'cpu_id':0,'completed_world_indices':list(range(world_count))}],
                  'worlds':[{'world_index':w,'assignment_digest':worlds[w]['assignment_digest'],'build_calls':7,
                             'trials':[dict(trials[w],timings={'any':123})]} for w in range(world_count)],'semantics':semantics,'elapsed_micros':100,'strategy_operations':100}
        sample = lambda i:{'repetition':i,'warmup':i==0,'semantic_equality':True,'result':__import__('copy').deepcopy(result)}
        return {'schema':'foundations-fixed-search-row-v1','status':'fixed_row_verified','engine_build_id':'a'*64,'root_index':0,'horizon':256,'workers':1,'worker_cpu_ids':[0],'available_physical_cpu_ids':[0,1,2,3],
                'frozen_work':work,'frozen_work_sha256':f.digest(work),'reference_semantics':semantics,'reference_semantics_sha256':f.digest(semantics),'reference_complete_useful':True,'warmup':sample(0),'samples':[sample(i) for i in range(1,6)]}

    def validate(self,row):
        return f.validate_row(row,root=0,cpus=[0],available=[0,1,2,3],build='a'*64)

    def test_complete_positive_and_all_worker_rows_before_ratios(self):
        validated = self.validate(self.fixture())
        rows = [{'workers':i,'host_status':'passed','validated':validated} for i in [1,2,4]]
        self.assertEqual(len(f.comparison(rows,[1,2,4])),3)
        for values in (rows[:-1], [dict(rows[0],host_status='failed'),*rows[1:]]):
            with self.assertRaises(ValueError):
                f.comparison(values,[1,2,4])

    def test_each_raw_field_tamper_rejects_despite_success_booleans(self):
        changes = [lambda r:r['samples'][0]['result']['worlds'].pop(),
                   lambda r:r['samples'][0]['result']['worlds'][0]['trials'][0]['counts'].__setitem__('query_calls',401),
                   lambda r:r['samples'][0]['result']['worlds'][0]['trials'][0]['result'].__setitem__('state_sha256','b'*64),
                   lambda r:r['samples'][0]['result']['worker_measurements'][0]['completed_world_indices'].pop(),
                   lambda r:r['samples'][0]['result']['worlds'][0]['trials'][0]['result'].__setitem__('outcome','not_started'),
                   lambda r:r['samples'].pop(),lambda r:r.__setitem__('engine_build_id','b'*64),
                   lambda r:r['frozen_work']['worlds'][1].__setitem__('assignment_digest','0'*64)]
        for change in changes:
            r = self.fixture()
            change(r)
            with self.assertRaises(ValueError):
                self.validate(r)

    def test_rehashed_short_horizon_or_utility_omission_still_rejected(self):
        for change in (lambda t:t['counts'].__setitem__('accepted_inputs',255),lambda t:t['result']['evaluations'].pop()):
            r = self.fixture()
            change(r['reference_semantics']['trials'][0])
            r['reference_semantics_sha256']=f.digest(r['reference_semantics'])
            with self.assertRaises(ValueError):
                self.validate(r)

    def test_predeclared_32_world_workload_rejects_eight_world_substitution(self):
        valid=f.validate_row(self.fixture(32),root=0,cpus=[0],available=[0,1,2,3],build='a'*64,world_count=32)
        self.assertEqual(valid['useful_trials_per_sample'],32)
        with self.assertRaises(ValueError):
            f.validate_row(self.fixture(),root=0,cpus=[0],available=[0,1,2,3],build='a'*64,world_count=32)

    def test_cross_row_counts_fullstate_and_assignment_equality_not_boolean(self):
        a=self.validate(self.fixture())
        for field in ('counts','result'):
            b=__import__('copy').deepcopy(a)
            b['semantics']['trials'][0][field]['changed']=1
            with self.assertRaises(ValueError):
                f.comparison([{'workers':1,'host_status':'passed','validated':a},{'workers':2,'host_status':'passed','validated':b}],[1,2])


class AuthorityProof(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.root=Path(self.tmp.name)
        self.root.chmod(0o700)
        for directory in ['packages/game-engine/crates','packages/game-server/src','packages/game-server/tests','packages/game-engine/tests/fixtures']:
            (self.root/directory).mkdir(parents=True)
        for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','packages/game-server/Cargo.toml','packages/game-engine/crates/lib.rs','packages/game-engine/tests/fixtures/catalog.json']:
            (self.root/name).write_text('source fixture')
        self.contract=self.root/'contract.json'
        self.contract.write_text('{"fixture":true}')
        self.settings={'rustc':'rustc 1.99.0 (fixture 2026-10-15)','profile':'release','target':'x86_64-unknown-linux-gnu','rustflags':'','opt_level':'3','debug':'false','debug_assertions':'false','overflow_checks':True}
        self.artifact=self.root/'cargo-artifact.json'
        h.save_json(self.artifact,{'reason':'compiler-artifact','target':{'name':'mtg-sim','kind':['bin']},'profile':{'test':True,'opt_level':'3','debuginfo':0,'debug_assertions':False,'overflow_checks':True},'executable':str(Path(sys.executable).resolve())})
        self.manifest=self.root/'native.json'
        self.files=[{'path':name,'sha256':h.sha(self.root/name),'bytes':(self.root/name).stat().st_size} for name in f.inventory(self.root)]
        self.build=f.compiled_build_id(self.root,self.files,self.settings)
        h.save_json(self.manifest,{'status':'compiled-source-closure-verified','build_id':self.build,'files':self.files})
        self.authority=self.root/'authority.json'
        self.value={'schema':'performance-host-authority-v1','stage_root':str(self.root),'engine_build_id':self.build,
                    'build_settings':self.settings,'test_target':f.SIM_TARGET,'compiler_artifact':{'path':str(self.artifact),'sha256':h.sha(self.artifact)},
                    'native_manifest':{'path':str(self.manifest),'sha256':h.sha(self.manifest)},
                    'test_executable':{'path':str(Path(sys.executable).resolve()),'sha256':h.sha(Path(sys.executable).resolve())},
                    'contracts':[{'path':str(self.contract),'sha256':h.sha(self.contract)}]}
        h.save_json(self.authority,self.value)
        self.pin=h.sha(self.authority)

    def tearDown(self):
        self.tmp.cleanup()

    def test_positive_closed_fixture_and_source_hash_mutation_negative(self):
        f.authority_check(self.authority,self.pin)
        (self.root/'packages/game-engine/crates/lib.rs').write_text('changed')
        with self.assertRaises(ValueError):
            f.authority_check(self.authority,self.pin)

    def test_added_native_file_not_in_manifest_rejected(self):
        (self.root/'packages/game-engine/crates/extra.rs').write_text('extra')
        with self.assertRaises(ValueError):
            f.authority_check(self.authority,self.pin)

    def test_changed_contract_and_wrong_compiled_build_rejected(self):
        self.contract.write_text('changed')
        with self.assertRaises(ValueError):
            f.authority_check(self.authority,self.pin)
        self.contract.write_text('{"fixture":true}')
        m=h.read_json(self.manifest)
        m['build_id']='b'*64
        h.save_json(self.manifest,m)
        self.value['native_manifest']['sha256']=h.sha(self.manifest)
        h.save_json(self.authority,self.value)
        with self.assertRaises(ValueError):
            f.authority_check(self.authority,h.sha(self.authority))

    def test_missing_toolchain_fields_false_profile_and_metadata_build_drift_reject(self):
        original=__import__('copy').deepcopy(self.value)
        for change in [lambda a:a['build_settings'].pop('rustc'),lambda a:a['build_settings'].update(profile='debug'),lambda a:a['build_settings'].update(debug_assertions=False),lambda a:a['build_settings'].update(overflow_checks=False),lambda a:a['build_settings'].update(rustc='rustc changed')]:
            a=__import__('copy').deepcopy(original);change(a);h.save_json(self.authority,a)
            with self.assertRaises(ValueError):f.authority_check(self.authority,h.sha(self.authority))

    def test_overflow_claim_requires_actual_pinned_cargo_artifact(self):
        artifact=h.read_json(self.artifact);artifact['profile']['overflow_checks']=False
        h.save_json(self.artifact,artifact);self.value['compiler_artifact']['sha256']=h.sha(self.artifact)
        h.save_json(self.authority,self.value)
        with self.assertRaises(ValueError):f.authority_check(self.authority,h.sha(self.authority))

    def test_core_test_authority_is_valid_but_sim_callers_reject_it(self):
        artifact=h.read_json(self.artifact);artifact['target']={'name':'mtg_core','kind':['lib']}
        h.save_json(self.artifact,artifact);self.value['compiler_artifact']['sha256']=h.sha(self.artifact);self.value['test_target']=f.CORE_TARGET
        h.save_json(self.authority,self.value)
        authority=f.authority_check(self.authority,h.sha(self.authority))
        with self.assertRaises(ValueError):f.require_target(authority,f.SIM_TARGET)
        f.require_target(authority,f.CORE_TARGET)

    def test_helper_wrong_pin_rejects_before_import(self):
        with self.assertRaises(ValueError):
            f.load_host('0'*64)

    def test_wrong_authority_pin_fails_before_output_creation(self):
        with self.assertRaises(ValueError):
            f.authority_check(self.authority,'0'*64)
        self.assertFalse((self.root/'run').exists())

    def test_real_systemd_duration_parser(self):
        self.assertEqual(h.duration_us('2min 5s'),125000000)
        self.assertEqual(h.duration_us('125s'),125000000)
        self.assertEqual(h.duration_us('1500ms'),1500000)
        for bad in ['infinity','125','2min what','-1s']:
            with self.assertRaises(RuntimeError):
                h.duration_us(bad)

if __name__ == '__main__':
    unittest.main()
