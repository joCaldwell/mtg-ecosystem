"""Pure synthetic report/source-closure guards. No build, child, dependency install."""
import copy,hashlib,importlib.util,io,json,tarfile,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
P=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('counter_host',P/'host-run-rust-allocation-phase-v1.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)

def vector(**changes):
    d={k:0 for k in m.COUNTERS};d.update(changes);return d

def report():
    # Tiny synthetic semantics stand-in, not a native correctness proof. Existing
    # reviewed ordinary.legal_semantics is tested separately; exact baseline
    # equality here must cover ALL its opaque data, not selected convenient keys.
    baseline={'inputs':1024,'journal':{'decisions':[{'response':'pass'}]},'final_state_sha256':'a'*64,
              'trace_sha256':'b'*64,'paid_command_casts':6,'actual_outcome_kinds':{'life_gained':2}}
    r={'schema':'ordinary-core-allocation-v1','status':'passed','error':None,'build_id':'c'*64,
       'instrumentation':{'id':m.ID,'crate':'stats_alloc','version':'0.1.10','required_SIM_features':['diagnostic-allocations'],'allocator':'StatsAlloc<System>','cfg':'test AND feature diagnostic-allocations'},
       'separate_instrumentation_authority_required':True,'latency_acceptance_or_production_allocator_claim':False,
       'reference_semantics':copy.deepcopy(baseline),'accepted_inputs':1024,'advance_calls':1024,'instrumented_core_ns':1234,
       'raw_before':vector(allocations=3,bytes_reallocated=20),'raw_after':vector(allocations=5,deallocations=1,reallocations=2,bytes_allocated=64,bytes_deallocated=64,bytes_reallocated=4),
       'checked_difference':vector(allocations=2,deallocations=1,reallocations=2,bytes_allocated=64,bytes_deallocated=64,bytes_reallocated=-16),
       'empty_controls':[vector(),vector()], 'thread_guard':{'process':400,'current_thread':401,'observed_threads':[400,401]},
       'request_rate_denominator':{'accepted_inputs':1024,'no_rule_boundary_claim':True,'no_control_subtraction':True}}
    return r,{'engine_build_id':'c'*64},SimpleNamespace(legal_semantics=lambda _:None),baseline

def package_fixture(root):
    source=root/'source';(source/'src').mkdir(parents=True)
    (source/'Cargo.toml').write_text('[package]\nname="stats_alloc"\nversion="0.1.10"\n[features]\ndefault=[]\n')
    (source/'src/lib.rs').write_text('// Synthetic parser fixture only, no compilable dependency.\n')
    files=[{'path':str(f.relative_to(source)),'sha256':m.sha(f),'bytes':f.stat().st_size} for f in sorted(source.rglob('*')) if f.is_file()]
    archive=root/'fixture.crate'
    with tarfile.open(archive,'w:gz') as tar:
        for f in files:
            raw=(source/f['path']).read_bytes();info=tarfile.TarInfo('stats_alloc-0.1.10/'+f['path']);info.size=len(raw);tar.addfile(info,io.BytesIO(raw))
    checksum=m.sha(archive);manifest=root/'package.json'
    manifest.write_text(json.dumps({'schema':'test-instrumentation-package-source-v1','name':'stats_alloc','version':'0.1.10','registry':m.REGISTRY,'default_features':False,'crate_path':str(archive),'crate_sha256':checksum,'source_root':str(source),'source_files':files}))
    lock=root/'Cargo.lock';lock.write_text('version=4\n[[package]]\nname="stats_alloc"\nversion="0.1.10"\nsource="'+m.REGISTRY+'"\nchecksum="'+checksum+'"\n')
    artifact=root/'artifact.json';artifact.write_text(json.dumps({'reason':'compiler-artifact','target':{'name':'stats_alloc','kind':['lib']},'features':[],'manifest_path':str(source/'Cargo.toml'),'package_id':m.REGISTRY+'#stats_alloc@0.1.10'}))
    return {'path':str(manifest),'sha256':m.sha(manifest)},lock,{'path':str(artifact),'sha256':m.sha(artifact)},source

class Guards(unittest.TestCase):
    def test_complete_report_raw_signed_delta_and_rational_denominator(self):
        r,a,o,b=report();v=m.validate_report(r,a,o,b)
        self.assertEqual(v['requests']['bytes_reallocated'],-16)
        self.assertEqual(v['requests_per_accepted_input']['bytes_reallocated'],{'numerator':-16,'denominator':1024})
        self.assertFalse(v['complete_allocations_live_bytes_or_latency_acceptance_claim'])
    def test_same_engine_build_feature_off_and_extra_feature_rejected(self):
        artifact={'reason':'compiler-artifact','target':{'name':'mtg-sim','kind':['bin']},'features':['diagnostic-allocations']}
        m.features(artifact,['diagnostic-allocations'])
        for fs in [[],['default','diagnostic-allocations'],['diagnostic-allocations','nightly']]:
            x=copy.deepcopy(artifact);x['features']=fs
            with self.assertRaises(ValueError):m.features(x,['diagnostic-allocations'])
    def test_same_build_baseline_and_counter_feature_roles_not_interchangeable(self):
        artifact={'reason':'compiler-artifact','target':{'name':'mtg-sim','kind':['bin']},'features':['diagnostic-allocations']}
        with self.assertRaises(ValueError):m.features(artifact,[])
        artifact['features']=[];m.features(artifact,[])
        artifact['target']={'name':'mtg_core','kind':['lib']}
        with self.assertRaises(ValueError):m.features(artifact,[])
    def test_changed_baseline_journal_state_trace_paid_and_event_fields_rejected(self):
        for key in ['journal','final_state_sha256','trace_sha256','paid_command_casts','actual_outcome_kinds']:
            r,a,o,b=report();r['reference_semantics'][key]='changed'
            with self.assertRaises(ValueError):m.validate_report(r,a,o,b)
    def test_feature_off_raw_report_requires_actual_matching_executable_receipt(self):
        baseline={'report':{'sha256':'a'*64},'test_executable':{'sha256':'b'*64}}
        h={'status':'passed','executable_sha256':'b'*64,'audit':{'raw_report_sha256':'a'*64},'command':['/approved','--exact','--ignored','--test-threads=1','match_games::ordinary_diagnostic::ordinary_legal_pod_diagnostic'],'cleanup':{'errors':[],'launcher_reaped':True,'unit_empty':True},'workload':{'exit_code':0}}
        m.baseline_receipt(h,baseline)
        for change in [lambda x:x.update(executable_sha256='c'*64),lambda x:x['audit'].update(raw_report_sha256='c'*64),lambda x:x.update(status='failed'),lambda x:x['command'].__setitem__(-1,m.ENTRY)]:
            x=copy.deepcopy(h);change(x)
            with self.assertRaises(ValueError):m.baseline_receipt(x,baseline)

    def test_known_vector_receipt_binds_exact_actual_stdout_and_ignored_command(self):
        entry='diagnostic_allocations::diagnostic_allocation_known_vector_requests'
        out='running 1 test\ntest '+entry+' ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n'
        h={'status':'passed','executable_sha256':'a'*64,'workload':{'exit_code':0},'cleanup':{'errors':[],'launcher_reaped':True,'unit_empty':True},'command':['/approved','--exact','--ignored','--test-threads=1',entry],'audit':{'raw_stdout_sha256':hashlib.sha256(out.encode()).hexdigest()}}
        m.known_vector_receipt(h,out,'a'*64)
        for change in [lambda x:x.pop('audit'),lambda x:x['audit'].update(raw_stdout_sha256='b'*64),lambda x:x['command'].remove('--ignored'),lambda x:x['command'].insert(-1,'--skip')]:
            bad=copy.deepcopy(h);change(bad)
            with self.assertRaises(ValueError):m.known_vector_receipt(bad,out,'a'*64)
        with self.assertRaises(ValueError):m.known_vector_receipt(h,out,'b'*64)
        for bad in [out.replace('1 passed','0 passed').replace('running 1 test','running 0 tests'),out+out,out+'ignored\n',out[:-1],out.replace(' ... ok',' ... ignored')]:
            receipt=copy.deepcopy(h);receipt['audit']['raw_stdout_sha256']=hashlib.sha256(bad.encode()).hexdigest()
            with self.assertRaises(ValueError):m.known_vector_receipt(receipt,bad,'a'*64)

    def test_exact_input_denominator_types_reject_numeric_boolean_aliases(self):
        for change in [lambda r:r.update(accepted_inputs=1024.0),lambda r:r['request_rate_denominator'].update(accepted_inputs=1024.0),lambda r:r['request_rate_denominator'].update(no_rule_boundary_claim=1),lambda r:r['request_rate_denominator'].update(no_control_subtraction=1),lambda r:r['request_rate_denominator'].update(extra=True)]:
            r,a,o,b=report();change(r)
            with self.assertRaises(ValueError):m.validate_report(r,a,o,b)

    def test_counter_regression_forged_difference_and_machine_overflow_rejected(self):
        for change in [lambda r:r['raw_after'].update(allocations=1),lambda r:r['checked_difference'].update(allocations=3),lambda r:r['raw_after'].update(allocations=1<<64),lambda r:r['raw_before'].update(bytes_reallocated=-(1<<63)),lambda r:r['checked_difference'].update(allocations=True)]:
            r,a,o,b=report();change(r)
            with self.assertRaises(ValueError):m.validate_report(r,a,o,b)
    def test_nonzero_controls_wrong_denominator_short_prefix_and_missing_vectors_rejected(self):
        for change in [lambda r:r['empty_controls'][0].update(allocations=1),lambda r:r['request_rate_denominator'].update(accepted_inputs=1000),lambda r:r.update(accepted_inputs=1000),lambda r:r.pop('raw_after'),lambda r:r.update(status='running')]:
            r,a,o,b=report();change(r)
            with self.assertRaises((ValueError,KeyError)):m.validate_report(r,a,o,b)
    def test_extra_thread_invalid_ids_and_self_claim_rejected(self):
        for guard in [{'process':400,'current_thread':401,'observed_threads':[400,401,402]}, {'process':400,'current_thread':400,'observed_threads':[400]}, {'process':True,'current_thread':401,'observed_threads':[True,401]}]:
            r,a,o,b=report();r['thread_guard']=guard
            with self.assertRaises(ValueError):m.validate_report(r,a,o,b)
    def test_authority_and_source_authentication_before_any_import_or_child(self):
        with tempfile.TemporaryDirectory() as td:
            p=Path(td)/'module.py';sentinel=Path(td)/'executed';p.write_text('open('+repr(str(sentinel))+',"w").write("bad")')
            with self.assertRaises(ValueError):m.module('never',p,'0'*64)
            self.assertFalse(sentinel.exists())
            a=Path(td)/'authority.json';a.write_text('{}')
            with self.assertRaises(ValueError):m.guard(a,'0'*64)
    def test_exact_registry_archive_lock_and_source_inventory_pass(self):
        with tempfile.TemporaryDirectory() as td:
            row,lock,dep,_=package_fixture(Path(td));self.assertEqual(m.package_guard(row,lock,dep)['source_files'],2)
    def test_changed_resolved_lock_checksum_and_dependency_features_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            row,lock,dep,_=package_fixture(Path(td));old=lock.read_text();lock.write_text(old.replace('stats_alloc','not_stats'))
            with self.assertRaises(ValueError):m.package_guard(row,lock,dep)
            lock.write_text(old);artifact=Path(dep['path']);a=json.loads(artifact.read_text());a['features']=['nightly'];artifact.write_text(json.dumps(a));dep['sha256']=m.sha(artifact)
            with self.assertRaises(ValueError):m.package_guard(row,lock,dep)
    def test_forged_rehashed_source_manifest_cannot_float_away_from_locked_archive(self):
        with tempfile.TemporaryDirectory() as td:
            row,lock,dep,source=package_fixture(Path(td));f=source/'src/lib.rs';f.write_text('changed source')
            mp=Path(row['path']);value=json.loads(mp.read_text());entry=next(x for x in value['source_files'] if x['path']=='src/lib.rs');entry.update(sha256=m.sha(f),bytes=f.stat().st_size);mp.write_text(json.dumps(value));row['sha256']=m.sha(mp)
            with self.assertRaises(ValueError):m.package_guard(row,lock,dep)
    def test_extra_vendor_source_and_safe_path_violations_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            row,lock,dep,source=package_fixture(Path(td));(source/'build.rs').write_text('unreviewed')
            with self.assertRaises(ValueError):m.package_guard(row,lock,dep)
        for p in ['', '.', '../src/lib.rs','/src/lib.rs','src/../lib.rs','src\\lib.rs','src//lib.rs']:
            with self.assertRaises(ValueError):m.safe_relative(p)

if __name__=='__main__':unittest.main()
