#!/usr/bin/env python3
from pathlib import Path
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib,json,subprocess,time,traceback
import argparse,re,os
ROOT=EXE=DRIVER=BUILD=EXESHA=None
INPUTS=AUTHORITY=NATIVE_MANIFEST=FROZEN=None
STOP=False
# Fresh paths are selected only from the reserved current-search-primary area.
AREA=OUT=REPORT=WRAPPER=None

def read(p): return INPUTS.read(p)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''): h.update(b)
 return h.hexdigest()
def canonical(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'),sort_keys=True).encode()
def demand(v,why):
 if not v:raise AssertionError(why)
def save(): INPUTS.save(REPORT,result)
def safe_path(value):
 demand(isinstance(value,str) and value and not any(c in value for c in ['\\',':','\x00']) and not value.startswith('/') and all(x not in ['', '.', '..'] for x in value.split('/')),'canonical relative source path')
 return Path(value)
def load_inputs(expected):
 import importlib.util
 path=Path(__file__).absolute().with_name('host_inputs.py')
 demand(not path.is_symlink() and sha(path)==expected,'input guard SHA before import')
 spec=importlib.util.spec_from_file_location('authenticated_reviewer_inputs',path)
 out=importlib.util.module_from_spec(spec);spec.loader.exec_module(out);return out

def guard():
 global AUTHORITY,NATIVE_MANIFEST,FROZEN
 AUTHORITY,NATIVE_MANIFEST,FROZEN=INPUTS.check_search(ARGS.authority,ARGS.authority_sha256,Path(__file__))
 demand(AUTHORITY['primary_wrapper']=={'path':str(DRIVER),'sha256':sha(DRIVER)},'original primary wrapper source pinned')
 demand(AUTHORITY['host_sources'].get(str(DRIVER))==sha(DRIVER),'wrapper host map pin')
 return {**FROZEN,'driver_sha256':sha(DRIVER),'wrapper_sha256':sha(WRAPPER)}

IDLE_POLICY='node-idle600s-zero2s-self-and-waited-child-aggregate-1pct-one-core-v1'
CPU_FIELDS=('user_ticks','system_ticks','waited_child_user_ticks','waited_child_system_ticks')
def verify_idle_baseline(wrapper):
 # Independent arithmetic from raw counters; never trusts a PID exception list
 # or the wrapper's reported met/CPU deltas in place of reconstructing them.
 proof=wrapper['idle_baseline'];ticks=proof['clock_ticks_per_second']
 demand(type(ticks) is int and ticks==os.sysconf('SC_CLK_TCK') and ticks>0,'kernel CPU clock tick rate')
 demand(proof['policy']==IDLE_POLICY and proof['minimum_age_seconds']==600 and proof['aggregate_cpu_limit_percent_of_one_core']==1 and proof['aggregate_cpu_denominator']==100,'exact bounded idle baseline policy')
 demand(type(proof['preflight_sample_elapsed_ns']) is int and proof['preflight_sample_elapsed_ns']>=2_000_000_000,'two-second idle preflight')
 uptime=proof['preflight_uptime_ticks'];demand(len(uptime)==2 and all(type(x) is int and x>=0 for x in uptime) and uptime[1]>=uptime[0],'conservative preflight uptimes')
 def rows(values):
  demand(isinstance(values,list),'process rows list');mapped={}
  for row in values:
   demand(set(row)=={'pid','name','state','start_ticks','cpu_ticks',*CPU_FIELDS},'exact process facts without argv/secrets')
   demand(type(row['pid']) is int and row['pid']>1 and row['pid'] not in mapped,'unique actual process ID')
   demand(type(row['start_ticks']) is int and row['start_ticks']>=0,'actual process start identity')
   demand(all(type(row[k]) is int and row[k]>=0 for k in (*CPU_FIELDS,'cpu_ticks')) and row['cpu_ticks']==sum(row[k] for k in CPU_FIELDS),'raw self/waited-child CPU exact sum')
   demand(isinstance(row['name'],str) and (row['name'] in ['node','nodejs'] or row['name'].startswith('node-')),'only Node daemon baseline')
   demand(isinstance(row['state'],str) and len(row['state'])==1 and row['state'] not in ['T','t','Z','X','x'],'baseline process alive and not stopped')
   mapped[row['pid']]=row
  demand([r['pid'] for r in values]==sorted(mapped),'ordered unique process evidence')
  return mapped
 first=rows(proof['first_preflight_processes']);base=rows(proof['baseline_processes'])
 demand(first.keys()==base.keys(),'same complete two-sample heavy inventory')
 for pid,old in first.items():
  new=base[pid]
  demand(old['name']==new['name'] and old['start_ticks']==new['start_ticks'] and old['state']==new['state']=='S','same sleeping Node identity in both preflight samples')
  demand(all(old[k]==new[k] for k in CPU_FIELDS),'zero raw self and waited-child CPU preflight')
  demand(uptime[0]-old['start_ticks']>=600*ticks and uptime[1]-new['start_ticks']>=600*ticks,'all baseline daemons at least ten minutes old')
 demand(wrapper['initial_other_heavy_processes']==proof['baseline_processes'],'all initial heavy processes accounted by baseline')
 samples=proof['samples'];demand(isinstance(samples,list) and len(samples)>=2,'monitor and final idle samples present')
 previous=base;elapsed=0;maximum_window_ticks=0;maximum_window_ns=0;total=0
 for sample in samples:
  demand(sample['errors']==[] and sample['met'] is True,'no sampled idle baseline failure')
  now=sample['elapsed_ns'];window=sample['window_ns']
  demand(type(now) is int and type(window) is int and now>elapsed and window==now-elapsed,'adjacent elapsed window exact')
  current=rows(sample['processes']);demand(current.keys()==base.keys(),'no new heavy job or exited baseline process')
  per_process=[]
  for pid,row in current.items():
   demand(row['name']==base[pid]['name'] and row['start_ticks']==base[pid]['start_ticks'],'stable PID/starttime/name every sample')
   demand(all(row[k]>=previous[pid][k] for k in CPU_FIELDS),'each raw CPU counter monotonic')
   per_process.append({'pid':pid,'start_ticks':row['start_ticks'],'window_cpu_ticks':row['cpu_ticks']-previous[pid]['cpu_ticks'],'total_cpu_ticks':row['cpu_ticks']-base[pid]['cpu_ticks']})
  delta=sum(r['cpu_ticks']-previous[p]['cpu_ticks'] for p,r in current.items());total=sum(r['cpu_ticks']-base[p]['cpu_ticks'] for p,r in current.items())
  demand(sample['per_process_deltas']==per_process,'reported per-process CPU deltas independently derived')
  demand(type(sample['aggregate_window_cpu_ticks']) is int and type(sample['aggregate_total_cpu_ticks']) is int and sample['aggregate_window_cpu_ticks']==delta and sample['aggregate_total_cpu_ticks']==total,'reported aggregate CPU deltas independently derived')
  demand(delta*100*1_000_000_000<=window*ticks,'aggregate ONE-core one-percent limit every adjacent window')
  demand(total*100*1_000_000_000<=now*ticks,'aggregate ONE-core one-percent limit entire elapsed window')
  maximum_window_ticks=max(maximum_window_ticks,delta);maximum_window_ns=max(maximum_window_ns,window);previous=current;elapsed=now
 demand(proof['met'] is True and wrapper['quiescence_met'] is True and wrapper['other_heavy_process_observations']==[],'whole sampled-quiescence proof passed')
 demand(wrapper['final_other_heavy_processes']==samples[-1]['processes'],'final heavy inventory included in final sample')
 demand(type(wrapper['wall_seconds']) in [int,float] and wrapper['wall_seconds']>0 and elapsed>=int(wrapper['wall_seconds']*1_000_000_000),'idle monitoring covers full benchmark process lifecycle')
 return {'status':'verified_bounded_idle_baseline','policy':IDLE_POLICY,'baseline_process_count':len(base),'preflight_sample_elapsed_ns':proof['preflight_sample_elapsed_ns'],'samples_verified':len(samples),'clock_ticks_per_second':ticks,'aggregate_total_cpu_ticks':total,'elapsed_ns':elapsed,'maximum_sample_window_ns':maximum_window_ns,'maximum_sample_window_cpu_ticks':maximum_window_ticks,'scope':'sampled bounded background quiescence; no continuous absence claim; self+waited-child CPU charged'}

def aggregate(candidates,trials):
 rows=[]
 for i,c in enumerate(candidates):
  selected=[t for t in trials if t['candidate_index']==i]
  useful=[t for t in selected if t['result']['outcome'] in ['terminal','horizon']]
  distinct={t['assignment_digest'] for t in useful};wins=[0]*4;sums=[0]*4;counts=Counter(t['result']['outcome'] for t in useful)
  for t in useful:
   o=t['result'];winner=o.get('winner')
   if winner is not None:wins[winner]+=1
   demand(len(o['evaluations'])==4 and {v['player'] for v in o['evaluations']}==set(range(4)),'all4 evaluations')
   for v in o['evaluations']:sums[v['player']]+=int(v['utility'])
  rows.append({'candidate_index':i,'candidate_id':c['candidate_id'],'useful_distinct_worlds':len(distinct),'minimum_met':len(distinct)>=8,'terminal_games':counts['terminal'],'horizon_trials':counts['horizon'],'winner_counts':wins,'utility_sums':list(map(str,sums))})
 return rows
def verify_replay(native,evidence):
 demand(native['status']=='semantic_worklist_verified' and native['measurements']=='not_remeasured','native replay authority')
 for key,expected in [('verified_trials',evidence['useful_trials']),('unverified_diagnostic_stops',evidence['diagnostic_stops'])]:
  demand(type(native[key]) is int and native[key]>=0 and native[key]==expected,'native replay exact useful/diagnostic denominator')

def verify_replay_set(rows):
 demand(len(rows)==60 and {(r['index'],r['repetition'])for r in rows}=={(i,k)for i in range(12)for k in range(5)} and all(r['status']=='passed'for r in rows),'all60 unique exact replay outcomes passed')

def publish_success():
 try:
  demand(not STOP,'review interrupted before success publication')
  result['status']='passed';save()
  demand(not STOP,'review interrupted during success publication')
 except BaseException:
  result['status']='failed';save();raise

def _main():
 global AREA,OUT,REPORT,WRAPPER,result,ARGS,INPUTS,ROOT,EXE,DRIVER,BUILD,EXESHA,AUTHORITY,NATIVE_MANIFEST,FROZEN
 parser=argparse.ArgumentParser(description='PREPARED ONLY until parent authorization: independent current-build primary60 static audit and all useful-trial replay; no timing remeasurement.')
 parser.add_argument('--authority',required=True,type=Path)
 parser.add_argument('--authority-sha256',required=True)
 parser.add_argument('--inputs-sha256',required=True)
 parser.add_argument('--run-directory',required=True,type=Path)
 parser.add_argument('--expected-wrapper-sha256',required=True)
 parser.add_argument('--execute',action='store_true',help='Explicit parent-authorized semantic audit/replay; never starts a benchmark')
 args=parser.parse_args();ARGS=args
 demand(args.execute,'explicit --execute required; preparation does not run any replay')
 INPUTS=load_inputs(args.inputs_sha256)
 AUTHORITY,NATIVE_MANIFEST,FROZEN=INPUTS.check_search(args.authority,args.authority_sha256,Path(__file__))
 ROOT=Path(AUTHORITY['stage_root']);EXE=INPUTS.pin(AUTHORITY['cli']);BUILD=AUTHORITY['build_id'];EXESHA=AUTHORITY['cli']['sha256']
 DRIVER=INPUTS.pin(AUTHORITY['primary_wrapper'])
 AREA=INPUTS.plain(args.run_directory,directory=True)
 demand(AREA.parent==Path(AUTHORITY['run_parent']),'declared measured run namespace')
 WRAPPER=AREA.with_name(AREA.name+'-wrapper.json')
 demand(re.fullmatch('[0-9a-f]{64}',args.expected_wrapper_sha256) and sha(WRAPPER)==args.expected_wrapper_sha256,'exact reviewed measurement-wrapper report SHA')
 OUT=AREA.with_name(AREA.name+'-independent-review');REPORT=AREA.with_name(AREA.name+'-independent-primary-review.json')
 os.umask(0o077)
 demand(not OUT.exists() and not OUT.is_symlink() and not REPORT.exists() and not REPORT.is_symlink(),'fresh review output required')
 OUT.mkdir();result={'status':'running','build_id':BUILD,'executable_sha256':EXESHA,'driver_sha256':sha(Path(__file__)),'timings':'Original primary measurements audited, not remeasured by semantic replay','replays':[],'findings':[]};save()
 try:
  result['source_start_guards']=guard()
  wrapper=read(WRAPPER);top=read(AREA/'report.json');manifest=read(AREA/'manifest.json');workload=read(AREA/'workload.json')
  result['measurement_wrapper']={'path':str(WRAPPER),'sha256':sha(WRAPPER)}
  demand(wrapper['status']=='measurement_passed_replay_pending' and wrapper['exit_code']==0 and wrapper['direct_child_reaped'] and wrapper['owned_group_empty'] is True,'primary process success/reaped/group-empty')
  demand(wrapper['build_id']==top['build_id']==manifest['build_id']==BUILD and wrapper['executable_sha256']==EXESHA,'all identities')
  demand(wrapper['driver_sha256']==sha(DRIVER),'original wrapper exact bytes')
  demand(wrapper['report_sha256']==sha(AREA/'report.json'),'original report SHA')
  result['quiescence_review']=verify_idle_baseline(wrapper)
  demand(wrapper['source_end_verified'],'original end source guard')
  demand(wrapper['runtime_environment']=={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C'},'minimal recorded native runtime environment')
  demand(wrapper['frozen_inputs']==FROZEN,'measurement exact current authority/source/archive/workload inputs')
  demand(wrapper['authority']=={'path':str(args.authority),'sha256':args.authority_sha256},'measurement authority pin')
  demand(wrapper['predecessor_proofs']==FROZEN['predecessors'],'same completed predecessor evidence')
  demand(wrapper['physical_cpu_topology']['worker_cpu_ids']==[0,1,2,3] and len({tuple(v) for v in wrapper['physical_cpu_topology']['physical_package_and_core_ids']})==4,'measurement actual distinct physical topology')
  demand((AREA/'workload.json').read_bytes()==(ROOT/'packages/game-engine/tests/fixtures/search-workload.json').read_bytes(),'exact frozen workload')
  demand(manifest['workload_sha256']==sha(AREA/'workload.json'),'workload digest')
  demand(manifest['native_source_manifest']==NATIVE_MANIFEST,'reported complete native manifest equals pinned current release authority')
  demand(manifest['jobs']==60 and manifest['repetitions_per_root']==5 and manifest['profile']=='release' and manifest['reference_cpu_verified'] and manifest['quiescence_declared'],'reference release manifest')
  demand('AMD Ryzen 9 9950X' in manifest['cpu_info'],'reference machine')
  topology={str(i):{key:Path(f'/sys/devices/system/cpu/cpu{i}/topology/{key}').read_text().strip() for key in ['physical_package_id','core_id']} for i in range(4)}
  demand(len({tuple(r.values()) for r in topology.values()})==4,'four distinct physical cores')
  result['physical_cpu_topology']=topology
  jobs=top['jobs'];demand(len(jobs)==60 and {(j['index'],j['repetition']) for j in jobs}=={(i,k) for i in range(12) for k in range(5)},'all60 exact denominator')
  demand(top['attempted_jobs']==top['reported_job_latencies']==60 and top['p95_nearest_rank']==57,'all60 nearest rank')
  latencies=[];counts=Counter();evidence=[];first_by_root={};overlap={};compared=0;root_counts={};all_internal_peaks=[]
  for j in jobs:
   filename=f"root-{j['index']:02}-repeat-{j['repetition']}.json";demand(j['evidence']==filename,'exact evidence filename')
   p=AREA/filename;x=read(p);r=x['report'];case=x['case'];i=j['index'];rep=j['repetition'];candidates=r['candidate_policy']['candidates']
   demand(x['build_id']==BUILD and x['measurement_classification']=='declared-quiescent-reference-native','exact build/classification')
   demand(x['source_manifest']==manifest['native_source_manifest'] and hashlib.sha256(canonical(x['source_manifest'])).hexdigest()==top['native_source_manifest_sha256']==manifest['native_source_manifest_sha256'],'full source manifest binding')
   demand(case['index']==r['request']['root_index']==i and case['viewer']==i%4 and case['mulligans']==[(i//4+s)%4 for s in range(4)],'named case and mulligans')
   expected_seed=list(hashlib.sha256(f'commander-open-search-v1/job/{i}'.encode()).digest());demand(r['request']['public_job_seed']==expected_seed,'independent fixed job seed')
   demand(r['request']['worker_cpu_ids']==r['actual_worker_cpu_ids']==[0,1,2,3] and j['worker_cpu_ids_verified'],'four actual pinned workers')
   demand(1<=len(candidates)<=32 and len({c['candidate_id'] for c in candidates})==len(candidates),'nonempty unique candidate set')
   demand(r['elapsed_micros']==j['elapsed_micros']<=5_000_000 and r['hard_latency_met'] and r['p95_root_target_met']==(r['elapsed_micros']<=3_000_000),'complete lifecycle hard latency')
   latencies.append(r['elapsed_micros']);all_internal_peaks.append(r['maximum_process_rss_bytes']);demand(j['maximum_process_rss_bytes']==r['maximum_process_rss_bytes'] and r['memory_met'],'perjob peak binding')
   demand(not r['worker_faults'],'no worker fault')
   specs=r['world_specs'];demand(len(specs)==r['produced_distinct_worlds'] and len({s['assignment_digest'] for s in specs})==len(specs),'distinct semantic worlds')
   trials=r['trials'];demand(not any(t['result']['outcome'] in ['engine_fault','policy_failure','invalid_world'] for t in trials),'no engine/policy/world fault')
   demand(aggregate(candidates,trials)==r['aggregates'],'all trial aggregate exact independent sum')
   first=[t for t in trials if t['world_index']<8];demand(len(first)==8*len(candidates),'complete first8 worklist')
   demand(all(t['result']['outcome'] in ['terminal','horizon'] for t in first),'every first8 useful')
   demand(all({t['world_index'] for t in first if t['candidate_index']==c}==set(range(8)) for c in range(len(candidates))),'all candidate/world pairs')
   firstagg=aggregate(candidates,first);demand(firstagg==r['selection_aggregates'] and all(a['useful_distinct_worlds']==8 for a in firstagg) and r['minimum_met'] and j['minimum_met'],'8 distinct per candidate minimum')
   chosen=max(range(len(candidates)),key=lambda c:int(firstagg[c]['utility_sums'][case['viewer']]));demand(r['chosen_candidate_index']==chosen and r['chosen_candidate_id']==candidates[chosen]['candidate_id'] and r['chosen_plan']==candidates[chosen]['plan'],'fixed-first8 chosen plan')
   signature={'candidates':candidates,'specs':specs[:8],'selection':firstagg,'chosen':r['chosen_candidate_index'],'trials':[{k:t[k] for k in ['world_index','assignment_digest','candidate_index','candidate_id','counts','result']} for t in first]}
   if i in first_by_root:demand(first_by_root[i]==signature,'first8 exact cross repetition')
   else:first_by_root[i]=signature
   useful=0;diagnostic=0
   for t in trials:
    outcome=t['result']['outcome'];counts[outcome]+=1
    demand(t['counts']['accepted_inputs']<=256 and t['counts']['advance_calls']+t['counts']['query_calls']<=100_000,'pertrial fixed cap')
    if outcome in ['terminal','horizon']:
     useful+=1;key=(i,t['world_index'],t['candidate_index']);value={k:t[k] for k in ['assignment_digest','candidate_id','counts','result']}
     if key in overlap:demand(overlap[key]==value,'all overlapping optional useful outcomes/counts/StateSHA exact');compared+=1
     else:overlap[key]=value
    elif outcome in ['not_started','resource_truncated']:
     diagnostic+=1
     if outcome=='not_started':demand(not any(t['counts'].values()),'notstarted claims no work')
    else:raise AssertionError('unexpected infeasible/fault outcome '+outcome)
   evidence.append({'index':i,'repetition':rep,'path':str(p),'sha256':sha(p),'useful_trials':useful,'diagnostic_stops':diagnostic,'candidate_count':len(candidates)})
   root_counts[str(i)]=len(candidates)
  sorted_latency=sorted(latencies);demand(sorted_latency[56]==top['p95_elapsed_micros']==wrapper['p95_elapsed_micros']<=3_000_000,'independent rank57 p95')
  peak=max(wrapper['wait4_peak_rss_bytes'],wrapper['observed_process_hwm_bytes'],wrapper['observed_process_rss_bytes'],*all_internal_peaks)
  demand(peak==wrapper['whole_process_peak_upper_bound_bytes']<=2*1024**3 and wrapper['whole_process_memory_met'],'whole-process kernel peak upper bound')
  demand(top['numeric_gate_met'] and top['all_candidates_minimum_met'] and top['all_hard_latency_met'] and top['all_worker_cpu_ids_verified'] and top['memory_met'] and not top['engine_or_policy_or_job_faults'],'no native gate discrepancy')
  result.update(status='static_measurement_audit_passed_replaying',attempts=60,p95_nearest_rank=57,p95_elapsed_micros=sorted_latency[56],minimum_elapsed_micros=min(latencies),maximum_elapsed_micros=max(latencies),whole_process_peak_bytes=peak,recorded_outcome_counts=dict(counts),cross_repetition_overlapping_useful_comparisons=compared,all12_first8_cross_repetitions_identical=True,candidates_per_root=root_counts,evidence=evidence,wrapper_sha256=sha(WRAPPER),report_sha256=sha(AREA/'report.json'));save()
  def replay(e):
   label=f"root-{e['index']:02}-repeat-{e['repetition']}";out=OUT/(label+'.json');err=OUT/(label+'.stderr.log');began=time.monotonic()
   owned=None
   try:
    demand(sha(EXE)==EXESHA and sha(Path(e['path']))==e['sha256'],'exact executable/evidence before replay')
    owned=INPUTS.run_owned([str(EXE),'search-replay',e['path']],ROOT,out,err,300,lambda:STOP)
    demand(owned['status']=='passed' and owned['direct_child_reaped'] and owned['owned_group_empty'],'native replay exit/cleanup')
    native=read(out);verify_replay(native,e)
    demand(sha(EXE)==EXESHA and sha(Path(e['path']))==e['sha256'],'exact executable/evidence after replay')
    return {'index':e['index'],'repetition':e['repetition'],'status':'passed','verified_trials':native['verified_trials'],'unverified_diagnostic_stops':native['unverified_diagnostic_stops'],'stdout_sha256':sha(out),'stderr_sha256':sha(err),'evidence_sha256':e['sha256'],'elapsed_seconds_diagnostic_only':time.monotonic()-began,'owned_process':owned}
   except BaseException as error:return {'index':e['index'],'repetition':e['repetition'],'status':'failed','failure':type(error).__name__+': '+str(error),'stdout_sha256':sha(out) if out.exists() else None,'stderr_sha256':sha(err) if err.exists() else None,'owned_process':owned}
  with ThreadPoolExecutor(max_workers=2) as pool:
   futures=[pool.submit(replay,e) for e in evidence]
   for future in as_completed(futures):
    result['replays'].append(future.result());save();print('independent replays finished',len(result['replays']),'of60',flush=True)
  result['source_end_guards']=guard();demand(result['source_start_guards']==result['source_end_guards'],'source guard endpoints exact')
  verify_replay_set(result['replays'])
  result['replays'].sort(key=lambda r:(r['repetition'],r['index']));result['verified_trials']=sum(r['verified_trials'] for r in result['replays']);result['unverified_diagnostic_stops']=sum(r['unverified_diagnostic_stops'] for r in result['replays']);publish_success();print(json.dumps({'status':'passed','verified_trials':result['verified_trials'],'unverified_diagnostic_stops':result['unverified_diagnostic_stops'],'report_sha256':sha(REPORT)}),flush=True)
 except BaseException as error:
  result['status']='failed';result['findings'].append(type(error).__name__+': '+str(error));result['traceback']=traceback.format_exc();save();raise

def stopped(_signal,_frame):
 global STOP
 STOP=True

def main():
 global STOP
 import signal
 STOP=False
 previous={sig:signal.getsignal(sig)for sig in [signal.SIGTERM,signal.SIGINT]}
 try:
  for sig in previous:signal.signal(sig,stopped)
  return _main()
 finally:
  for sig,handler in previous.items():signal.signal(sig,handler)

if __name__=='__main__':main()
