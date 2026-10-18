#!/usr/bin/env python3
"""Source-only thin host for frozen synthetic dense diagnostics. Never builds.

One case/process. Native legal ordinary and dense256 retention remain separate
future rows: this source's retained_case is synthetic ordinary8, no Commander.
"""
from __future__ import annotations
import argparse
from collections import Counter
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys

HERE=Path(__file__).absolute().parent
STAGE=HERE.parents[1]
ENTRIES={'kernel':'engine::dense_kernel_tests::diagnostic::dense_kernel_diagnostic_host_adapter',
         'interaction':'engine::dense_kernel_tests::interactions::dense_interaction_host_diagnostic'}
KERNEL_CASES=['ordinary','moderate','dense','large_synthetic','dense_history','dense_combined','effects','retained_branches']
INTERACTION_CASES=['triggers','damage','targets','combat']
ALL_CASES=[{'family':f,'case':c} for f,cs in [('kernel',KERNEL_CASES),('interaction',INTERACTION_CASES)] for c in cs]
MEMORY=2*1024**3
MAX_STDOUT=64*1024**2
SEGMENTS=['observe_all_four','characteristics_all_objects','fork','check_mana_response','submit_mana_response','advance_mana_to_next_request','full_state_hash_cold','full_state_hash_warm','dispose_changed_branch']
MARKERS=['DENSE_DIAGNOSTIC','DENSE_RETAINED_SWEEP_START','DENSE_RETAINED_SWEEP_RESULT','DENSE_INTERACTION_START','DENSE_INTERACTION_MEMORY','DENSE_INTERACTION_RESULT']


def require(ok,why):
    if not ok: raise ValueError(why)


def plain(path):
    p=Path(path)
    require(p.is_absolute() and '..' not in p.parts,'absolute plain path required')
    for q in (p,*p.parents): require(not stat.S_ISLNK(q.lstat().st_mode),'symlink ancestry')
    require(stat.S_ISREG(p.stat().st_mode),'plain regular file required')
    return p


def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()


def hexsha(x):return type(x) is str and re.fullmatch('[0-9a-f]{64}',x) is not None

def natural(x):return type(x) is int and x>=0

def pairs(items):
    d={}
    for k,v in items:
        require(k not in d,'duplicate JSON key');d[k]=v
    return d


def decode(text):
    return json.loads(text,object_pairs_hook=pairs,parse_constant=lambda _ : (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()


def load_fixed(pin):
    p=plain(HERE/'run-fixed-work.py')
    require(hexsha(pin) and sha(p)==pin,'fixed authority helper SHA before import')
    spec=importlib.util.spec_from_file_location('authenticated_dense_fixed',p)
    m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m


def constants(source):
    text=plain(source).read_text()
    return {k: {'sha256':hashlib.sha256(v.encode()).hexdigest(),'value':decode(v)}
            for k,v in re.findall(r'const (CONTRACT|HOST_CONTRACT|INTERACTION_CONTRACT): &str = r#"(.*?)"#;',text,re.S)}


def source_guard(fixed,authority,pin,family,case):
    a=fixed.authority_check(authority,pin)
    fixed.require_target(a,fixed.CORE_TARGET)
    require(Path(a['stage_root'])==STAGE,'isolated performance stage only')
    require(a['build_settings'].get('profile')=='release' and a['build_settings'].get('debug_assertions') in (False,'false'),'release/no debug assertions required')
    require(a['timeout_seconds']==120 and len(a['cpu_ids'])==1,'one physical CPU/120s fixed contract')
    require(a.get('dense_cases')==ALL_CASES and a.get('core_test_entrypoints')==ENTRIES,'approved complete case/entrypoint worklist')
    require(family in ENTRIES and case in (KERNEL_CASES if family=='kernel' else INTERACTION_CASES),'declared case only')
    for p in [Path(__file__).absolute(),HERE/'measurement_host.py',HERE/'run-fixed-work.py']:
        plain(p);require(a['host_scripts'].get(str(p))==sha(p),'host script authority mismatch')
    k=constants(STAGE/'packages/game-engine/crates/core/src/engine/dense_kernel_tests.rs')
    i=constants(STAGE/'packages/game-engine/crates/core/src/engine/dense_interaction_tests.rs')
    refs=a['dense_correctness_references']
    require(set(refs)=={'kernel','interaction'},'both independently pinned correctness references required')
    read={}
    for name,row in refs.items():
        p=plain(Path(row['path']));require(hexsha(row['sha256']) and sha(p)==row['sha256'],'correctness reference SHA')
        read[name]=fixed.host.read_json(p)
    require(read['kernel']['status']=='focused_correctness_format_clippy_passed_no_performance_claim' and read['kernel']['contract_sha256']==k['CONTRACT']['sha256'],'kernel native correctness authority')
    require(read['interaction']['status']=='passed_correctness_clippy_fmt_not_measured' and read['interaction']['contract_embedded_sha256']==i['INTERACTION_CONTRACT']['sha256'],'interaction native correctness authority')
    source_rel='packages/game-engine/crates/core/src/engine/dense_interaction_tests.rs'
    require(read['interaction']['source'][source_rel]['sha256']==sha(STAGE/source_rel),'interaction reference must bind exact current frozen module')
    require(a.get('dense_contract_sha256')=={'kernel':k['HOST_CONTRACT']['sha256'],'kernel_fixture':k['CONTRACT']['sha256'],'interaction':i['INTERACTION_CONTRACT']['sha256']},'explicit approved embedded contract SHAs')
    effect=fixed.host.read_json(STAGE/'packages/game-engine/tests/fixtures/expected-effects-ir.json')
    instruction=next(d for d in effect['definitions'] if d['oracle_id']=='5748ebf1-24e3-499d-ab7c-c2cebd462a24')['spell']['instructions'][0]
    return a,{'kernel':k['HOST_CONTRACT'],'kernel_fixture':k['CONTRACT'],'interaction':i['INTERACTION_CONTRACT']},read,instruction


def parse_stdout(path,family,case,partial=False):
    plain(path);require(path.stat().st_size<=MAX_STDOUT,'stdout bound')
    raw=path.read_bytes();require(b'\0' not in raw,'NUL stdout')
    # A torn last record is excluded, never promoted to a completed repetition.
    torn=bool(raw and not raw.endswith(b'\n'))
    lines=raw.decode('utf-8').splitlines();excluded=None
    if torn:
        require(partial,'unterminated stdout record');excluded=lines.pop()
    entry=ENTRIES[family];events=[];summary=False;running=False
    marker_re=re.compile(r'^(?:test '+re.escape(entry)+r' \.\.\. )?('+'|'.join(MARKERS)+r')=(.*)$')
    for line in lines:
        if not line:continue
        m=marker_re.fullmatch(line)
        if m:
            name,text=m.groups();value=decode(text);require(isinstance(value,dict),'marker object required')
            require((family=='kernel' and name.startswith(('DENSE_DIAGNOSTIC','DENSE_RETAINED'))) or (family=='interaction' and name.startswith('DENSE_INTERACTION')),'cross-family marker')
            events.append((name,value));continue
        if line=='running 1 test':require(not running,'duplicate running header');running=True;continue
        if line in ('ok','test '+entry+' ... ok','test '+entry+' ... '):continue
        if re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(?:\.[0-9]+)?s',line):require(not summary,'duplicate success trailer');summary=True;continue
        if partial and (line=='FAILED' or line.startswith(('failures:','    '+entry,'test result: FAILED.'))):continue
        raise ValueError('unexpected native stdout line')
    if not partial:require(running and summary,'exact one successful test required')
    return {'events':events,'successful_test_trailer':summary,'excluded_torn_final_line':torn,'torn_final_line_bytes':len(excluded.encode()) if excluded is not None else 0,'stdout_sha256':sha(path)}


def mem(value):
    require(set(value)=={'rss_bytes','high_water_bytes'} and all(natural(v) and v>0 for v in value.values()),'memory counter shape')
    require(value['high_water_bytes']>=value['rss_bytes'],'memory counter conservation')


def segment(s,name,rep,logical,index=None):
    require(set(s)=={'name','repetition','warmup','nanos','logical_operations','operation_index'},'segment complete shape')
    require(natural(s['repetition']) and natural(s['logical_operations']) and (s['operation_index'] is None or natural(s['operation_index'])),'segment integer types')
    require(s['name']==name and s['repetition']==rep and s['warmup'] is (rep==0),'segment order/repetition')
    require(s['logical_operations']==logical and s['operation_index']==index,'actual logical operation/index mismatch')
    require(natural(s['nanos']) and 0<s['nanos']<=120_000_000_000,'bounded positive segment ns')


def mana_counts(c,objects,observations):
    expected={'initial_queries':observations+1,'query_resumptions':observations*((objects+3)//64),
              'submits':1,'advance_calls':1,'committed_rule_steps':2}
    require(canonical(c)==canonical(expected),'actual mana/query/advance work conservation')


def kernel_row(row,case,contracts,refs,instruction):
    c=contracts['kernel']['value'];specs={s['name']:s for s in c['kernel_cases']}
    if case in specs:
        spec=specs[case];n=spec['battlefield_objects'];require(row['case']==spec,'exact kernel fixture')
        p=row['oracle'];require(p['fixture']==spec and p['contract_sha256']==contracts['kernel_fixture']['sha256'],'kernel proof identity')
        require(p['public_admission_rejected'] is True and p['snapshot_and_replay_verified'] is True and p['timings_collected'] is False,'independent untimed native proofs')
        require(p['physical_cards']==n+400 and p['initial_other_zone_cards']==p['root_other_zone_cards']==400,'complete physical baseline')
        require(len(p['history_lengths'])==4 and all(natural(x) and x>=spec['minimum_history_entries'] for x in p['history_lengths']) and p['visible_rows_by_viewer']==([n+9,n+9,n+9,n+10] if case in ('dense_history','dense_combined') else [n+7]*4),'actual history/visible density')
        require(all(hexsha(p[x]) for x in ['root_state_sha256','first_write_state_sha256','synthetic_profile_digest']) and p['first_write_added_inputs']==1 and natural(p['setup_accepted_inputs']) and p['setup_accepted_inputs']>0,'full semantic first write/proof denominator')
        prior=[x for x in refs['kernel']['proofs'] if x['fixture']['name']==case]
        if case in ('ordinary','moderate','dense','large_synthetic'):
            require(len(prior)==1,'exactly one preserved independent kernel proof required')
            require(canonical(p)==canonical(prior[0]),'complete preserved independent kernel proof differs')
        else:require(not prior,'extended history oracle scope must not masquerade as historical proof')
        require(row['physical_cards']==n+400 and row['other_zone_cards']==400 and row['commanders']==0 and row['lives']==['6','2','2','2'],'synthetic scope exact')
        require(row['timed_oracle_work'] is False and row['all_segment_inputs_and_results_verified'] is True,'oracles excluded and results verified')
        require(len(row['samples'])==54 and len(row['work_counts_by_repetition'])==6,'one warmup/five complete samples')
        for rep in range(6):
            for j,name in enumerate(SEGMENTS):segment(row['samples'][9*rep+j],name,rep,4 if j==0 else n+400 if j==1 else 1)
            mana_counts(row['work_counts_by_repetition'][rep],n+400,4)
        mem(row['memory_including_setup_and_oracles'])
        return {'scope':'synthetic kernel; public admission rejected','case':case,'measured_repetitions':5,'physical_cards':n+400,'complete_semantic_proof':p,'proof_scope':'complete preserved reference equality' if prior else 'current compiled untimed oracle; no historical reference row exists','actual_work_counts':row['work_counts_by_repetition'],'raw_segments':row['samples']}
    if case=='effects':
        require(row['case']=='effects' and [r['effect_instances'] for r in row['rows']]==[0,8,64,256],'all evaluator counts required')
        for r in row['rows']:
            count=r['effect_instances'];require(r['actual_fixture_capacity']==16 and r['released_match_capacity']==32 and r['targets']==12,'actual evaluator fixture/capacity/target density')
            require(r['domain_evaluator_only'] is True and r['supported_invocation_creation_claim'] is False and r['production_restore_rejected'] is (count!=0) and r['synthetic_profile_admission_rejected'] is True,'no above-capacity admission or creation claim')
            require(r['source_instruction']==instruction and len(r['samples'])==6,'complete source-backed modifier/five samples')
            for rep,s in enumerate(r['samples']):segment(s,'pure_characteristics_modifier_sweep',rep,12)
            mem(r['memory'])
        return {'scope':'0/8/64/256 pure modifier evaluator ONLY, not pending triggers or admitted creation','rows':row['rows'],'measured_repetitions_each':5}
    require(case=='retained_branches','known kernel case')
    require(row['case']==case and row['fixture']==specs['ordinary'],'retention source is synthetic ordinary8 only')
    require((row['declared_total_sweeps'],row['warmup_sweeps'],row['measured_sweeps'])==(24,4,20),'all retention denominators')
    require(all(row[k] is True for k in ['individual_fork_samples_are_not_independent_sweep_repetitions','no_tail_precision_claim_from_five_lifecycle_samples','internal_rss_guard_is_not_outer_process_cap_proof','rss_difference_is_not_exact_live_heap_allocation','resource_shortfall_is_not_proof_target_can_never_fit']),'retention scope qualifiers')
    require(len(row['rows'])==24,'all fixed retention attempts required')
    baseline=next(p['root_state_sha256'] for p in refs['kernel']['proofs'] if p['fixture']['name']=='ordinary')
    summaries=[retained_row(r,rep,size,baseline) for r,(rep,size) in zip(row['rows'],[(r,n) for r in range(6) for n in [64,256,1024,10000]])]
    return {'scope':'retained synthetic ordinary8 only; legal ordinary and dense256 retention remain future work','attempts':summaries,'all_requested_sizes_reached':all(s['full_size_sample'] for s in summaries),'resource_shortfall_attempts':sum(not s['full_size_sample'] for s in summaries),'no_tail_precision_claim':True}


def retained_row(r,rep,size,baseline):
    require(natural(r['repetition']) and r['repetition']==rep and r['warmup'] is (rep==0) and r['requested']==size,'retention fixed schedule')
    n=r['retained'];stop=r['resource_shortfall'];require(natural(n) and n<=size and stop in (None,'rss_reserve_before_next_fork','rss_hard_limit_observed_after_fork'),'retention actual count/stop')
    require(r['requested_count_reached'] is (n==size) and r['accepted_full_size_sample'] is (n==size and stop is None),'shortfall cannot become full-size pass')
    require(stop is not None or n==size,'missing explicit resource stop')
    require(r['baseline_state_sha256']==baseline,'same initial semantic State every sweep')
    for k in ['memory_before_retention','memory_at_retention','memory_after_disposal']:mem(r[k])
    if r['accepted_full_size_sample']:require(r['memory_at_retention']['rss_bytes']<MEMORY,'full-size sample exceeded RSS budget')
    samples=r['samples'];selected=sorted({0,n//2,max(0,n-1)}&set(range(n))) if stop!='rss_hard_limit_observed_after_fork' else []
    require(len(samples)==n+len(selected)+1,'fork/write/disposal actual sample denominator')
    for index in range(n):segment(samples[index],'retained_fork',rep,1,index)
    for offset,index in enumerate(selected):segment(samples[n+offset],'retained_selected_first_write',rep,1,index)
    segment(samples[-1],'dispose_retained_branches',rep,n)
    require([r['index'] for r in r['selected_first_write_counts']]==selected,'selected branch count/index proof')
    for item in r['selected_first_write_counts']:mana_counts(item['counts'],408,1)
    require(natural(r['sweep_lifecycle_nanos_including_guards_and_field_assertions']) and r['sweep_lifecycle_nanos_including_guards_and_field_assertions']>=sum(s['nanos'] for s in samples),'inclusive lifecycle clock')
    return {'repetition':rep,'warmup':rep==0,'requested':size,'retained':n,'full_size_sample':r['accepted_full_size_sample'],'resource_stop':stop,'lifecycle_ns':r['sweep_lifecycle_nanos_including_guards_and_field_assertions'],'individual_fork_samples':n,'selected_first_writes':len(selected)}


def interaction_row(r,case,build,contract,reference):
    require(r['schema']=='synthetic-dense-interaction-result-v2' and r['build_id']==build,'interaction schema/build')
    require(r['contract_sha256']==contract['sha256'] and canonical(r['contract'])==canonical(contract['value']),'interaction complete contract identity')
    require(r['acceptance_latency_claim'] is False and r['outer_guard_only_declared_here'] is True,'native declaration is not OS proof')
    row=copy.deepcopy(r['result']);samples=row['work']['samples'];row['work']['samples']=[]
    require(canonical(row)==canonical(reference),'complete independent interaction counts/State/semantics differ')
    w=row['work'];expected={'advance':w['advance_calls'],'validate_and_accept_input':w['accepted_inputs'],'observe_initial':w['initial_queries'],'observe_resume':w['query_resumes'],'atomic_event_group':w['explicit_batches_attempted']}
    if case=='damage':expected['prepare_simultaneous_damage']=2
    require(Counter(s.get('operation') for s in samples)==Counter({k:v for k,v in expected.items() if v}),'actual operation sample counts')
    require(all(set(s)=={'operation','nanos'} and natural(s['nanos']) and 0<s['nanos']<=120_000_000_000 for s in samples),'bounded exact operation timing shape')
    return {'semantics':row,'raw_operations':samples,'timed_operation_count':len(samples)}


def progress(events,family,case,complete):
    if family=='kernel':
        starts=[v for k,v in events if k=='DENSE_RETAINED_SWEEP_START'];ends=[v for k,v in events if k=='DENSE_RETAINED_SWEEP_RESULT'];finals=[v for k,v in events if k=='DENSE_DIAGNOSTIC']
        require(all(k in ('DENSE_RETAINED_SWEEP_START','DENSE_RETAINED_SWEEP_RESULT','DENSE_DIAGNOSTIC') for k,_ in events),'kernel marker namespace')
        if case!='retained_branches':require(not starts and not ends,'unexpected retention markers')
        else:
            schedule=[(rep,n) for rep in range(6) for n in [64,256,1024,10000]];cursor=0;pending=False
            for k,v in events:
                if k=='DENSE_DIAGNOSTIC':require(not pending and cursor==24,'final before retention progress complete');continue
                require(cursor<24,'extra retention progress')
                rep,n=schedule[cursor];require(natural(v['repetition']) and v['repetition']==rep and v['warmup'] is (rep==0) and v['requested']==n,'retention progress schedule')
                if k.endswith('START'):
                    require(set(v)=={'repetition','warmup','requested','baseline_state_sha256'} and hexsha(v['baseline_state_sha256']),'retention start identity')
                    require(not pending,'duplicate/missing retained completion');pending=True
                else:require(pending,'missing retained start');pending=False;cursor+=1
            if complete:require(cursor==24 and not pending,'missing retention progress/repetition')
        require(len(finals)<=1,'duplicate kernel final')
        if complete:require(len(finals)==1,'missing kernel final')
        return {'started':len(starts),'completed':len(ends),'expected':24 if case=='retained_branches' else 1,'finals':finals,'retained_rows':ends,'retained_starts':starts}
    starts=[];ends=[];pending=False;memories=[];phase_index=0
    for k,v in events:
        if k=='DENSE_INTERACTION_START':
            phase_index=0
            require(not pending and v['case']==case and natural(v['repetition']) and v['repetition']==len(ends) and v['warmup'] is (len(ends)==0),'interaction progress start');starts.append(v);pending=True
        elif k=='DENSE_INTERACTION_RESULT':
            require(pending and natural(v['repetition']) and v['repetition']==len(ends) and v['warmup'] is (len(ends)==0),'interaction progress completion')
            require(case!='triggers' or phase_index==4,'all trigger phase-memory records required')
            ends.append(v);pending=False
        elif k=='DENSE_INTERACTION_MEMORY':
            require(case=='triggers' and pending,'memory namespace/order')
            phases=['after_all64_placements_before_oracles','after_all64_resolutions_before_oracle','after_all64_resolution_oracle','after_exact_replay_and_disk_byte_comparison']
            require(phase_index<4 and v['phase']==phases[phase_index],'trigger phase order')
            require(v['history_lengths']==[131 if phase_index==0 else 835]*4,'actual trigger history work')
            require(set(v)=={'phase','history_lengths','kernel_counters','note'},'native memory record shape')
            require(len(v['kernel_counters'])==2,'two phase counters')
            counters={}
            for text in v['kernel_counters']:
                m=re.fullmatch(r'(VmRSS|VmHWM):\s+([0-9]+) kB',text);require(m is not None,'kernel memory lexical shape');require(m[1] not in counters,'duplicate memory field');counters[m[1]]=int(m[2])*1024
            require(set(counters)=={'VmRSS','VmHWM'} and 0<counters['VmRSS']<=counters['VmHWM'],'memory counter facts')
            phase_index+=1;memories.append(v)
        else:raise ValueError('interaction marker namespace')
    require(len(starts)<=6 and len(ends)<=6,'extra interaction repetitions')
    if complete:require(len(starts)==len(ends)==6 and not pending,'one warmup plus five completed repetitions')
    return {'started':len(starts),'completed':len(ends),'expected':6,'results':ends,'memory_snapshots':memories}


def validate_output(parsed,family,case,build,contracts,refs,instruction):
    p=progress(parsed['events'],family,case,True)
    if family=='kernel':
        r=p['finals'][0];require(r['schema']=='synthetic-dense-diagnostic-result-v1' and r['build_id']==build,'kernel result schema/build')
        require(r['contract_sha256']==contracts['kernel']['sha256'] and canonical(r['contract'])==canonical(contracts['kernel']['value']),'kernel complete contract identity')
        require(r['outer_guard_declared_not_verified_here'] is True and r['acceptance_latency_claim'] is False and r['legal_match_admission_claim'] is False,'native declarations not OS/admission proof')
        value=kernel_row(r['result'],case,contracts,refs,instruction)
        if case=='retained_branches':
            require(p['retained_rows']==r['result']['rows'],'progress/final full row equality')
            require(all(a['baseline_state_sha256']==b['baseline_state_sha256'] for a,b in zip(p['retained_starts'],p['retained_rows'])),'start/completion State SHA differs')
        short=case=='retained_branches' and not value['all_requested_sizes_reached']
        return {'classification':'verified_resource_shortfall' if short else 'complete_declared_row_verified','regular_full_size_pass':not short,'validated':value}
    reference=next(r for r in refs['interaction']['actual_proofs'] if r['row']==case)
    values=[interaction_row(r,case,build,contracts['interaction'],reference) for r in p['results']]
    return {'classification':'complete_declared_row_verified','regular_full_size_pass':True,'repetitions':values,'memory_snapshots_scope':'native phaseRSS/cumulativeHWM only; independent OS report is authoritative'}


def main():
    p=argparse.ArgumentParser();p.add_argument('--fixed-driver-sha256',required=True);p.add_argument('--helper-sha256',required=True);p.add_argument('--authority',type=Path,required=True);p.add_argument('--authority-sha256',required=True);p.add_argument('--family',choices=ENTRIES,required=True);p.add_argument('--case',required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--execute',action='store_true');args=p.parse_args()
    require(args.execute,'prepared only until root explicitly authorizes execution');os.umask(0o077)
    fixed=load_fixed(args.fixed_driver_sha256);h=fixed.load_host(args.helper_sha256)
    a,contracts,refs,instruction=source_guard(fixed,args.authority,args.authority_sha256,args.family,args.case)
    h.real_path(args.output.parent,private=True);require(not args.output.exists() and not args.output.is_symlink(),'exclusive fresh output');args.output.mkdir(mode=0o700)
    execution=args.output/'execution';stdout=execution/'workload.stdout'
    env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','DENSE_KERNEL_OUTER_GUARD':'120s-2gib-v1',('DENSE_KERNEL_CASE' if args.family=='kernel' else 'DENSE_INTERACTION_CASE'):args.case}
    def audit():
        again=source_guard(fixed,args.authority,args.authority_sha256,args.family,args.case);require(again==(a,contracts,refs,instruction),'source/evidence endpoints changed')
        return validate_output(parse_stdout(stdout,args.family,args.case),args.family,args.case,a['engine_build_id'],contracts,refs,instruction)
    try:row=h.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',ENTRIES[args.family]],cwd=STAGE,env=env,directory=execution,label='dense-'+args.family+'-'+args.case.replace('_','-'),cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
    except BaseException as e:row={'status':'failed','failure':{'type':type(e).__name__,'reason':str(e)[:2048]},'resource_report_created':False}
    partial=None
    if stdout.exists():
        try:parsed=parse_stdout(stdout,args.family,args.case,partial=True);partial=progress(parsed['events'],args.family,args.case,False);partial.update(stdout_sha256=parsed['stdout_sha256'],excluded_torn_final_line=parsed['excluded_torn_final_line'])
        except Exception as e:partial={'status':'malformed_partial_not_success','error':type(e).__name__,'stdout_sha256':sha(stdout)}
    passed=row['status']=='passed';failure=row.get('failure')
    try:require(source_guard(fixed,args.authority,args.authority_sha256,args.family,args.case)==(a,contracts,refs,instruction),'final authority changed')
    except Exception as e:passed=False;failure={'type':type(e).__name__,'reason':str(e)[:2048]}
    audited=row.get('audit');full=passed and audited and audited['regular_full_size_pass']
    result={'schema':'dense-host-result-v1','status':'complete_declared_row_verified' if full else 'verified_resource_shortfall' if passed else 'failed_or_truncated','family':args.family,'case':args.case,'authority_sha256':args.authority_sha256,'source_contract_sha256':contracts[args.family]['sha256'],'host_report_sha256':sha(execution/'host-report.json') if (execution/'host-report.json').exists() else None,'failure':failure,'audit':audited,'partial_progress':partial,'declared_row_complete':bool(passed),'all_requested_sizes_full_pass':bool(full),'product_latency_or_legal_admission_claim':False,'retained_scope_warning':'retained_case is synthetic ordinary8; legal ordinary ANDdense256 retained costs and evolved legal history remain separate future work'}
    h.save_json(args.output/'result.json',result);print(json.dumps({'status':result['status']}));return 0 if full else 1


if __name__=='__main__':sys.exit(main())
