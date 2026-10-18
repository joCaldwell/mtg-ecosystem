#!/usr/bin/env python3
"""Pure synthetic guard fixtures. No generated evidence, native run or cgroup."""
import copy,hashlib,importlib.util,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
from types import SimpleNamespace
P=Path(__file__).absolute().parent

def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
p=load('persistent_test',P/'run-persistent-work-v1.py');f=load('fixed_test',P/'run-fixed-work.py');f.host=load('measurement_test',P/'measurement_host.py')
BUILD='a'*64

def report(workers=1):
 candidates=[{'candidate_id':'candidate-'+str(i),'plan':{'synthetic_guard_fixture':i}}for i in range(3)]
 worlds=[{'world_index':i,'assignment_digest':hashlib.sha256(str(i).encode()).hexdigest(),'sampling_attempt':0,'proposal':{'context_id':'synthetic-context','world_index':i}}for i in range(32)]
 work={'version':'foundations-fixed-search-work-v1','context_id':'synthetic-context','horizon':256,'public_job_seed':list(hashlib.sha256(b'commander-open-search-v1/job/0').digest()),'candidate_policy':{'truncated_at_limit':False,'candidates':candidates},'worlds':worlds}
 trials=[{'world_index':w['world_index'],'assignment_digest':w['assignment_digest'],'candidate_index':i,'candidate_id':c['candidate_id'],'counts':{'accepted_inputs':256,'advance_calls':257,'query_calls':512,'rule_steps':513},'result':{'outcome':'horizon','state_sha256':'b'*64,'winner':None,'evaluations':[{'player':j,'utility':str(j-i)}for j in range(4)]}}for w in worlds for i,c in enumerate(candidates)]
 reference={'work_sha256':f.digest(work),'world_build_calls':[[i,1]for i in range(32)],'trials':trials,'aggregates':f.aggregates(work,trials)}
 schedule={'workers':workers,'complete_useful':True,'worker_faults':[],'elapsed_micros':100,'strategy_operations':10000,'worker_measurements':[{'worker_index':i,'cpu_id':i,'completed_world_indices':list(range(i,32,workers)),'elapsed_micros':90}for i in range(workers)],'worlds':[{'world_index':i,'assignment_digest':worlds[i]['assignment_digest'],'build_calls':1,'trials':[t for t in trials if t['world_index']==i]}for i in range(32)],'semantics':reference}
 return {'schema':'fixed-persistent-pool-diagnostic-v1','build_id':BUILD,'root':0,'frozen_work':work,'reference':reference,'worker_cpu_ids':list(range(workers)),'available_physical_cpu_ids':[0,1,2,3],'warmups':1,'measured_repeats':5,'rows':{'startup_micros':10,'shutdown_micros':10,'lifecycle_micros':650,'schedules':[copy.deepcopy(schedule)for _ in range(6)]},'startup_excluded_from_steady_clock':True,'shutdown_excluded_from_steady_clock':True,'steady_scope':p.STEADY,'whole_host_still_includes_startup_oracle_warmup_audit_shutdown':True,'no_product_latency_claim':True}

def validate(r):return p.validate_report(f,r,r['worker_cpu_ids'],[0,1,2,3],BUILD)
def stdout(r):
 lines=['running 1 test']
 for i,s in enumerate(r['rows']['schedules']):
  lines+=['PERSISTENT_SCHEDULE_START='+json.dumps({'generation':i,'world_order':list(range(32)),'workers':s['workers']}),'PERSISTENT_SCHEDULE_RESULT='+json.dumps({'generation':i,'complete_row':s})]
 return '\n'.join(lines)+'\nok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.01s\n'
def check_stdout(text,r):
 with tempfile.TemporaryDirectory()as tmp:
  path=Path(tmp)/'stdout';path.write_text(text);return p.validate_stdout(f,path,r,len(r['worker_cpu_ids']))

class Guards(unittest.TestCase):
 def test_complete_reference96_and_one_warm_five_persistent_runs(self):
  for n in [1,2,4]:
   value=validate(report(n));self.assertEqual(value['useful_trials_per_schedule'],96);self.assertEqual(value['measured_steady_micros'],[100]*5)
 def test_separate_clocks_and_inclusive_lifecycle(self):
  r=report();r['rows']['lifecycle_micros']=619
  with self.assertRaisesRegex(ValueError,'lifecycle includes'):validate(r)
  r=report();r['startup_excluded_from_steady_clock']=False
  with self.assertRaises(ValueError):validate(r)
 def test_incomplete_trials_short_horizon_and_duplicate_worlds_rejected(self):
  for mutate in [lambda r:r['rows']['schedules'][0]['worlds'][0]['trials'].pop(),lambda r:r['reference']['trials'][0]['counts'].__setitem__('accepted_inputs',255),lambda r:r['frozen_work']['worlds'][1].__setitem__('assignment_digest',r['frozen_work']['worlds'][0]['assignment_digest'])]:
   r=copy.deepcopy(report());mutate(r)
   with self.assertRaises(ValueError):validate(r)
 def test_worker_fault_wrong_affinity_partition_and_missing_generation(self):
  for mutate in [lambda r:r['rows']['schedules'].pop(),lambda r:r['rows']['schedules'][0]['worker_faults'].append('fault'),lambda r:r['rows']['schedules'][0]['worker_measurements'][0].__setitem__('cpu_id',9),lambda r:r['rows']['schedules'][0]['worker_measurements'][0]['completed_world_indices'].reverse()]:
   r=report();mutate(r)
   with self.assertRaises(ValueError):validate(r)
 def test_wrong_build_seed_context_and_candidate_scope(self):
  for mutate in [lambda r:r.__setitem__('build_id','c'*64),lambda r:r['frozen_work']['public_job_seed'].__setitem__(0,False),lambda r:r['frozen_work']['worlds'][0]['proposal'].__setitem__('context_id','other'),lambda r:r['frozen_work']['candidate_policy']['candidates'].pop()]:
   r=report();mutate(r)
   with self.assertRaises(ValueError):validate(r)
 def test_bool_float_numeric_aliases_rejected(self):
  for mutate in [lambda r:r.__setitem__('root',False),lambda r:r.__setitem__('warmups',True),lambda r:r['rows'].__setitem__('startup_micros',10.0),lambda r:r['rows']['schedules'][0].__setitem__('workers',True),lambda r:r['rows']['schedules'][0]['worlds'][0]['trials'][0].__setitem__('world_index',False)]:
   r=report();mutate(r)
   with self.assertRaises(ValueError):validate(r)
 def test_same_build_complete_work_and_semantics_required_for_scaling(self):
  rows=[{'workers':n,'host_status':'passed','validated':validate(report(n))}for n in [1,2,4]]
  self.assertEqual(len(p.compare(f,rows,[1,2,4])),3)
  rows[1]['validated']['work']['context_id']='other-Build'
  with self.assertRaisesRegex(ValueError,'SAME-currentBuild'):p.compare(f,rows,[1,2,4])
 def test_no_scaling_from_failed_or_missing_rows(self):
  rows=[{'workers':1,'host_status':'failed','validated':{}}]
  with self.assertRaises(ValueError):p.compare(f,rows,[1,2,4])
 def test_complete_stdout_and_missing_or_reordered_progress(self):
  r=report();text=stdout(r);self.assertTrue(check_stdout(text,r)['six_streamed_schedule_rows_equal_final'])
  for bad in [text.replace('"generation": 1','"generation": 0'),text.replace('running 1 test\n',''),text+'unexpected\n',text[:-1],text.replace('PERSISTENT_SCHEDULE_START=','UNKNOWN_MARKER=',1)]:
   with self.assertRaises(ValueError):check_stdout(bad,r)
 def test_streamed_full_semantics_and_final_report_must_match(self):
  r=report();text=stdout(r).replace('"strategy_operations": 10000','"strategy_operations": 10001',1)
  with self.assertRaises(ValueError):check_stdout(text,r)
  text=stdout(r).replace('"generation": 0','"generation": 0,"generation": 0',1)
  with self.assertRaisesRegex(ValueError,'duplicate'):check_stdout(text,r)
 def test_plan_rejects_missing_worker_counts_or_topology_drift(self):
  v={'schema':p.SCHEMA,'status':'prepared_only','available_physical_cpu_ids':[0,1,2,3],'worker_counts':[1,2,4],'root':0,'worlds':32,'timeout_seconds':120,'memory_bytes':2**31,'host_sources':{}}
  with patch.object(f.host,'physical_cpus',return_value=[0,1,2,3]):
   p.plan_values(f,v,{'host_scripts':{}})
   for field,value in [('worker_counts',[1,4]),('available_physical_cpu_ids',[0,1,2,4]),('worlds',8),('timeout_seconds',120.0)]:
    bad=copy.deepcopy(v);bad[field]=value
    with self.assertRaises(ValueError):p.plan_values(f,bad,{'host_scripts':{}})
 def test_authority_rejects_wrong_source_correctness_and_target(self):
  with tempfile.TemporaryDirectory()as tmp:
   stage=Path(tmp);q=stage/'generated/performance-verification';q.mkdir(parents=True)
   for name in ['run-persistent-work-v1.py','run-fixed-work.py','measurement_host.py']:(q/name).write_text('synthetic guard fixture')
   source=stage/'packages/game-engine/crates/sim/src/search_jobs/fixed_work/persistent.rs';source.parent.mkdir(parents=True);source.write_text('const PERSISTENT_REPETITIONS: usize = 6;\n'+p.STEADY)
   artifact=q/'artifact.json';artifact.write_text(json.dumps({'features':[]}))
   proof=q/'proof.json';proof.write_text(json.dumps({'status':'passed_persistent_focused_correctness_clippy_fmt','source_sha256':f.host.sha(source)}))
   a={'compiler_artifact':{'path':str(artifact),'sha256':f.host.sha(artifact)},'test_target':f.SIM_TARGET,'stage_root':str(stage),'timeout_seconds':120,'persistent_entrypoint':p.ENTRY,'persistent_root':0,'persistent_worlds':32,'host_scripts':{str(x):f.host.sha(x)for x in q.glob('*.py')},'persistent_source_sha256':f.host.sha(source),'persistent_correctness':{'path':str(proof),'sha256':f.host.sha(proof)}}
   adapter=SimpleNamespace(authority_check=lambda *_:a,require_target=f.require_target,SIM_TARGET=f.SIM_TARGET,host=f.host,pinned=f.pinned)
   with patch.object(p,'P',q),patch.object(p,'STAGE',stage),patch.object(p,'__file__',str(q/'run-persistent-work-v1.py')):
    self.assertIs(p.authority(adapter,q/'authority','pin'),a)
    artifact.write_text(json.dumps({'features':['diagnostic-allocations']}));a['compiler_artifact']['sha256']=f.host.sha(artifact)
    with self.assertRaisesRegex(ValueError,'feature-off'):p.authority(adapter,q/'authority','pin')
    artifact.write_text(json.dumps({'features':[]}));a['compiler_artifact']['sha256']=f.host.sha(artifact)
    a['test_target']=f.CORE_TARGET
    with self.assertRaisesRegex(ValueError,'test-target'):p.authority(adapter,q/'authority','pin')
    a['test_target']=f.SIM_TARGET;source.write_text(source.read_text()+'changed')
    with self.assertRaisesRegex(ValueError,'compiled persistent'):p.authority(adapter,q/'authority','pin')
    a['persistent_source_sha256']=f.host.sha(source)
    with self.assertRaisesRegex(ValueError,'correctness required'):p.authority(adapter,q/'authority','pin')
 def test_resource_failure_preserves_attempt_and_forbids_scaling(self):
  with tempfile.TemporaryDirectory()as tmp:
   out=Path(tmp)/'new';v={'output':str(out),'worker_counts':[1,2,4],'available_physical_cpu_ids':[0,1,2,3]};a={'test_executable':{'path':'/not-executed','sha256':'a'*64}}
   seen=[]
   def failed(**kwargs):
    seen.append(kwargs);return {'status':'failed','failure':{'reason':'resource_cap'}}
   with patch.object(p,'plan_guard',return_value=(v,a)),patch.object(f.host,'run_bounded',side_effect=failed):
    status=p.execute(f,Path(tmp)/'plan','pin')
   self.assertEqual(status['status'],'failed');self.assertEqual(len(status['rows']),1);self.assertNotIn('comparisons',status)
   self.assertEqual(seen[0]['timeout_seconds'],120);self.assertEqual(seen[0]['cpus'],[0,1,2,3]);self.assertEqual(seen[0]['command'][-1],p.ENTRY);self.assertEqual(seen[0]['env']['MTG_FIXED_SEARCH_WORLDS'],'32')
 def test_before_import_digest_rejects_mutated_helper_and_symlink(self):
  with tempfile.TemporaryDirectory()as tmp:
   q=Path(tmp);source=q/'run-fixed-work.py';source.write_text('raise RuntimeError("import must not occur")')
   with patch.object(p,'P',q):
    with self.assertRaisesRegex(ValueError,'before import'):p.load_fixed('0'*64)
   source.unlink();source.symlink_to(P/'run-fixed-work.py')
   with patch.object(p,'P',q):
    with self.assertRaisesRegex(ValueError,'before import'):p.load_fixed('0'*64)

if __name__=='__main__':unittest.main()
