#!/usr/bin/env python3
"""One bounded fixed18 legal-pod batch. No builds, seed selection or proof waiver."""
import argparse
from collections import Counter
import hashlib,importlib.util,json,math,os,re,stat,sys
from pathlib import Path
P=Path(__file__).absolute().parent
ENTRY='match_games::current_pod_throughput::current_fixed18_pod_throughput_diagnostic'
DOMAIN='commander-foundations-v1/completion-batch/'
VECTORS={'advance_ns':'advances','observation_query_ns':'decisions','policy_ns':'decisions','validation_query_ns':'decisions','submit_ns':'decisions'}
SCALARS=['admission_ns','journal_serialization_ns','journal_io_ns','snapshot_envelope_ns','final_state_hash_ns','disposal_ns','generation_ns']
EVIDENCE={'config','policy','mulligans','journal','trace_mode','losses','checkpoints','outcome','winner','decisions','advances','rule_steps','query_calls','work_limit','quantum','turns','final_state_sha256','surviving_seats','survivor_counts','elapsed_millis'}
MAX_LINE=4*1024**2
# Full seed0 detailed trace/four checkpoints; control JSON keeps its own default.
POD_EVIDENCE_MAX_BYTES=256*1024**2

def need(ok,why):
 if not ok:raise ValueError(why)
def plain(path):
 p=Path(path);need(p.is_absolute()and'..'not in p.parts,'absolute plain path')
 for q in (p,*p.parents):need(not q.is_symlink(),'no symlink ancestry')
 need(p.is_file(),'plain file');return p

def sha(path):
 h=hashlib.sha256()
 with plain(path).open('rb')as f:
  for b in iter(lambda:f.read(1024**2),b''):h.update(b)
 return h.hexdigest()

def load(pin):
 f=P/'run-ordinary-diagnostic-v1.py';need(re.fullmatch('[0-9a-f]{64}',pin or '')and sha(f)==pin,'ordinary helper SHA before import')
 s=importlib.util.spec_from_file_location('current_pod_ordinary',f);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m

def source_decks(stage):
 # Parse only the exact finite Rust constructors, never infer cards from names.
 paths=[stage/'packages/game-engine/crates/core/src/engine.rs',stage/'packages/game-engine/crates/core/src/engine/match_admission.rs',stage/'packages/game-engine/crates/core/src/engine/choices.rs']
 texts=[plain(p).read_text()for p in paths];symbols={}
 for name,value in re.findall(r'\bconst\s+([A-Z_0-9]+)\s*:\s*&str\s*=\s*"([^"]*)"\s*;', '\n'.join(texts)):
  need(name not in symbols or symbols[name]==value,'ambiguous source constant');symbols[name]=value
 decks=[]
 for name in ['white_deck','black_green_deck']:
  m=re.search(r'pub fn '+name+r'\(\) -> Decklist \{(.*?)\n\}',texts[1],re.S);need(m is not None,'finite source deck constructor')
  body=re.fullmatch(r'\s*Decklist\s*\{\s*commander:\s*([A-Z_0-9]+)\.into\(\),\s*cards:\s*entries\(&\[(.*?)\]\),\s*\}\s*',m[1],re.S);need(body is not None,'unsupported deck source shape')
  pairs=re.findall(r'\(\s*([A-Z_0-9]+)\s*,\s*([0-9]+)\s*\)',body[2]);need(re.sub(r'\(\s*[A-Z_0-9]+\s*,\s*[0-9]+\s*\)','',body[2]).replace(',','').strip()=='','no omitted deck expressions')
  cards=sorted([{'oracle_id':symbols[n],'count':int(c)}for n,c in pairs],key=lambda r:r['oracle_id'])
  need(sum(r['count']for r in cards)==100 and len({r['oracle_id']for r in cards})==len(cards),'source100 rows unique')
  commander=symbols[body[1]];need(any(r=={'oracle_id':commander,'count':1}for r in cards),'commander counted inside100')
  decks.append({'commander':commander,'cards':cards})
 return symbols['MATCH_PROFILE'],decks

def config(index,profile,decks):
 need(type(index)is int and index in range(18),'fixed case0..17')
 pod=[decks[0],decks[1],decks[0],decks[1]]if index<16 else[decks[index-16]]*4
 return {'profile':profile,'decks':pod,'starting_player':index%4,'seed':list(hashlib.sha256((DOMAIN+str(index)).encode()).digest())}

def same(o,a,b,why):need(o.canonical(a)==o.canonical(b),why)
def n(o,v,why):need(o.natural(v),why);return v

def baseline_features(artifact):
 need(artifact.get('features')==[],'actual SIM featureOFF required; shared engine Build alone does not bind instrumentation')

def auth(o,fixed,path,pin):
 a=fixed.authority_check(path,pin);fixed.require_target(a,fixed.SIM_TARGET)
 baseline_features(fixed.host.read_json(plain(a['compiler_artifact']['path'])))
 stage=Path(a['stage_root']);need(stage==P.parents[1]and a['timeout_seconds']==120 and len(a['cpu_ids'])==1,'one isolated SIM/CPU/120s')
 need(a['current_pod_cases']==list(range(18))and all(type(i)is int for i in a['current_pod_cases'])and a['current_pod_entrypoint']==ENTRY,'exact fixed18 entry')
 sources={'pod_source_sha256':stage/'packages/game-engine/crates/sim/src/match_games/current_pod_throughput.rs','match_source_sha256':stage/'packages/game-engine/crates/sim/src/match_games.rs','admission_source_sha256':stage/'packages/game-engine/crates/core/src/engine/match_admission.rs'}
 for key,p in sources.items():need(sha(p)==a[key],'exact actual current native source')
 for name in ['run-current-pods-v1.py','run-ordinary-diagnostic-v1.py','run-fixed-work.py','measurement_host.py']:
  need(a['host_scripts'].get(str(P/name))==sha(P/name),'exact host source')
 text=sources['match_source_sha256'].read_text();need('sha256(format!("'+DOMAIN+'{index}").as_bytes())'in text,'frozen source seed domain')
 limits={k:int(v.replace('_',''))for k,v in re.findall(r'const (DECISION_LIMIT|TURN_LIMIT|WORK_LIMIT):\s*\w+\s*=\s*([0-9_]+);',text)}
 need(limits=={'DECISION_LIMIT':100000,'TURN_LIMIT':1000,'WORK_LIMIT':2000000},'fixed source ceilings')
 ref=a['current_pod_correctness'];need(sha(Path(ref['path']))==ref['sha256'],'correctness reference pin');proof=fixed.host.read_json(Path(ref['path']))
 need(proof['status']=='passed_current_pod_instrumentation_parity_clippy_fmt'and proof['source_sha256']==a['pod_source_sha256'],'current fullEvidence parity proof required')
 decks=source_decks(stage);catalog=fixed.host.read_json(stage/'packages/game-engine/tests/fixtures/expected-match-ir.json');ids={r['oracle_id']for r in catalog['definitions']}
 need(all(d['commander']in ids and all(c['oracle_id']in ids for c in d['cards'])for d in decks[1]),'source deck IDs all in pinned catalog')
 return a,decks,limits

def clocks(o,c,e):
 need(set(c)==set(VECTORS)|set(SCALARS),'complete component clock fields')
 for key,field in VECTORS.items():need(type(c[key])is list and len(c[key])==e[field]and all(o.natural(v)for v in c[key]),'all actual component vector entries')
 need(all(o.natural(c[k])for k in SCALARS)and c['generation_ns']>0,'component scalar clocks')
 need(c['generation_ns']>=sum(sum(c[k])for k in VECTORS)+sum(c[k]for k in SCALARS if k!='generation_ns'),'disjoint components inside generation')
 return {k:len(c[k])for k in VECTORS}

def evidence(o,e,index,profile,decks):
 need(set(e)==EVIDENCE,'full native Evidence shape')
 same(o,e['config'],config(index,profile,decks),'seed/start/profile/source-derived exact decklists')
 need(e['policy']=='develop'and e['mulligans']==[(index+s)%3 for s in range(4)]and all(type(v)is int for v in e['mulligans']),'unchanged policy/mulligans')
 need(e['trace_mode']==('detailed'if index==0 else'off'),'exact trace mode')
 tags=o.journal(e['journal']);need(not e['journal']['concessions']and e['decisions']==len(tags),'no concessions/input conservation')
 need(e['work_limit']==2000000 and type(e['work_limit'])is int and e['quantum']==1024 and type(e['quantum'])is int,'same host ceilings and quantum')
 need(all(o.natural(e[k])for k in ['decisions','advances','rule_steps','query_calls','turns','elapsed_millis']),'actual structural counts')
 need(e['decisions']<=100000 and e['turns']<=1000 and e['advances']+e['query_calls']<=2000000,'actual ceilings')
 need(e['advances']>=e['decisions']+1 and e['query_calls']>=2*e['decisions'],'automatic/query input conservation')
 need(o.hexsha(e['final_state_sha256']),'final fullState SHA')
 seats=e['surviving_seats'];need(type(seats)is list and all(type(v)is int and v in range(4)for v in seats)and len(seats)==len(set(seats)),'surviving seat set')
 survivors=e['survivor_counts'];need(type(survivors)is list and survivors and survivors[0]==4 and survivors[-1]==len(seats)and all(type(v)is int and v in range(5)for v in survivors)and all(a>b for a,b in zip(survivors,survivors[1:])),'full decreasing elimination sequence')
 if e['outcome']=='finished':need(type(e['winner'])is int and seats==[e['winner']],'terminal winner must be last survivor')
 elif e['outcome']=='draw':need(e['winner']is None and seats==[],'terminal draw must have no survivors')
 else:need(e['winner']is None,'fault/host stop not a rules winner')
 need(type(e['checkpoints'])is list and len(e['checkpoints'])==(4 if index==0 else 0),'seed0 four checkpoints only')
 previous=(-1,-1)
 for point in e['checkpoints']:
  need(set(point)=={'decisions','rule_boundary','snapshot'}and o.natural(point['decisions'])and o.natural(point['rule_boundary'])and type(point['snapshot'])is str and point['snapshot'],'full retained checkpoint envelope')
  now=(point['decisions'],point['rule_boundary']);need(now[0]>=previous[0]and now[1]>=previous[1]and point['decisions']<=e['decisions']and point['rule_boundary']<=e['rule_steps'],'checkpoint prefix boundary');previous=now
 if index==0:need(e['checkpoints'][0]['decisions']==0 and e['checkpoints'][-1]['decisions']==e['decisions']and e['checkpoints'][-1]['rule_boundary']==e['rule_steps'],'initial/final checkpoints')
 if index!=0:need(e['losses']==[],'Off mode cannot claim typed loss rows')
 else:
  need(type(e['losses'])is list and all(type(x)is list and len(x)==2 and type(x[0])is int and x[0]in range(4)and type(x[1])is str for x in e['losses']),'typed loss rows')
  need(len({x[0]for x in e['losses']})==len(e['losses'])==4-len(seats),'once-only typed loss facts')
 return Counter(tags)

def stream(o,h,path,e,index,profile,decks,build):
 expected_header={'kind':'header','build_id':build,'config':config(index,profile,decks),'policy':'develop','mulligans':[(index+s)%3 for s in range(4)]}
 accepted=0;footer=False
 with plain(path).open('rb')as f:
  for number,raw in enumerate(f):
   need(len(raw)<=MAX_LINE and raw.endswith(b'\n'),'bounded complete journal lines')
   v=json.loads(raw,object_pairs_hook=h.no_duplicates,parse_constant=lambda _:(_ for _ in()).throw(ValueError('nonfinite')))
   if number==0:same(o,v,expected_header,'actual streamed header');continue
   need(not footer,'no trailing records after footer')
   if v.get('kind')=='accepted':
    need(set(v)=={'kind','input'}and accepted<len(e['journal']['decisions']),'actual accepted stream count');same(o,v['input'],e['journal']['decisions'][accepted],'byte-equivalent accepted journal input');accepted+=1
   else:
    same(o,v,{'kind':'footer','outcome':e['outcome'],'winner':e['winner'],'state_sha256':e['final_state_sha256'],'rule_boundary':e['rule_steps']},'terminal/fault footer');footer=True
 need(footer and accepted==e['decisions'],'all actual inputs and footer retained')
 return {'sha256':sha(path),'accepted_inputs':accepted}

def validate_batch(o,h,directory,report,build,source_decks_value):
 profile,decks=source_decks_value
 need(report['schema']=='current-fixed18-pod-throughput-v1'and report['build_id']==build,'native batch identity')
 need(report['planned_pods']==18 and type(report['planned_pods'])is int and len(report['rows'])==18,'all18 denominator')
 need(type(report['passed'])is bool and report['no_search_acceptance_or_strategy_quality_claim']is True,'honest batch status/scope')
 need(all(o.natural(report[k])for k in ['completed_pods','catalog_decode_ns','compiler_verify_ns','generation_sum_ns','independent_proof_sum_ns','whole_batch_ns']),'actual batch scalar clocks')
 rows=[];retained=[];hist=Counter();wins=[0]*4;completed=0;generation=0;proof=0;persist=0;expected_failures=[]
 for index,row in enumerate(report['rows']):
  need(type(row['index'])is int and row['index']==index,'fixed ordered18 cases')
  if row.get('phase')=='before complete Evidence':
   need(type(row['error'])is str and o.natural(row['attempt_wall_ns'])and row['partial_stream']==str(directory/f'game-{index:02}.jsonl'),'failed attempt retained')
   need(report['passed']is False,'missing Evidence prevents pass');hist['failed_before_evidence']+=1;generation+=row['attempt_wall_ns'];expected_failures.append(row);rows.append({'index':index,'outcome':'failed_before_evidence','counts':'unknown','attempt_wall_ns':row['attempt_wall_ns']});continue
  path=directory/f'game-{index:02}.json';need(row['evidence_file']==str(path)and sha(path)==row['evidence_sha256'],'exact private Evidence file SHA')
  e=h.read_json(plain(path),max_bytes=POD_EVIDENCE_MAX_BYTES);tags=evidence(o,e,index,profile,decks)
  for k in ['outcome','winner','survivor_counts','turns','decisions','advances','rule_steps','query_calls','final_state_sha256']:same(o,row[k],e[k],'full evidence/report '+k)
  need(row['journal_sha256']==o.digest(e['journal']),'accepted journal SHA')
  stream_proof=stream(o,h,directory/f'game-{index:02}.jsonl',e,index,profile,decks,build)
  vector_counts=clocks(o,row['clocks'],e);generation+=row['clocks']['generation_ns']
  retained.extend([{'path':str(path),'bytes':path.stat().st_size,'sha256':row['evidence_sha256']},{'path':str(directory/f'game-{index:02}.jsonl'),'bytes':(directory/f'game-{index:02}.jsonl').stat().st_size,'sha256':stream_proof['sha256']}])
  need(all(o.natural(row[k])for k in ['evidence_encode_ns','evidence_io_ns','independent_replay_ns']),'actual proof/persist clocks');proof+=row['independent_replay_ns'];persist+=row['evidence_encode_ns']+row['evidence_io_ns']
  need(type(row['verified'])is bool,'proof bool');terminal=e['outcome']in['finished','draw'];need(not row['verified']or terminal,'fault/truncation cannot verify as terminal')
  if row['verified']:completed+=1
  else:
   need(report['passed']is False,'unverified outcome prevents throughput');expected_failures.append({'index':index,'outcome':e['outcome']})
  hist[e['outcome']]+=1
  if e['winner']is not None:wins[e['winner']]+=1
  rows.append({'index':index,'outcome':e['outcome'],'verified':row['verified'],'decisions':e['decisions'],'rule_steps':e['rule_steps'],'turns':e['turns'],'response_histogram':dict(tags),'vector_counts':vector_counts,'full_state_sha256':e['final_state_sha256'],'evidence_sha256':row['evidence_sha256'],'journal':stream_proof})
  del e  # Release this full game before the next read/parse, including seed0.
 need(o.hist(report['outcome_histogram'])==dict(hist)and type(report['winner_counts'])is list and all(o.natural(v)for v in report['winner_counts'])and report['winner_counts']==wins and report['completed_pods']==completed,'all raw outcomes/winners/verified completion denominator')
 need(report['generation_sum_ns']==generation and report['independent_proof_sum_ns']==proof and report['whole_batch_ns']>=generation+proof+persist,'actual clock conservation')
 need(type(report['failures'])is list and all(type(f.get('index'))is int for f in report['failures'])and [f.get('index')for f in report['failures']]==[f['index']for f in expected_failures],'every failed case retained once')
 need(type(report['loss_reason_coverage'])is bool,'native typed loss coverage bool')
 passed=completed==18 and not report['failures']and report['loss_reason_coverage'];need(report['passed']is passed,'all18/native replay/required life+commander coverage before rates')
 if passed:
  need({f.name for f in directory.iterdir()}=={'report.json'}|{f'game-{i:02}.{ext}'for i in range(18)for ext in ['json','jsonl']},'exact complete Evidence/journal closure')
  for key,denom in [('generation_pods_per_second',generation),('whole_batch_pods_per_second',report['whole_batch_ns'])]:
   need(denom>0 and type(report[key])in(float,int)and math.isfinite(report[key])and math.isclose(report[key],18e9/denom,rel_tol=1e-12),'all18 exact throughput quotient')
 else:need(report['generation_pods_per_second']is None and report['whole_batch_pods_per_second']is None,'no rate on incomplete batch')
 retained.append({'path':str(directory/'report.json'),'bytes':(directory/'report.json').stat().st_size,'sha256':sha(directory/'report.json')})
 return {'retained_files':retained,'all18_complete_verified':passed,'planned_pods':18,'completed_pods':completed,'outcome_histogram':dict(hist),'reported_winner_counts':wins,'rows':rows,'generation_sum_ns':generation,'independent_proof_sum_ns':proof,'whole_batch_ns':report['whole_batch_ns'],'native_oracle_scope':'source-bound verify_suite_entry and full independent Rust replay/CheckpointVerifier remain authoritative; host independently validates retained config/input/outcome/count/clock consistency, not a new Rust replay','statistical_scope':'one heterogeneous18pod batch; root must predeclare warmup+atleast5/10 batches and keep every failure before variance claim'}

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--ordinary-driver-sha256',required=True);ap.add_argument('--fixed-driver-sha256',required=True);ap.add_argument('--helper-sha256',required=True);ap.add_argument('--authority',type=Path,required=True);ap.add_argument('--authority-sha256',required=True);ap.add_argument('--output',type=Path,required=True);ap.add_argument('--execute',action='store_true');x=ap.parse_args();need(x.execute,'root explicit execution required');os.umask(0o077)
 o=load(x.ordinary_driver_sha256);fixed=o.load_fixed(x.fixed_driver_sha256);h=fixed.load_host(x.helper_sha256);a,decks,limits=auth(o,fixed,x.authority,x.authority_sha256)
 h.real_path(x.output.parent,private=True);need(not x.output.exists(),'fresh output');x.output.mkdir(mode=0o700);scratch=x.output/'tmp';scratch.mkdir(mode=0o700);games=x.output/'games'
 def check():need(auth(o,fixed,x.authority,x.authority_sha256)==(a,decks,limits),'source/authority endpoint')
 def audit():
  check();return validate_batch(o,h,games,h.read_json(plain(games/'report.json')),a['engine_build_id'],decks)
 try:r=h.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',ENTRY],cwd=Path(a['stage_root']),env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','TMPDIR':str(scratch),'MTG_CURRENT_POD_OUTPUT':str(games)},directory=x.output/'execution',label='current18pods',cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
 except BaseException as e:r={'status':'failed','failure':{'type':type(e).__name__,'reason':str(e)[:2048]},'resource_report_created':False}
 audit_value=r.get('audit')or{}
 passed=r.get('status')=='passed'and bool(audit_value.get('all18_complete_verified'));failure=r.get('failure')
 try:check()
 except Exception as e:passed=False;failure={'type':type(e).__name__,'reason':str(e)[:2048]}
 # Failed attempts retain all raw streams/files without invented counts/rates.
 retained=list(audit_value.get('retained_files',[]))
 if not passed and games.exists():
  retained=[]
  for f in sorted(games.iterdir()):
   if f.is_file():retained.append({'path':str(f),'bytes':f.stat().st_size,'sha256':sha(f)})
 result={'schema':'bounded-current18-pod-batch-v1','status':'all18_complete_and_verified'if passed else'failed_or_truncated_no_throughput','engine_build_id':a['engine_build_id'],'authority_sha256':x.authority_sha256,'failure':failure,'audit':r.get('audit'),'retained_files':retained,'post_cleanup_inventory_scope':'retained raw bytes only, outside failed-row measurement if interrupted; no fabricated terminal/count','host_report_sha256':sha(x.output/'execution/host-report.json')if(x.output/'execution/host-report.json').exists()else None,'generation_pods_per_second':(18e9/r['audit']['generation_sum_ns'])if passed else None,'whole_batch_pods_per_second':(18e9/r['audit']['whole_batch_ns'])if passed else None,'variance_or_scaling_claim':False,'one_batch_only':True}
 h.save_json(x.output/'result.json',result);print(json.dumps({'status':result['status']}));return 0 if passed else 1
if __name__=='__main__':sys.exit(main())
