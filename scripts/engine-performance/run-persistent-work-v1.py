#!/usr/bin/env python3
"""Current-Build persistent-pool diagnostic only. Never a build or product gate."""
import argparse,hashlib,importlib.util,json,os,re,statistics,sys
from pathlib import Path
P=Path(__file__).absolute().parent
STAGE=P.parents[1]
ENTRY='search_jobs::fixed_work::persistent::fixed_work_persistent_diagnostic_row'
SCHEMA='fixed-persistent-host-plan-v1'
STEADY='dispatch+fresh world builds+all existing trials/queries/StateSHA/disposal+results+sorted aggregation; existing256 horizon unchanged'
MAX_REPORT=64*1024**2

def need(ok,why):
 if not ok:raise ValueError(why)
def load_fixed(pin):
 p=P/'run-fixed-work.py'
 need(not p.is_symlink()and re.fullmatch('[0-9a-f]{64}',pin)is not None and hashlib.sha256(p.read_bytes()).hexdigest()==pin,'fixed helper SHA before import')
 spec=importlib.util.spec_from_file_location('authenticated_persistent_fixed',p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def exact(f,a,b):return f.canonical(a)==f.canonical(b)
def number(f,x,why):need(f.natural(x),why);return x
def read_json(f,path):
 f.host.real_path(path);need(path.is_file()and path.stat().st_size<=MAX_REPORT,'bounded regular report')
 # Shared reader rejects duplicate keys/nonfinite values.
 return f.host.read_json(path)

def authority(f,path,pin):
 a=f.authority_check(path,pin);f.require_target(a,f.SIM_TARGET)
 artifact=a['compiler_artifact'];f.pinned(Path(artifact['path']),artifact['sha256'])
 need(read_json(f,Path(artifact['path'])).get('features')==[],'baseline SIM feature-off artifact required; Build identity alone omits instrumentation features')
 need(Path(a['stage_root'])==STAGE and a['timeout_seconds']==120,'isolated stage/120s')
 need(a['persistent_entrypoint']==ENTRY and a['persistent_root']==0 and type(a['persistent_root'])is int and a['persistent_worlds']==32 and type(a['persistent_worlds'])is int,'fixed root0/32world scope')
 for p in [Path(__file__).absolute(),P/'run-fixed-work.py',P/'measurement_host.py']:
  need(a['host_scripts'].get(str(p))==f.host.sha(p),'current host script pins')
 source=STAGE/'packages/game-engine/crates/sim/src/search_jobs/fixed_work/persistent.rs'
 need(f.host.sha(source)==a['persistent_source_sha256'],'actual compiled persistent module')
 text=source.read_text();need('const PERSISTENT_REPETITIONS: usize = 6;'in text and STEADY in text,'native repetitions/steady scope')
 proof=a['persistent_correctness'];f.pinned(Path(proof['path']),proof['sha256']);v=read_json(f,Path(proof['path']))
 need(v['status']=='passed_persistent_focused_correctness_clippy_fmt'and v['source_sha256']==a['persistent_source_sha256'],'current persistent correctness required')
 return a

def validate_report(f,r,cpus,available,build):
 need(r['schema']=='fixed-persistent-pool-diagnostic-v1'and r['build_id']==build and type(r['root'])is int and r['root']==0,'currentBuild/root0 report')
 need(exact(f,r['worker_cpu_ids'],cpus)and exact(f,r['available_physical_cpu_ids'],available),'actual physical affinity inventory')
 need(type(r['warmups'])is int and r['warmups']==1 and type(r['measured_repeats'])is int and r['measured_repeats']==5,'one warmup/five measurements')
 need(r['startup_excluded_from_steady_clock']is True and r['shutdown_excluded_from_steady_clock']is True and r['whole_host_still_includes_startup_oracle_warmup_audit_shutdown']is True and r['no_product_latency_claim']is True and r['steady_scope']==STEADY,'separate honest clock scopes')
 work=r['frozen_work'];reference=r['reference'];worlds=work['worlds'];c=work['candidate_policy']
 need(work['version']=='foundations-fixed-search-work-v1'and type(work['horizon'])is int and work['horizon']==256,'fixed work version/horizon')
 need(exact(f,work['public_job_seed'],list(hashlib.sha256(b'commander-open-search-v1/job/0').digest())),'fixed public job seed')
 need(type(worlds)is list and len(worlds)==32 and exact(f,[w['world_index']for w in worlds],list(range(32)))and all(f.hexsha(w['assignment_digest'])and f.natural(w['sampling_attempt'])for w in worlds)and len({w['assignment_digest']for w in worlds})==32,'complete32distinct worlds')
 need(c['truncated_at_limit']is False and len(c['candidates'])==3 and len({x['candidate_id']for x in c['candidates']})==3,'exact96trial full candidate scope')
 for w in worlds:need(w['proposal']['context_id']==work['context_id']and type(w['proposal']['world_index'])is int and w['proposal']['world_index']==w['world_index'],'proposal/work context identity')
 need(reference['work_sha256']==f.digest(work),'full frozen work SHA')
 f.semantic_trials(work,reference['trials'])
 need(len(reference['trials'])==96 and exact(f,reference['aggregates'],f.aggregates(work,reference['trials'])),'independent ordinary execute reference96trials/aggregates')
 need(len(reference['world_build_calls'])==32,'all reference world build counts')
 for i,row in enumerate(reference['world_build_calls']):need(type(row)is list and len(row)==2 and type(row[0])is int and row[0]==i and f.natural(row[1])and row[1]>0,'complete positive ordinary world builds')
 for t in reference['trials']:
  need(type(t['world_index'])is int and type(t['candidate_index'])is int,'trial ordinal integer')
  need(exact(f,sorted(v['player']for v in t['result']['evaluations']),[0,1,2,3]),'utility player integers')
  winner=t['result'].get('winner');need(winner is None or(type(winner)is int and winner in range(4)),'winner integer')
 rows=r['rows'];schedules=rows['schedules'];need(type(schedules)is list and len(schedules)==6,'six complete persistent schedules')
 for key in ['startup_micros','shutdown_micros','lifecycle_micros']:need(number(f,rows[key],'clock integer')<=120_000_000,'bounded lifecycle clock')
 need(rows['startup_micros']>0 and rows['shutdown_micros']>0,'actual startup/shutdown observations')
 elapsed=[];operations=[]
 for row in schedules:
  need(type(row['workers'])is int and row['workers']==len(cpus),'worker count integer')
  elapsed.append(f.validate_schedule(row,work,cpus,reference))
  need(exact(f,row['semantics'],reference),'typed complete semantic reference equality')
  for world in row['worlds']:
   need(type(world['world_index'])is int,'world ordinal integer')
   for trial in world['trials']:
    need(type(trial['world_index'])is int and type(trial['candidate_index'])is int,'raw trial ordinal integer')
    need(exact(f,sorted(v['player']for v in trial['result']['evaluations']),[0,1,2,3]),'raw utility player integers')
    winner=trial['result'].get('winner');need(winner is None or(type(winner)is int and winner in range(4)),'raw winner integer')
  need(row['elapsed_micros']<=120_000_000,'bounded steady interval')
  for m in row['worker_measurements']:
   need(type(m['worker_index'])is int and type(m['cpu_id'])is int and exact(f,m['completed_world_indices'],list(range(m['worker_index'],32,len(cpus)))),'deterministic complete per-lane world partition')
   need(f.natural(m['elapsed_micros'])and m['elapsed_micros']<=row['elapsed_micros'],'worker clock inside steady interval')
  operations.append(number(f,row['strategy_operations'],'actual strategy work integer'))
 need(operations[0]>0 and len(set(operations))==1,'identical actual work across persistent generations')
 need(rows['lifecycle_micros']>=rows['startup_micros']+rows['shutdown_micros']+sum(elapsed),'lifecycle includes ALL six schedules plus startup/shutdown')
 return {'work':work,'semantics':reference,'work_sha256':f.digest(work),'semantics_sha256':f.digest(reference),'useful_trials_per_schedule':96,'strategy_operations':operations[0],'startup_micros':rows['startup_micros'],'shutdown_micros':rows['shutdown_micros'],'lifecycle_micros':rows['lifecycle_micros'],'warmup_steady_micros':elapsed[0],'measured_steady_micros':elapsed[1:],'median_steady_micros':statistics.median(elapsed[1:])}

def validate_stdout(f,path,report,workers):
 f.host.real_path(path);need(path.stat().st_size<=MAX_REPORT,'bounded raw stdout');raw=path.read_bytes();need(b'\0'not in raw and raw.endswith(b'\n'),'complete stdout bytes')
 marker=re.compile(r'^(?:test '+re.escape(ENTRY)+r' \.\.\. )?(PERSISTENT_SCHEDULE_START|PERSISTENT_SCHEDULE_RESULT)=(.*)$')
 header=False;trailer=False;pending=False;generation=0
 for line in raw.decode('utf-8').splitlines():
  if not line:continue
  if line=='running 1 test':need(not header,'one test header');header=True;continue
  m=marker.fullmatch(line)
  if m:
   need(header and not trailer and generation<6,'marker within test')
   v=json.loads(m[2],object_pairs_hook=f.host.no_duplicates,parse_constant=lambda _:(_ for _ in()).throw(ValueError('nonfinite JSON')))
   if m[1]=='PERSISTENT_SCHEDULE_START':
    need(not pending and exact(f,v,{'generation':generation,'world_order':list(range(32)),'workers':workers}),'exact generation start/order');pending=True
   else:
    need(pending and exact(f,v,{'generation':generation,'complete_row':report['rows']['schedules'][generation]}),'streamed/final COMPLETE schedule equality');pending=False;generation+=1
   continue
  if line in ['ok','test '+ENTRY+' ... ok','test '+ENTRY+' ... ']:need(header and not trailer,'test status order');continue
  if re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(?:\.[0-9]+)?s',line):need(header and not trailer and generation==6 and not pending,'exact successful completed test');trailer=True;continue
  raise ValueError('unknown native stdout')
 need(header and trailer and generation==6 and not pending,'complete native persistent lifecycle')
 return {'raw_stdout_sha256':f.host.sha(path),'six_streamed_schedule_rows_equal_final':True}

def compare(f,rows,expected):
 need([r['workers']for r in rows]==expected and all(r['host_status']=='passed'for r in rows),'all currentBuild rows passed before scaling')
 base=rows[0]['validated'];values=[]
 for row in rows:
  v=row['validated'];need(exact(f,v['work'],base['work'])and exact(f,v['semantics'],base['semantics'])and v['strategy_operations']==base['strategy_operations'],'byte-identical SAME-currentBuild frozenwork/full State/count/outcome semantics')
  values.append({'workers':row['workers'],'median_steady_micros':v['median_steady_micros'],'startup_micros':v['startup_micros'],'shutdown_micros':v['shutdown_micros'],'persistent_lifecycle_micros':v['lifecycle_micros'],'observed_current_build_steady_speedup':base['median_steady_micros']/v['median_steady_micros'],'useful_trials_per_schedule':96})
 return values

def plan_values(f,v,a):
 need(v['schema']==SCHEMA and v['status']=='prepared_only','prepared plan')
 available=f.host.physical_cpus();need(exact(f,v['available_physical_cpu_ids'],available)and 4<=len(available)<=64,'current physical topology')
 need(exact(f,v['worker_counts'],list(dict.fromkeys([1,2,4,len(available)]))),'complete1/2/4/all physical rows')
 need(v['root']==0 and type(v['root'])is int and v['worlds']==32 and type(v['worlds'])is int and type(v['timeout_seconds'])is int and v['timeout_seconds']==120 and type(v['memory_bytes'])is int and v['memory_bytes']==2**31,'fixed work/resource scope')
 for name,sha in v['host_sources'].items():f.pinned(Path(name),sha)
 need(v['host_sources']==a['host_scripts'],'whole approved host-source map')

def plan_guard(f,path,pin):
 f.pinned(path,pin);v=read_json(f,path)
 a=authority(f,Path(v['authority_path']),v['authority_sha256']);plan_values(f,v,a);return v,a

def execute(f,path,pin):
 v,a=plan_guard(f,path,pin);h=f.host;out=Path(v['output']);h.real_path(out.parent,private=True);need(not out.exists()and not out.is_symlink(),'fresh output');out.mkdir(mode=0o700)
 status={'schema':'fixed-persistent-host-result-v1','status':'running','classification':'current-Build persistent-pool diagnostic; no product latency or causal comparison to old303 build','rows':[],'plan_sha256':pin}
 h.save_json(out/'status.json',status)
 try:
  for workers in v['worker_counts']:
   cpus=v['available_physical_cpu_ids'][:workers];directory=out/f'workers-{workers}';payload=directory/'native-row.json';validated={}
   env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','MTG_FIXED_SEARCH_OUTPUT':str(payload),'MTG_FIXED_SEARCH_ROOT':'0','MTG_FIXED_SEARCH_WORLDS':'32','MTG_FIXED_SEARCH_CPUS':json.dumps(cpus)}
   def audit():
    need(plan_guard(f,path,pin)==(v,a),'immutable authority/plan endpoints')
    report=read_json(f,payload);validated.update(validate_report(f,report,cpus,v['available_physical_cpu_ids'],a['engine_build_id']))
    result=validate_stdout(f,directory/'workload.stdout',report,workers)
    if status['rows']:
     baseline=status['rows'][0]['validated']
     need(exact(f,validated['work'],baseline['work'])and exact(f,validated['semantics'],baseline['semantics'])and validated['strategy_operations']==baseline['strategy_operations'],'same current-Build work/reference before any further row')
    result.update(raw_report_sha256=h.sha(payload),semantics_sha256=validated['semantics_sha256'],useful_trials_per_schedule=96);return result
   try:r=h.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',ENTRY],cwd=STAGE,env=env,directory=directory,label=f'persistent-root-0-w{workers}',cpus=v['available_physical_cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
   except BaseException as error:r={'status':'failed','failure':{'type':type(error).__name__,'reason':str(error)[:2048]},'resource_report_created':False}
   status['rows'].append({'workers':workers,'host_status':r['status'],'host_report_sha256':h.sha(directory/'host-report.json')if(directory/'host-report.json').exists()else None,'validated':validated,'failure':r.get('failure')});h.save_json(out/'status.json',status)
   need(r['status']=='passed','failed row forbids scaling; preserved partial stdout/report')
  need(plan_guard(f,path,pin)==(v,a),'final immutable authority/plan')
  status.update(status='complete_same_build_semantics_verified_diagnostic',comparisons=compare(f,status['rows'],v['worker_counts']))
 except BaseException as error:status.update(status='failed',failure={'type':type(error).__name__,'reason':str(error)[:2048]})
 h.save_json(out/'status.json',status);return status

def main():
 p=argparse.ArgumentParser();p.add_argument('--fixed-driver-sha256',required=True);p.add_argument('--helper-sha256',required=True);p.add_argument('--authority',type=Path);p.add_argument('--authority-sha256');p.add_argument('--prepare-plan',type=Path);p.add_argument('--output',type=Path);p.add_argument('--execute-plan',type=Path);p.add_argument('--plan-sha256');x=p.parse_args();os.umask(0o077)
 f=load_fixed(x.fixed_driver_sha256);h=f.load_host(x.helper_sha256)
 if x.execute_plan:
  r=execute(f,x.execute_plan,x.plan_sha256);print(json.dumps({'status':r['status']}));return 0 if r['status']=='complete_same_build_semantics_verified_diagnostic'else 1
 need(x.authority is not None and x.authority_sha256 and x.prepare_plan is not None and x.output is not None,'explicit prepare inputs; never automatic execution')
 a=authority(f,x.authority,x.authority_sha256);h.real_path(x.prepare_plan.parent,private=True);h.real_path(x.output.parent,private=True)
 need(not x.prepare_plan.exists()and not x.prepare_plan.is_symlink()and not x.output.exists()and not x.output.is_symlink(),'exclusive prepared plan/output')
 available=h.physical_cpus();v={'schema':SCHEMA,'status':'prepared_only','authority_path':str(x.authority),'authority_sha256':x.authority_sha256,'root':0,'worlds':32,'available_physical_cpu_ids':available,'worker_counts':list(dict.fromkeys([1,2,4,len(available)])),'timeout_seconds':120,'memory_bytes':2**31,'host_sources':a['host_scripts'],'output':str(x.output)}
 # Reuse plan validation before publishing an execution command; no run.
 plan_values(f,v,a);h.save_json(x.prepare_plan,v);pin=h.sha(x.prepare_plan);plan_guard(f,x.prepare_plan,pin);print(json.dumps({'status':'prepared_only','plan_sha256':pin}));return 0
if __name__=='__main__':sys.exit(main())
