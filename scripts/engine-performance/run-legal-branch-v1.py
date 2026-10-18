#!/usr/bin/env python3
"""Thin bounded host for real legal-root branching. No build/fallback root."""
import argparse,hashlib,importlib.util,json,os,re,stat,sys
from pathlib import Path
P=Path(__file__).absolute().parent
STAGE=P.parents[1]
ENTRY='engine::legal_branch_tests::legal_branch_host_diagnostic'
CASES=[f'{family}_{n}'for family in ['ordinary','dense']for n in [4,3,2]]
SWEEPS=['selected_detail','all_first_write']
WORKLIST=[{'case':c,'sweep':s}for c in CASES for s in SWEEPS]
STAGES=['creation_started','created','query_started','queried','validated','submitted','fully_advanced','verified']
PHASES=['query_started','queried','validated','submitted_not_yet_consumed','fully_advanced','verified_full_State']
LIMIT=2**31
MAX_FILE=512*1024**2
MAX_LINE=64*1024**2

def need(ok,why):
 if not ok:raise ValueError(why)
def plain(path):
 p=Path(path);need(p.is_absolute()and'..'not in p.parts,'absolute plain path')
 for q in (p,*p.parents):need(not stat.S_ISLNK(q.lstat().st_mode),'symlink ancestry')
 need(p.is_file(),'plain file');return p

def sha(p):
 h=hashlib.sha256()
 with plain(p).open('rb')as f:
  for b in iter(lambda:f.read(1024**2),b''):h.update(b)
 return h.hexdigest()

def load_support(pin):
 path=P/'run-dense-diagnostic-v1.py';need(re.fullmatch('[0-9a-f]{64}',pin)is not None and sha(path)==pin,'dense validation support SHA before import')
 spec=importlib.util.spec_from_file_location('verified_legal_dense_support',path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def auth(d,fixed,path,pin,case,sweep):
 a=fixed.authority_check(path,pin);fixed.require_target(a,fixed.CORE_TARGET)
 need(Path(a['stage_root'])==STAGE and a['timeout_seconds']==120 and len(a['cpu_ids'])==1,'one isolated stage/core/120s')
 need(a['legal_branch_cases']==WORKLIST and a['legal_branch_entrypoint']==ENTRY,'exact six roots/two sweeps')
 need(case in CASES and sweep in SWEEPS,'declared legal root/sweep')
 for file in [Path(__file__).absolute(),P/'run-dense-diagnostic-v1.py',P/'run-fixed-work.py',P/'measurement_host.py']:
  need(a['host_scripts'].get(str(file))==sha(file),'host-source authority')
 source=STAGE/'packages/game-engine/crates/core/src/engine/legal_branch_tests.rs';need(sha(source)==a['legal_branch_source_sha256'],'actual compiled legal module')
 text=plain(source).read_text();m=re.search(r'const LEGAL_BRANCH_CONTRACT: &str = r#"(.*?)"#;',text,re.S);need(m is not None,'full embedded contract')
 raw=m[1];contract=d.decode(raw);need(hashlib.sha256(raw.encode()).hexdigest()==a['legal_branch_contract_sha256'],'exact embedded contract SHA')
 need(contract['retained_counts']==[64,256,1024,10000]and contract['warmup_repetitions']==1 and contract['measured_repetitions']==5 and contract['initial_physical_cards']==400 and contract['initial_physical_commanders']==4,'fixed legal work')
 ref=a['legal_branch_correctness'];need(sha(Path(ref['path']))==ref['sha256'],'focused correctness reference pin');proof=fixed.host.read_json(Path(ref['path']))
 need(proof['status']=='passed_legal_branch_focused_correctness_clippy_fmt' and proof['source_sha256']==a['legal_branch_source_sha256'],'actual current-source correctness required')
 return a,contract

def root_case(name):
 need(name in CASES,'declared root');family,n=name.rsplit('_',1);return {'family':family,'living':int(n)}
def exact(a,b):return type(a)is type(b)and json.dumps(a,sort_keys=True,separators=(',',':'),allow_nan=False)==json.dumps(b,sort_keys=True,separators=(',',':'),allow_nan=False)
def case_is(value,name):return type(value)is dict and exact(value,root_case(name))
def integer(d,value,why):need(d.natural(value),why);return value
def selection(n):return sorted({0,n//2,max(0,n-1)}&set(range(n)))
def naturals(d,row,keys):need(set(row)==set(keys)and all(d.natural(row[k])for k in keys),'exact nonnegative integer counters')
def counts(d,c,success=False,objects=None):
 naturals(d,c,['initial_queries','query_resumptions','submits','advance_calls','rule_steps'])
 if success:
  need(c['initial_queries']==2 and c['submits']==1 and c['advance_calls']==1 and c['rule_steps']==2,'actual successful firstwrite work')
  need(c['query_resumptions']==(objects+3)//64,'all-object observation query resumption work')

def root_proof(d,r,case):
 expected=root_case(case);n=expected['living'];dense=expected['family']=='dense'
 need(case_is(r['case'],case) and integer(d,r['visits'],'root visit integer')==([65,69,73]if dense else[3,5,7])[[4,3,2].index(n)],'exact real-root visit')
 need(all(d.natural(r[k])for k in ['physical_config_cards','physical_card_facts','current_objects','battlefield','setup_accepted'])and r['physical_config_cards']==r['physical_card_facts']==400 and r['current_objects']==100*n,'actual initial400/departed-owned object count')
 alive={4:[0,1,2,3],3:[0,1,3],2:[0,3]}[n];need(exact(r['living_players'],alive),'original seats preserved')
 need(r['all_fullbytes_query_partition_restore_replay_checks']is True,'complete native independent oracles')
 need(all(d.hexsha(r[k])for k in ['root_state_sha256','firstwrite_state_sha256','profile_digest'])and r['root_state_sha256']!=r['firstwrite_state_sha256'],'actual semantic firstwrite')
 need(len(r['history_entries'])==len(r['own_knowledge_action_counts'])==4 and all(d.natural(x)for x in r['history_entries']+r['own_knowledge_action_counts']),'actual complete history/knowledge counts')
 j=r['raw_journal'];need(set(j)=={'decisions','concessions'}and len(j['decisions'])==r['setup_accepted'],'actual setup journal/input count')
 counts(d,r['setup_work']);need(r['setup_work']['submits']==len(j['decisions']),'accepted setup denominator')
 takes=[0]*4;bottom=[0]*4;bottom_rounds=[[]for _ in range(4)];due=[0]*4;lands=[0]*4;keep=[0]*4
 for item in j['decisions']:
  need(set(item)=={'decision_id','player','response'}and type(item['player'])is int and item['player']in range(4)and d.natural(item['decision_id']),'accepted input shape')
  p=item['player'];response=item['response'];tag=response['response'];need(tag in ['mulligan','bottom_opening','discard','attackers','blockers','pass','play_land'],'no undeclared strategy/mechanic')
  if tag=='mulligan':
   need(type(response['take'])is bool and keep[p]==0 and due[p]==0,'mulligan before keep/after prior bottom completion')
   if response['take']:takes[p]+=1;due[p]=max(0,takes[p]-1)
   else:keep[p]+=1
  elif tag=='bottom_opening':
   # Pinned CR103.5+103.5c: bottom after EACH nonfree round, not
   # just the final kept hand. Three takes yield lengths1 then2 (total3).
   need(type(response['objects'])is list and keep[p]==0 and due[p]>0 and len(response['objects'])==due[p],'exact completed-round bottom count')
   bottom_rounds[p].append(due[p]);bottom[p]+=due[p];due[p]=0
  elif tag=='play_land':need(keep==[1]*4,'gameplay after all keeps');lands[p]+=1
  elif tag in ['attackers','blockers']:need(keep==[1]*4,'combat after all keeps');need(response['attacks'if tag=='attackers'else'blocks']==[],'empty combat policy')
 expected_takes=[0]*4 if dense else[0,1,2,3]
 need(takes==expected_takes and keep==[1]*4 and due==[0]*4 and bottom_rounds==[list(range(1,n))for n in expected_takes]and bottom==[n*(n-1)//2 for n in expected_takes],'source-declared per-round mulligan/bottom sequence and triangular total')
 departed=[2,1][:4-n];need(exact([c['player']for c in j['concessions']],departed),'fixed real departure sequence')
 last=(0,0)
 for c in j['concessions']:
  need(set(c)=={'player','after_decisions','boundary'}and d.natural(c['after_decisions'])and d.natural(c['boundary'])and c['after_decisions']<=len(j['decisions']),'external concession proof')
  now=(c['after_decisions'],c['boundary']);need(now>=last,'ordered external input boundary');last=now
 need(r['battlefield']==sum(lands[p]for p in alive),'actual BF density from all public land inputs and owner departures')
 if dense:need(lands==[64]*4 and r['battlefield']==64*n,'natural 256/192/128 legal density')
 response=r['actual_response']
 if dense:need(set(response)=={'response','object'}and response['response']=='play_land'and type(response['object'])is str and 0<len(response['object'])<=1024,'actual visible land firstwrite')
 else:need(response=={'response':'pass'},'ordinary afterland firstwrite is real Pass')
 return {'case':expected,'actual_battlefield':r['battlefield'],'histories':r['history_entries'],'own_knowledge_actions':r['own_knowledge_action_counts'],'setup_accepted':r['setup_accepted'],'mulligan_takes':takes,'bottom_round_lengths':bottom_rounds,'bottom_total_cards':bottom,'final_kept_bottom_count':[max(0,n-1)for n in takes],'root_state_sha256':r['root_state_sha256'],'firstwrite_state_sha256':r['firstwrite_state_sha256'],'actual_response':response,'current_objects':r['current_objects'],'raw_journal_sha256':hashlib.sha256(d.canonical(j)).hexdigest()}

class Progress:
 def __init__(self,rep,target,sweep):
  self.rep=rep;self.target=target;self.sweep=sweep;self.stages=dict.fromkeys(STAGES,0);self.pending_create=None;self.targets=None;self.writing=0;self.phase=0;self.verified=[]
 def event(self,d,v):
  need(type(v['repetition'])is int and type(v['requested'])is int and v['repetition']==self.rep and v['warmup']is(self.rep==0)and v['requested']==self.target and v['sweep']==self.sweep,'progress work identity')
  branch=v['branch'];phase=v['phase'];need(d.natural(branch)and branch<self.target,'progress branch index')
  if phase=='creation_started':
   need(self.targets is None and self.pending_create is None and branch==self.stages['created'],'creation order');self.stages['creation_started']+=1;self.pending_create=branch
  elif phase=='created_unmutated':
   need(self.targets is None and self.pending_create==branch,'missing creation start');self.stages['created']+=1;self.pending_create=None
  else:
   need(self.pending_create is None,'creation not finished')
   if self.targets is None:self.targets=selection(self.stages['created'])if self.sweep=='selected_detail'else list(range(self.stages['created']))
   need(self.writing<len(self.targets)and branch==self.targets[self.writing]and phase==PHASES[self.phase],'fresh write stage/order/branch')
   self.stages[STAGES[2+self.phase]]+=1;self.phase+=1
   if self.phase==6:self.verified.append(branch);self.writing+=1;self.phase=0
  naturals(d,v['stages'],STAGES);need(v['stages']==self.stages,'actual progress counter increments')
 def summary(self):return {'repetition':self.rep,'requested':self.target,'stages':self.stages,'current_unfinished_stage':self.phase,'current_unfinished_creation':self.pending_create,'verified_indices':self.verified}


def sweep_row(d,r,track,root,sweep,case):
 rep=track.rep;target=track.target;n=track.stages['created'];useful=track.stages['verified'];need(case_is(r['root'],case)and r['sweep']==sweep and type(r['repetition'])is int and r['repetition']==rep and r['warmup']is(rep==0),'sweep identity')
 naturals(d,r['stages'],STAGES)
 need(all(d.natural(r[k])for k in ['requested','retained','useful_firstwrite_branches','untouched_or_incomplete_retained_branches'])and r['requested']==target and r['retained']==n and r['stages']==track.stages,'full raw stage conservation')
 need(track.pending_create is None,'complete row cannot leave uncreated started handle')
 need(r['baseline_state_sha256']==root['root_state_sha256']and r['firstwrite_state_sha256']==root['firstwrite_state_sha256']and d.canonical(r['actual_response'])==d.canonical(root['actual_response']),'separate exact perroot response/State reference')
 need(r['actual_action']==r['actual_response']['response'],'actual response label')
 need(r['created_handles_are_not_useful_branches']is True and r['five_sweeps_not_tail_precision']is True and r['legal_match_admission']is True,'honest branch/measurement scope')
 stop=r['resource_or_fault_shortfall'];need(stop is None or stop in ['rss_reserve_before_next_fork','rss_hard_after_fork','rss_reserve_before_next_firstwrite','rss_hard_after_firstwrite']or(type(stop)is str and stop.startswith('firstwrite_failed:')),'explicit real resource/fault stop')
 need(stop is not None or(n==target and track.phase==0),'incomplete row needs explicit stop')
 targets=selection(n)if sweep=='selected_detail'else list(range(n))
 if stop is None:need(track.writing==len(targets),'missing firstwrites without explicit shortfall')
 if stop=='rss_reserve_before_next_fork':need(n<target,'fork reserve only before incomplete creation')
 if stop=='rss_hard_after_fork':need(n>0 and track.stages['query_started']==0,'hard fork stop precludes writes')
 if stop=='rss_reserve_before_next_firstwrite':need(track.writing<len(targets),'firstwrite reserve must leave actual work')
 if stop=='rss_hard_after_firstwrite':need(track.writing>0,'hard firstwrite stop follows completed write')
 attempted=track.targets or[];started=track.stages['query_started'];bc=r['branch_counts'];need(type(bc)is list and all(type(v)is dict and d.natural(v['index'])for v in bc)and [v['index']for v in bc]==attempted[:started],'all attempted branch work retained, including error')
 for index,v in enumerate(bc):
  success=index<len(track.verified);need(set(v)=={'index','work','completed','error'}and (v['error']is None or type(v['error'])is str)and v['completed']is success and(v['error']is None)is success,'completed/error work identity');counts(d,v['work'],success,root['current_objects'])
 if stop and stop.startswith('firstwrite_failed:'):need(track.phase>0 and type(r['failed_branch_index'])is int and r['failed_branch_index']==attempted[track.writing]and bc[-1]['error']is not None,'failed write index/partial stage')
 else:need(r['failed_branch_index']is None and track.phase==0,'no unreported partial write')
 full=n==target and stop is None;all_useful=sweep=='all_first_write'and useful==target and full;sel=sweep=='selected_detail'and useful==len(selection(n))and full
 need(r['requested_handles_created_without_stop']is full and r['all_requested_firstwrites_verified']is all_useful and r['selected_detail_complete']is sel,'handles versus real useful branches')
 need(r['useful_firstwrite_branches']==useful and r['untouched_or_incomplete_retained_branches']==n-useful,'untouched denominator')
 # Check ordered real operations, not merely an unordered multiset. A failed
 # action/response-shape check after Observation can stop before validation;
 # stage2 therefore has exactly two legitimate operation prefixes.
 expected=[('fork',index)for index in range(n)]
 failed_options=[[]]
 operations=['own_observation','check_first_write','submit_first_write','advance_first_write']
 for index in attempted[:started]:
  if index in track.verified:expected.extend((op,index)for op in operations)
  else:
   sizes={1:[1],2:[1,2],3:[3],4:[4]}.get(track.phase)
   need(sizes is not None,'failed write has a real reachable partial stage')
   failed_options=[[(op,index)for op in operations[:size]]for size in sizes]
 suffix=[]
 for index in selection(n):
  if index in track.verified:
   suffix.extend((op,index)for op in ['selected_full_state_hash_cold','selected_full_state_hash_warm','dispose_selected_cold_state'])
 suffix.append(('dispose_all_retained',None))
 actual=[];total=0
 for sample in r['samples']:
  need(set(sample)=={'operation','branch','nanos'}and d.natural(sample['nanos'])and sample['nanos']<=120_000_000_000,'actual bounded segment timing')
  need(sample['branch']is None or d.natural(sample['branch']),'sample branch integer')
  actual.append((sample['operation'],sample['branch']));total+=sample['nanos']
 need(any(actual==expected+extra+suffix for extra in failed_options),'exact ordered operation/sample work count')
 if stop and stop.startswith('firstwrite_failed:'):
  work=bc[-1]['work'];failed=attempted[track.writing]
  own_ops=[op for op,index in actual if index==failed and op in operations]
  need(work['initial_queries']==(2 if 'check_first_write'in own_ops else 1),'failed query attempts retained')
  need(work['submits']==(1 if track.phase>=4 else 0),'failed submit count matches committed stage')
  need((1<=work['advance_calls']<=100000)if track.phase>=4 else work['advance_calls']==0,'failed advance work retained')
  need(work['rule_steps']<=64*work['advance_calls'],'failed rule-step work bounded by actual advance calls')
 need(d.natural(r['inclusive_lifecycle_ns'])and r['inclusive_lifecycle_ns']>=total,'whole lifecycle includes actual production segments/proofs')
 for key in ['memory_before','memory_at_retention','memory_after_disposal']:d.mem(r[key])
 if full:need(r['memory_at_retention']['rss_bytes']<LIMIT,'no fullpass above nativeRSSbudget')
 return {'repetition':rep,'warmup':rep==0,'requested':target,'created':n,'useful_firstwrites':useful,'all_requested_firstwrites_verified':all_useful,'selected_detail_complete':sel,'shortfall':stop,'raw_operation_count':len(r['samples']),'lifecycle_ns':r['inclusive_lifecycle_ns'],'row_sha256':hashlib.sha256(d.canonical(r)).hexdigest()}

class Audit:
 def __init__(self,d,case,sweep,build,contract,contract_sha):
  self.d=d;self.case=case;self.sweep=sweep;self.build=build;self.contract=contract;self.contract_sha=contract_sha;self.started=False;self.root=None;self.current=None;self.rows=[];self.final=False;self.last_setup=0;self.last_setup_history=None
 def event(self,name,v):
  d=self.d
  if name=='LEGAL_BRANCH_ROOT_START':
   need(not self.started and not self.rows and self.root is None,'one root start')
   need(case_is(v['case'],self.case)and v['sweep']==self.sweep and v['build_id']==self.build and v['contract_sha256']==self.contract_sha and d.canonical(v['contract'])==d.canonical(self.contract),'full real root identity')
   self.started=True;return
  need(self.started and not self.final,'event outside declared root lifecycle')
  if name=='LEGAL_BRANCH_ROOT_PROGRESS':
   need(self.root is None and self.current is None and case_is(v['case'],self.case),'setup progress stage')
   counts(d,v['work']);n=v['accepted_inputs'];need(d.natural(n)and n%64==0 and n>self.last_setup and n<=50000 and v['work']['submits']==n,'actual setup progress denominator')
   need(d.natural(v['P0_main_visits'])and d.natural(v['current_battlefield'])and v['current_battlefield']<=400 and type(v['living'])is int and v['living']in[2,3,4],'actual setup public dimensions')
   need(len(v['history_lengths'])==4 and all(d.natural(x)for x in v['history_lengths'])and v['pending_input_may_not_yet_be_consumed']is True,'complete setup history/pending fact')
   self.last_setup=n;self.last_setup_history=v['history_lengths'];return
  if name=='LEGAL_BRANCH_ROOT_PROOF':
   need(self.root is None and self.current is None,'one complete root proof');self.root=root_proof(d,v,self.case);need(v['setup_accepted']>=self.last_setup,'setup completion count');return
  need(self.root is not None,'no synthetic replacement for incomplete legal root')
  if name=='LEGAL_BRANCH_SWEEP_START':
   need(self.current is None and len(self.rows)<24,'one started sweep at time');rep,size=[(r,n)for r in range(6)for n in[64,256,1024,10000]][len(self.rows)]
   need(case_is(v['root'],self.case)and type(v['repetition'])is int and type(v['requested'])is int and v['repetition']==rep and v['warmup']is(rep==0)and v['requested']==size and v['sweep']==self.sweep and v['baseline_state_sha256']==self.root['root_state_sha256'],'exact24sweep schedule/root');self.current=Progress(rep,size,self.sweep);return
  if name=='LEGAL_BRANCH_PROGRESS':need(self.current is not None,'progress without sweep');self.current.event(d,v);return
  if name=='LEGAL_BRANCH_SWEEP_RESULT':
   need(self.current is not None,'completion without start');self.rows.append(sweep_row(d,v,self.current,self.root,self.sweep,self.case));self.current=None;return
  need(name=='LEGAL_BRANCH_RESULT'and self.current is None and len(self.rows)==24,'only final after24 complete declared attempts')
  need(v['schema']=='legal-branch-diagnostic-result-v1'and v['build_id']==self.build and case_is(v['case'],self.case)and v['sweep']==self.sweep and v['contract_sha256']==self.contract_sha and d.canonical(v['contract'])==d.canonical(self.contract),'final identity')
  need(type(v['warmup_sweeps'])is int and type(v['measured_sweeps'])is int and v['warmup_sweeps']==4 and v['measured_sweeps']==20 and v['outer_guard_declared_not_proven_here']is True and v['latency_or_strategic_strength_claim']is False and v['initial400_card_legal_admission']is True,'complete honest final scope')
  need(len(v['rows'])==24 and all(hashlib.sha256(d.canonical(row)).hexdigest()==ref['row_sha256']for row,ref in zip(v['rows'],self.rows)),'exact progress/final rawrow equality')
  self.final=True
 def result(self):
  return {'root_started':self.started,'root_proof_verified':self.root is not None,'root':self.root,'last_confirmed_setup_accepted':self.last_setup,'last_confirmed_setup_histories':self.last_setup_history,'completed_sweeps':len(self.rows),'expected_sweeps':24,'sweeps':self.rows,'current_partial_sweep':self.current.summary()if self.current else None,'native_final_received':self.final,'unobserved_interruption_boundary_claim':False,'synthetic_root_replacement':False}


def parse_output(path,d,case,sweep,build,contract,contract_sha,complete):
 need(plain(path).stat().st_size<=MAX_FILE,'bounded full raw stdout');a=Audit(d,case,sweep,build,contract,contract_sha);header=False;trailer=False;torn=0
 marker=re.compile(r'^(?:test '+re.escape(ENTRY)+r' \.\.\. )?(LEGAL_BRANCH_[A-Z_]+)=(.*)$')
 with path.open('rb')as f:
  while True:
   raw=f.readline(MAX_LINE+1)
   if not raw:break
   need(len(raw)<=MAX_LINE and b'\0'not in raw,'stdout line bound/NUL')
   if not raw.endswith(b'\n'):
    need(not complete,'unterminated native record');torn=len(raw);break
   line=raw.decode('utf-8').rstrip('\r\n')
   if not line:continue
   m=marker.fullmatch(line)
   if m:
    need(header and not trailer,'marker outside native test');value=d.decode(m[2]);need(type(value)is dict,'marker object');a.event(m[1],value);continue
   if line=='running 1 test':need(not header,'one native test');header=True;continue
   if line in['ok','test '+ENTRY+' ... ok','test '+ENTRY+' ... ']:
    need(header and not trailer,'test status outside lifecycle');continue
   if re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(?:\.[0-9]+)?s',line):need(header and not trailer and a.final,'one exact successful test after final');trailer=True;continue
   if not complete and(line=='FAILED'or line in['failures:','    '+ENTRY]or line.startswith('test result: FAILED.')):continue
   raise ValueError('unknown native stdout line')
 if complete:need(header and trailer and a.final and a.current is None,'one full successful test/24sweeps; no partial-root pass')
 result=a.result();result.update(raw_stdout_sha256=sha(path),excluded_torn_record_bytes=torn)
 if complete:
  result['all_requested_sizes_full_pass']=all(row['all_requested_firstwrites_verified']if sweep=='all_first_write'else row['selected_detail_complete']for row in a.rows)
  result['has_native_firstwrite_fault']=any(type(row['shortfall'])is str and row['shortfall'].startswith('firstwrite_failed:')for row in a.rows)
 return result


def progress_after(result,read_partial):
 # Successful parsing is already inside the shared120s audit lease. Reuse it;
 # only a failed attempt receives explicitly post-failure diagnostic parsing.
 if result.get('status')=='passed':return result['audit'],'reused_original_in_deadline_audit'
 return read_partial(),'post_failure_diagnostic_outside_bounded_row'


def main():
 p=argparse.ArgumentParser();p.add_argument('--dense-driver-sha256',required=True);p.add_argument('--fixed-driver-sha256',required=True);p.add_argument('--helper-sha256',required=True);p.add_argument('--authority',type=Path,required=True);p.add_argument('--authority-sha256',required=True);p.add_argument('--case',choices=CASES,required=True);p.add_argument('--sweep',choices=SWEEPS,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--execute',action='store_true');x=p.parse_args();need(x.execute,'prepared only until root authorizes');os.umask(0o077)
 d=load_support(x.dense_driver_sha256);fixed=d.load_fixed(x.fixed_driver_sha256);h=fixed.load_host(x.helper_sha256);a,c=auth(d,fixed,x.authority,x.authority_sha256,x.case,x.sweep)
 h.real_path(x.output.parent,private=True);need(not x.output.exists()and not x.output.is_symlink(),'fresh exclusive attempt');x.output.mkdir(mode=0o700)
 run=x.output/'execution';stdout=run/'workload.stdout';scratch=x.output/'scratch';scratch.mkdir(mode=0o700)
 env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','TMPDIR':str(scratch),'LEGAL_BRANCH_OUTER_GUARD':'120s-2gib-v1','LEGAL_BRANCH_CASE':x.case,'LEGAL_BRANCH_SWEEP':x.sweep}
 def check():need(auth(d,fixed,x.authority,x.authority_sha256,x.case,x.sweep)==(a,c),'source/authority endpoint')
 def audit():
  check();return parse_output(stdout,d,x.case,x.sweep,a['engine_build_id'],c,a['legal_branch_contract_sha256'],True)
 try:r=h.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',ENTRY],cwd=STAGE,env=env,directory=run,label='legal-'+x.case.replace('_','-')+'-'+x.sweep.replace('_','-'),cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
 except BaseException as error:r={'status':'failed','failure':{'type':type(error).__name__,'reason':str(error)[:2048]},'resource_report_created':False}
 partial=None;partial_scope=None
 if r.get('status')=='passed' or stdout.exists():
  try:partial,partial_scope=progress_after(r,lambda:parse_output(stdout,d,x.case,x.sweep,a['engine_build_id'],c,a['legal_branch_contract_sha256'],False))
  except Exception as error:partial={'status':'malformed_partial_not_success','error':type(error).__name__,'raw_stdout_sha256':sha(stdout)};partial_scope='post_failure_diagnostic_outside_bounded_row'
 passed=r['status']=='passed';failure=r.get('failure')
 try:check()
 except Exception as error:passed=False;failure={'type':type(error).__name__,'reason':str(error)[:2048]}
 v=r.get('audit');fault=bool(v and v.get('has_native_firstwrite_fault'));full=bool(passed and v and v['all_requested_sizes_full_pass']and not fault)
 status='all_declared_useful_branches_verified'if full else 'failed_native_firstwrite'if passed and fault else 'verified_explicit_resource_shortfall'if passed else 'failed_or_truncated'
 result={'schema':'legal-branch-host-result-v1','status':status,'case':x.case,'sweep':x.sweep,'authority_sha256':x.authority_sha256,'source_contract_sha256':a['legal_branch_contract_sha256'],'driver_sha256':sha(Path(__file__).absolute()),'host_report_sha256':sha(run/'host-report.json')if(run/'host-report.json').exists()else None,'failure':failure,'audit':v,'partial_progress':partial,'partial_progress_scope':partial_scope,'all_declared_attempt_records_verified':bool(passed),'full_declared_worklist_verified':full,'all_requested_sizes_full_pass':full,'initial400_legal_root_proof_verified':bool(v and v['root_proof_verified']),'root_preparation_incomplete':not bool((v or partial or{}).get('root_proof_verified')),'unobserved_interruption_boundary_claim':False,'synthetic_replacement':False,'acceptance_latency_or_strategy_strength_claim':False}
 h.save_json(x.output/'result.json',result);print(json.dumps({'status':status}));return 0 if full else 1
if __name__=='__main__':sys.exit(main())
