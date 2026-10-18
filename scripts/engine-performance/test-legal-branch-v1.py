#!/usr/bin/env python3
"""Tiny synthetic parser/authority fixtures only; no native or OS workload runs."""
import copy,hashlib,importlib.util,json,tempfile,unittest
from types import SimpleNamespace
from pathlib import Path
from unittest.mock import patch
P=Path(__file__).absolute().parent

def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
l=module('legal_guard',P/'run-legal-branch-v1.py')
d=module('dense_guard',P/'run-dense-diagnostic-v1.py')
BUILD='a'*64
# These values test report guards. They are NOT real correctness/timing evidence.
CONTRACT={'initial_physical_cards':400,'initial_physical_commanders':4,'retained_counts':[64,256,1024,10000],'warmup_repetitions':1,'measured_repetitions':5,'fixture':'synthetic parser input'}
CSHA=hashlib.sha256(d.canonical(CONTRACT)).hexdigest()

def work(success=False):return {'initial_queries':2 if success else 0,'query_resumptions':6 if success else 0,'submits':1 if success else 0,'advance_calls':1 if success else 0,'rule_steps':2 if success else 0}
def memory():return {'rss_bytes':1024**2,'high_water_bytes':2*1024**2}
def root(case='ordinary_4'):
 family,alive=case.rsplit('_',1);alive=int(alive);decisions=[]
 def add(p,response):decisions.append({'decision_id':len(decisions),'player':p,'response':response})
 for p in range(4):
  for round_index in range(p if family=='ordinary'else 0):
   add(p,{'response':'mulligan','take':True})
   if round_index:add(p,{'response':'bottom_opening','objects':['bottom']*round_index})
  add(p,{'response':'mulligan','take':False})
 for p in range(4):
  for _ in range(64 if family=='dense'else 1):add(p,{'response':'play_land','object':'own-visible-fixture'})
 departed=[2,1][:4-alive]
 setup=work();setup['submits']=len(decisions)
 return {'case':l.root_case(case),'visits':([65,69,73]if family=='dense'else[3,5,7])[[4,3,2].index(alive)],'physical_config_cards':400,'physical_card_facts':400,'current_objects':100*alive,'battlefield':(64 if family=='dense'else 1)*alive,'living_players':{4:[0,1,2,3],3:[0,1,3],2:[0,3]}[alive],'all_fullbytes_query_partition_restore_replay_checks':True,'root_state_sha256':'b'*64,'firstwrite_state_sha256':'c'*64,'profile_digest':'d'*64,'history_entries':[10]*4,'own_knowledge_action_counts':[2]*4,'setup_accepted':len(decisions),'setup_work':setup,'raw_journal':{'decisions':decisions,'concessions':[{'player':p,'after_decisions':len(decisions),'boundary':100+i}for i,p in enumerate(departed)]},'actual_response':{'response':'play_land','object':'own-visible-fixture'}if family=='dense'else{'response':'pass'}}

def root_summary(case='ordinary_4'):return l.root_proof(d,root(case),case)
def sample(operation,branch):return {'operation':operation,'branch':branch,'nanos':10}
def row(rep=0,target=64,n=0,sweep='all_first_write',case='ordinary_4',failed_phase=None,failed_ops=None):
 t=l.Progress(rep,target,sweep);events=[];samples=[];branch_counts=[]
 def emit(phase,index):
  # Model the independent source stage increment, then exercise production parser.
  stages=copy.deepcopy(t.stages)
  key={'created_unmutated':'created','submitted_not_yet_consumed':'submitted','verified_full_State':'verified'}.get(phase,phase)
  stages[key]+=1
  event={'repetition':rep,'warmup':rep==0,'requested':target,'sweep':sweep,'branch':index,'phase':phase,'stages':stages}
  t.event(d,event);events.append(('LEGAL_BRANCH_PROGRESS',event))
 for index in range(n):
  emit('creation_started',index);samples.append(sample('fork',index));emit('created_unmutated',index)
 targets=l.selection(n)if sweep=='selected_detail'else list(range(n))
 for j,index in enumerate(targets):
  if failed_phase is not None and j>0:break
  phases=l.PHASES if failed_phase is None else l.PHASES[:failed_phase]
  for phase in phases:emit(phase,index)
  ops=['own_observation','check_first_write','submit_first_write','advance_first_write']
  samples.extend(sample(op,index)for op in ops[:4 if failed_phase is None else failed_ops])
  w=work(True)
  if failed_phase is not None:w={'initial_queries':2 if failed_ops>=2 else 1,'query_resumptions':0,'submits':int(failed_phase>=4),'advance_calls':int(failed_phase>=4),'rule_steps':int(failed_phase>=4)}
  branch_counts.append({'index':index,'work':w,'completed':failed_phase is None,'error':None if failed_phase is None else 'synthetic failure'})
 for index in l.selection(n):
  if index in t.verified:samples.extend(sample(op,index)for op in ['selected_full_state_hash_cold','selected_full_state_hash_warm','dispose_selected_cold_state'])
 samples.append(sample('dispose_all_retained',None))
 stop='firstwrite_failed:synthetic failure'if failed_phase is not None else None if n==target else 'rss_reserve_before_next_fork'
 full=n==target and stop is None;useful=t.stages['verified'];r=root_summary(case)
 result={'root':l.root_case(case),'sweep':sweep,'repetition':rep,'warmup':rep==0,'requested':target,'retained':n,'stages':copy.deepcopy(t.stages),'branch_counts':branch_counts,'resource_or_fault_shortfall':stop,'failed_branch_index':targets[0]if failed_phase else None,'baseline_state_sha256':r['root_state_sha256'],'firstwrite_state_sha256':r['firstwrite_state_sha256'],'actual_response':r['actual_response'],'actual_action':r['actual_response']['response'],'created_handles_are_not_useful_branches':True,'five_sweeps_not_tail_precision':True,'legal_match_admission':True,'requested_handles_created_without_stop':full,'all_requested_firstwrites_verified':full and sweep=='all_first_write'and useful==target,'selected_detail_complete':full and sweep=='selected_detail'and useful==len(targets),'useful_firstwrite_branches':useful,'untouched_or_incomplete_retained_branches':n-useful,'samples':samples,'inclusive_lifecycle_ns':sum(s['nanos']for s in samples)+10,'memory_before':memory(),'memory_at_retention':memory(),'memory_after_disposal':memory()}
 return result,t,events

def schedule(case='ordinary_4',sweep='all_first_write'):
 start={'case':l.root_case(case),'sweep':sweep,'build_id':BUILD,'contract':CONTRACT,'contract_sha256':CSHA}
 events=[('LEGAL_BRANCH_ROOT_START',start),('LEGAL_BRANCH_ROOT_PROOF',root(case))];rows=[]
 for rep in range(6):
  for target in [64,256,1024,10000]:
   r,_,progress=row(rep,target,sweep=sweep,case=case)
   events.append(('LEGAL_BRANCH_SWEEP_START',{k:r[k]for k in ['root','repetition','warmup','requested','sweep','baseline_state_sha256']}));events.extend(progress);events.append(('LEGAL_BRANCH_SWEEP_RESULT',r));rows.append(r)
 events.append(('LEGAL_BRANCH_RESULT',{'schema':'legal-branch-diagnostic-result-v1','build_id':BUILD,'case':l.root_case(case),'sweep':sweep,'contract':CONTRACT,'contract_sha256':CSHA,'warmup_sweeps':4,'measured_sweeps':20,'rows':rows,'outer_guard_declared_not_proven_here':True,'latency_or_strategic_strength_claim':False,'initial400_card_legal_admission':True}))
 return events

def output(events,entry=l.ENTRY):return 'running 1 test\n'+''.join(name+'='+json.dumps(value,separators=(',',':'))+'\n'for name,value in events)+'ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.01s\n'
def parse(text,complete=True):
 with tempfile.TemporaryDirectory()as tmp:
  p=Path(tmp)/'stdout';p.write_text(text);return l.parse_output(p,d,'ordinary_4','all_first_write',BUILD,CONTRACT,CSHA,complete)

class Guards(unittest.TestCase):
 def test_actual_ordinary4_multiplayer_round_history_includes_both_P3_bottoms(self):
  # Exact setup-only13-input prefix from legal-branches-run-1-ordinary4-selected.
  # Raw physical IDs remain fixture facts; this is a parser regression, not a
  # new semantic/timing proof. CR103.5/103.5c rules SHA8d860e45...eb070ca.
  prefix=[{'decision_id': 0, 'player': 0, 'response': {'response': 'mulligan', 'take': False}}, {'decision_id': 1, 'player': 1, 'response': {'response': 'mulligan', 'take': True}}, {'decision_id': 2, 'player': 2, 'response': {'response': 'mulligan', 'take': True}}, {'decision_id': 3, 'player': 3, 'response': {'response': 'mulligan', 'take': True}}, {'decision_id': 4, 'player': 1, 'response': {'response': 'mulligan', 'take': False}}, {'decision_id': 5, 'player': 2, 'response': {'response': 'mulligan', 'take': True}}, {'decision_id': 6, 'player': 3, 'response': {'response': 'mulligan', 'take': True}}, {'decision_id': 7, 'player': 2, 'response': {'objects': [{'card': 297, 'incarnation': 3}], 'response': 'bottom_opening'}}, {'decision_id': 8, 'player': 3, 'response': {'objects': [{'card': 383, 'incarnation': 1}], 'response': 'bottom_opening'}}, {'decision_id': 9, 'player': 2, 'response': {'response': 'mulligan', 'take': False}}, {'decision_id': 10, 'player': 3, 'response': {'response': 'mulligan', 'take': True}}, {'decision_id': 11, 'player': 3, 'response': {'objects': [{'card': 394, 'incarnation': 1}, {'card': 393, 'incarnation': 1}], 'response': 'bottom_opening'}}, {'decision_id': 12, 'player': 3, 'response': {'response': 'mulligan', 'take': False}}]
  r=root();lands=[x for x in r['raw_journal']['decisions']if x['response']['response']=='play_land']
  r['raw_journal']['decisions']=copy.deepcopy(prefix)+lands
  for i,item in enumerate(r['raw_journal']['decisions']):item['decision_id']=i
  r['setup_accepted']=len(r['raw_journal']['decisions']);r['setup_work']['submits']=r['setup_accepted']
  value=l.root_proof(d,r,'ordinary_4')
  self.assertEqual(value['bottom_round_lengths'],[[],[],[1],[1,2]])
  self.assertEqual(value['bottom_total_cards'],[0,0,1,3]);self.assertEqual(value['final_kept_bottom_count'],[0,0,1,2])
  p3=[x for x in r['raw_journal']['decisions']if x['player']==3 and x['response']['response']=='bottom_opening'];self.assertEqual(len(p3),2)
  def same_total_wrong_rounds(x):
   a=x['raw_journal']['decisions'][8]['response'];b=x['raw_journal']['decisions'][11]['response'];a['objects'],b['objects']=b['objects'],a['objects']
  for mutate in [same_total_wrong_rounds,lambda x:x['raw_journal']['decisions'][8]['response'].__setitem__('objects',[]),lambda x:x['raw_journal']['decisions'][11]['response'].__setitem__('objects',[{'card':394,'incarnation':1}])]:
   bad=copy.deepcopy(r);mutate(bad)
   with self.assertRaises(ValueError):l.root_proof(d,bad,'ordinary_4')
 def test_completed_audit_is_not_parsed_twice_outside_deadline(self):
  audit={'completed_sweeps':24}
  def forbidden():raise RuntimeError('duplicate full parse')
  value,scope=l.progress_after({'status':'passed','audit':audit},forbidden)
  self.assertIs(value,audit);self.assertEqual(scope,'reused_original_in_deadline_audit')
  value,scope=l.progress_after({'status':'failed'},lambda:{'root_proof_verified':False})
  self.assertEqual(value,{'root_proof_verified':False});self.assertIn('post_failure',scope)
 def test_all_six_declared_real_root_journals(self):
  self.assertEqual(len(l.WORKLIST),12)
  for case in l.CASES:self.assertEqual(root_summary(case)['current_objects'],100*l.root_case(case)['living'])
 def test_24_resource_attempts_are_not_full_success(self):
  result=parse(output(schedule()));self.assertEqual(result['completed_sweeps'],24);self.assertFalse(result['all_requested_sizes_full_pass']);self.assertFalse(result['has_native_firstwrite_fault'])
 def test_each_retained_branch_needs_real_firstwrite(self):
  for sweep in l.SWEEPS:
   r,t,_=row(n=64,sweep=sweep);v=l.sweep_row(d,r,t,root_summary(),sweep,'ordinary_4');self.assertEqual(v['useful_firstwrites'],64 if sweep=='all_first_write'else 3)
  r,t,_=row(n=64,sweep='selected_detail');r['sweep']='all_first_write'
  with self.assertRaises(ValueError):l.sweep_row(d,r,t,root_summary(),'all_first_write','ordinary_4')
 def test_failed_action_vs_validation_prefixes(self):
  for phase,ops in [(1,1),(2,1),(2,2),(3,3),(4,4)]:
   r,t,_=row(n=1,failed_phase=phase,failed_ops=ops);self.assertEqual(l.sweep_row(d,r,t,root_summary(),'all_first_write','ordinary_4')['useful_firstwrites'],0)
  r,t,_=row(n=1,failed_phase=2,failed_ops=3)
  with self.assertRaises(ValueError):l.sweep_row(d,r,t,root_summary(),'all_first_write','ordinary_4')
 def test_missing_duplicate_reordered_operations(self):
  r,t,_=row(n=64)
  for mutate in [lambda x:x['samples'].pop(),lambda x:x['samples'].append(x['samples'][0]),lambda x:x['samples'].reverse(),lambda x:x['branch_counts'].pop()]:
   bad=copy.deepcopy(r);mutate(bad)
   with self.assertRaises(ValueError):l.sweep_row(d,bad,t,root_summary(),'all_first_write','ordinary_4')
 def test_numeric_bool_float_rejected(self):
  for field in ['visits','physical_config_cards','current_objects','setup_accepted']:
   for bad in [True,float(root()[field])]:
    r=root();r[field]=bad
    with self.assertRaises(ValueError):l.root_proof(d,r,'ordinary_4')
  r,t,_=row(n=64)
  for path in [('repetition',),('requested',),('retained',),('stages','created')]:
   bad=copy.deepcopy(r);target=bad
   for k in path[:-1]:target=target[k]
   target[path[-1]]=float(target[path[-1]])
   with self.assertRaises(ValueError):l.sweep_row(d,bad,t,root_summary(),'all_first_write','ordinary_4')
 def test_missing_repetition_or_progress(self):
  events=schedule();del events[2]
  with self.assertRaises(ValueError):parse(output(events))
  t=l.Progress(0,64,'all_first_write')
  with self.assertRaises(ValueError):t.event(d,{'repetition':0,'warmup':True,'requested':64,'sweep':'all_first_write','branch':0,'phase':'created_unmutated','stages':dict.fromkeys(l.STAGES,0)})
 def test_partial_root_never_substitutes_synthetic_root(self):
  start=schedule()[0];r=parse('running 1 test\n'+start[0]+'='+json.dumps(start[1])+'\n',False)
  self.assertFalse(r['root_proof_verified']);self.assertFalse(r['synthetic_root_replacement']);self.assertEqual(r['completed_sweeps'],0)
  with self.assertRaises(ValueError):parse(output([start]))
 def test_torn_record_excluded_only_in_partial(self):
  text='running 1 test\nLEGAL_BRANCH_ROOT_START='+json.dumps(schedule()[0][1])+'\nLEGAL_BRANCH_ROOT_PROOF={"torn":'
  r=parse(text,False);self.assertGreater(r['excluded_torn_record_bytes'],0);self.assertFalse(r['root_proof_verified'])
  with self.assertRaises(ValueError):parse(text)
 def test_wrong_build_contract_extra_stdout_or_duplicate_json(self):
  for field in ['build_id','contract_sha256']:
   ev=schedule();ev[0][1][field]='e'*64
   with self.assertRaises(ValueError):parse(output(ev))
  text=output(schedule())
  for changed in [text+'garbage\n',text.replace('"schema":"legal-branch-diagnostic-result-v1"','"schema":"legal-branch-diagnostic-result-v1","schema":"alias"'),text.replace('running 1 test\n',''),text.replace('ok\ntest result:', 'test result:') .replace('1 passed; 0 failed','2 passed; 0 failed')]:
   with self.assertRaises(ValueError):parse(changed)
 def test_final_rows_must_equal_streamed_rows(self):
  ev=schedule();ev[-1][1]['rows'][0]['samples'][0]['nanos']=99
  # Break shared fixture reference deliberately: the original row must remain.
  ev[-2]=copy.deepcopy(ev[-2]);ev[3]=('LEGAL_BRANCH_SWEEP_RESULT',row()[0])
  with self.assertRaises(ValueError):parse(output(ev))
 def test_no_full_pass_with_fewer_writes_and_no_stop(self):
  r,t,_=row(n=1);r['resource_or_fault_shortfall']=None;r['retained']=64
  with self.assertRaises(ValueError):l.sweep_row(d,r,t,root_summary(),'all_first_write','ordinary_4')
 def test_failed_work_counts_must_match_real_phase(self):
  for key,value in [('initial_queries',0),('submits',0),('advance_calls',0),('rule_steps',1000)]:
   r,t,_=row(n=1,failed_phase=4,failed_ops=4);r['branch_counts'][0]['work'][key]=value
   with self.assertRaises(ValueError):l.sweep_row(d,r,t,root_summary(),'all_first_write','ordinary_4')
 def test_thin_authority_binds_actual_source_contract_correctness_and_target(self):
  with tempfile.TemporaryDirectory()as tmp:
   stage=Path(tmp);p=stage/'generated/performance-verification';p.mkdir(parents=True)
   for name in ['run-legal-branch-v1.py','run-dense-diagnostic-v1.py','run-fixed-work.py','measurement_host.py']:(p/name).write_text('synthetic authority fixture')
   source=stage/'packages/game-engine/crates/core/src/engine/legal_branch_tests.rs';source.parent.mkdir(parents=True)
   raw=json.dumps(CONTRACT);source.write_text('const LEGAL_BRANCH_CONTRACT: &str = r#"'+raw+'"#;')
   reference=p/'correctness.json';reference.write_text('{}')
   a={'stage_root':str(stage),'timeout_seconds':120,'cpu_ids':[0],'legal_branch_cases':l.WORKLIST,'legal_branch_entrypoint':l.ENTRY,'host_scripts':{str(f):l.sha(f)for f in p.iterdir()if f.suffix=='.py'},'legal_branch_source_sha256':l.sha(source),'legal_branch_contract_sha256':hashlib.sha256(raw.encode()).hexdigest(),'legal_branch_correctness':{'path':str(reference),'sha256':l.sha(reference)}}
   proof={'status':'passed_legal_branch_focused_correctness_clippy_fmt','source_sha256':a['legal_branch_source_sha256']}
   def target(_a,t):l.need(t=='core','CORE target required')
   fixed=SimpleNamespace(authority_check=lambda *_:a,require_target=target,CORE_TARGET='core',host=SimpleNamespace(read_json=lambda _:proof))
   with patch.object(l,'STAGE',stage),patch.object(l,'P',p),patch.object(l,'__file__',str(p/'run-legal-branch-v1.py')):
    self.assertEqual(l.auth(d,fixed,p/'authority','pin','ordinary_4','all_first_write')[1],CONTRACT)
    source.write_text(source.read_text()+'//changed')
    with self.assertRaisesRegex(ValueError,'compiled legal module'):l.auth(d,fixed,p/'authority','pin','ordinary_4','all_first_write')
    a['legal_branch_source_sha256']=l.sha(source)
    a['legal_branch_contract_sha256']='e'*64
    with self.assertRaisesRegex(ValueError,'embedded contract SHA'):l.auth(d,fixed,p/'authority','pin','ordinary_4','all_first_write')
    a['legal_branch_contract_sha256']=hashlib.sha256(raw.encode()).hexdigest()
    with self.assertRaisesRegex(ValueError,'correctness required'):l.auth(d,fixed,p/'authority','pin','ordinary_4','all_first_write')
    proof['source_sha256']=a['legal_branch_source_sha256'];proof['status']='tests_not_run'
    with self.assertRaisesRegex(ValueError,'correctness required'):l.auth(d,fixed,p/'authority','pin','ordinary_4','all_first_write')
    fixed.require_target=lambda *_:(_ for _ in()).throw(ValueError('SIM target rejected'))
    with self.assertRaisesRegex(ValueError,'SIM target'):l.auth(d,fixed,p/'authority','pin','ordinary_4','all_first_write')
 def test_new_root_undeclared_mode_and_wrong_stage_work_fail(self):
  with self.assertRaises(ValueError):l.root_case('synthetic_fallback_4')
  t=l.Progress(0,64,'all_first_write');event={'repetition':0,'warmup':True,'requested':64,'sweep':'all_first_write','phase':'creation_started','branch':0,'stages':dict.fromkeys(l.STAGES,0)}
  with self.assertRaises(ValueError):t.event(d,event)
  r,t,_=row(n=1,failed_phase=4,failed_ops=4);r['branch_counts'][0]['work']['advance_calls']=1.0
  with self.assertRaises(ValueError):l.sweep_row(d,r,t,root_summary(),'all_first_write','ordinary_4')
 def test_noncanonical_authority_import_rejected_before_side_effect(self):
  with tempfile.TemporaryDirectory()as tmp:
   p=Path(tmp);source=p/'run-dense-diagnostic-v1.py';source.write_text('raise RuntimeError("imported")')
   with patch.object(l,'P',p):
    with self.assertRaisesRegex(ValueError,'before import'):l.load_support('0'*64)
   source.unlink();source.symlink_to(P/'run-dense-diagnostic-v1.py')
   with patch.object(l,'P',p):
    with self.assertRaisesRegex(ValueError,'symlink'):l.load_support('0'*64)

if __name__=='__main__':unittest.main()
