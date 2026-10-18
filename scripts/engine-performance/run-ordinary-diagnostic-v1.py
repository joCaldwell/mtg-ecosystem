#!/usr/bin/env python3
"""Thin ordinary legal-prefix/artifact host. No implicit build or execution."""
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import statistics
import sys

HERE=Path(__file__).absolute().parent
STAGE=HERE.parents[1]
ENTRIES={'legal':'match_games::ordinary_diagnostic::ordinary_legal_pod_diagnostic',
         'artifacts':'match_games::ordinary_diagnostic::ordinary_artifact_batch_diagnostic'}


def require(ok,reason):
    if not ok:
        raise ValueError(reason)


def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):
            h.update(block)
    return h.hexdigest()


def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def hexsha(value):
    return type(value) is str and re.fullmatch('[0-9a-f]{64}',value) is not None


def natural(value):
    return type(value) is int and value>=0


def load_fixed(expected):
    path=HERE/'run-fixed-work.py'
    require(not path.is_symlink() and sha(path)==expected,'source-authority driver SHA before import')
    spec=importlib.util.spec_from_file_location('authenticated_fixed_driver',path)
    fixed=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixed)
    return fixed


def hist(value):
    require(isinstance(value,dict) and all(type(k) is str and type(v) is int and v>0 for k,v in value.items()),'positive integer histogram required')
    return value


def journal(value):
    require(set(value)=={'decisions','concessions'} and isinstance(value['decisions'],list) and isinstance(value['concessions'],list),'full raw journal shape')
    previous=-1
    for row in value['decisions']:
        require(set(row)=={'decision_id','player','response'} and natural(row['decision_id']) and row['decision_id']>previous,'ordered unique actual decision IDs')
        previous=row['decision_id']
        require(type(row['player']) is int and 0<=row['player']<4 and isinstance(row['response'].get('response'),str),'actual input seat/response')
    for row in value['concessions']:
        require(set(row)=={'after_decisions','boundary','player'} and natural(row['boundary']) and natural(row['after_decisions']) and row['after_decisions']<=len(value['decisions']) and type(row['player']) is int and 0<=row['player']<4,'concession shape')
    return [d['response']['response'] for d in value['decisions']]


def scalar_times(value,keys):
    require(set(value)==set(keys) and all(natural(v) for v in value.values()),'complete nonnegative nanosecond timing fields')


def percentile(values,p):
    require(values and all(natural(x) for x in values),'nonempty complete time denominator')
    return sorted(values)[max(0,math.ceil(len(values)*p)-1)]


def catalog_load(load,fixture):
    encoded=canonical(fixture['value'])
    require(load['input_bytes']==fixture['bytes'] and load['input_sha256']==fixture['sha256'],'actual mapped IR input identity/bytes')
    require(load['canonical_bytes']==len(encoded) and load['catalog_sha256']==hashlib.sha256(encoded).hexdigest(),'canonical decoded catalog identity/bytes')
    require(natural(load['decode_ns']) and natural(load['source_compile_verify_ns']),'catalog timing fields')
    return {'decoder_scope':'already-mapped embedded IR strict decode only; excludes filesystem/startup and separately timed source-compiler validation','bytes':load['input_bytes'],'decode_ns':load['decode_ns'],'source_compile_verify_ns':load['source_compile_verify_ns']}


def legal_semantics(s):
    keys={'inputs','rule_steps','advances','query_calls','response_kinds','event_kinds','actual_outcome_kinds','commander_casts','noncreature_casts','paid_command_casts','paid_noncreature_casts','survivor_counts','outcome','final_state_sha256','journal_sha256','trace_sha256','journal'}
    require(set(s)==keys,'complete ordinary semantics fields')
    order=journal(s['journal'])
    require(s['journal']['concessions']==[],'ordinary source never concedes')
    require(s['inputs']==len(order) and type(s['inputs']) is int and 0<s['inputs']<=1024,'actual nonzero input denominator')
    require(s['response_kinds']==dict(Counter(order)),'raw-journal response histogram')
    require(s['journal_sha256']==digest(s['journal']),'raw-journal canonical SHA')
    require(all(hexsha(s[k]) for k in ['final_state_sha256','trace_sha256']),'complete State/trace SHA')
    require(all(natural(s[k]) and s[k]>0 for k in ['rule_steps','advances','query_calls']),'ordinary operation counts')
    require(s['advances']>=s['inputs']+1 and s['query_calls']>=s['inputs']*2,'ordinary call conservation')
    events=hist(s['event_kinds']); outcomes=hist(s['actual_outcome_kinds'])
    require(sum(outcomes.values())==events.get('committed',0),'every native committed event has one actual outcome')
    require(outcomes.get('life_gained',0)+outcomes.get('life_lost',0)>0,'actual typed simple life effects required')
    cast_count=s['response_kinds'].get('cast',0)
    require(all(type(s[k]) is int and 0<s[k]<=cast_count for k in ['commander_casts','noncreature_casts']),'positive cast-proposal fixture coverage')
    require(s['commander_casts']+s['noncreature_casts']<=cast_count,'disjoint creature-commander/noncreature proposal counts')
    require(type(s['paid_command_casts']) is int and 0<s['paid_command_casts']<=s['commander_casts'] and type(s['paid_noncreature_casts']) is int and 0<s['paid_noncreature_casts']<=s['noncreature_casts'],'actual positive paid-cast witnesses bounded by proposals')
    survivors=s['survivor_counts']
    require(survivors and survivors[0]==4 and all(type(n) is int and 0<=n<=4 for n in survivors) and all(a>b for a,b in zip(survivors,survivors[1:])),'survivor sequence')
    require(s['outcome'] in ('fixed_prefix_horizon','finished','draw'),'honest terminal or prefix classification')
    if s['outcome']=='fixed_prefix_horizon':
        require(s['inputs']==1024,'no shortened prefix')
    elif s['outcome']=='finished':
        require(survivors[-1]==1,'terminal survivor count')
    return order


def legal_times(t,order):
    scalar=['admission_ns','setup_advance_ns','replay_verify_ns','dispose_ns']
    require(set(t)==set(scalar+['observation_ns','policy_ns','check_submit_advance_ns','response_order']),'complete timing vector shape')
    require(all(natural(t[k]) for k in scalar),'nonnegative separate operation clocks')
    require(t['response_order']==order,'clock labels exactly match accepted input order')
    for key in ['observation_ns','policy_ns','check_submit_advance_ns']:
        require(len(t[key])==len(order) and all(natural(v) for v in t[key]),'no omitted response timings')
    require(sum(t['check_submit_advance_ns'])>0,'nonzero timed core work')


def validate_legal(r,build,fixture):
    require(r['schema']=='ordinary-legal-pod-diagnostic-v1' and r['status']=='passed' and r['error'] is None,'complete native legal report')
    require(r['build_id']==build and r['seed_index']==1 and r['starting_player']==1 and r['mulligans']==[0]*4 and r['trace_mode']=='off' and r['quantum']==1024,'predeclared ordinary fixture')
    require(r['warmups']==1 and r['measured_repeats']==5 and len(r['samples'])==5,'one warmup plus five measured prefixes')
    load=catalog_load(r['catalog_load'],fixture)
    s=r['reference_semantics']; order=legal_semantics(s)
    legal_times(r['warmup_timings'],order)
    durations=[]; core=[]; obs=[]; policy=[]
    for i,row in enumerate(r['samples']):
        require(row['repetition']==i and row['semantics']==s,'full repeated semantics including every raw accepted input')
        legal_times(row['timings'],order)
        durations.append(sum(row['timings']['check_submit_advance_ns']))
        core.extend(row['timings']['check_submit_advance_ns']); obs.extend(row['timings']['observation_ns']); policy.extend(row['timings']['policy_ns'])
    def timing(values):
        return {'operations':len(values),'p50_ns':percentile(values,.5),'p95_ns':percentile(values,.95),'p99_ns':percentile(values,.99)}
    return {'status':'ordinary_legal_prefix_semantics_verified','classification':s['outcome'],
            'actual_inputs_per_repetition':s['inputs'],'measured_repetitions':5,'measured_inputs':5*s['inputs'],
            'per_repetition_core_ns':durations,'per_repetition_inputs_per_core_second':[s['inputs']*1e9/n for n in durations],
            'transition':timing(core),'observation':timing(obs),'policy':timing(policy),'catalog_load':load,
            'full_state_sha256':s['final_state_sha256'],'journal_sha256':s['journal_sha256'],
            'cast_counts_scope':'commander_casts/noncreature_casts are validated proposals; paid_command_casts/paid_noncreature_casts join actual SpellProposed to SpellCast in native Detailedtrace; payment is not a resolution claim',
            'paid_command_casts_per_repetition':s['paid_command_casts'],'paid_noncreature_casts_per_repetition':s['paid_noncreature_casts'],
            'native_oracle_scope':'Detailed independent replay derives event/outcome histograms and trace SHA; host checks scalar consistency, not unavailable raw event trace',
            'timing_scope':'check+submit+advance segments only; excludes separate observation/policy/admission/replay/serialization/I/O; host whole-tree metrics separate',
            'completed_game_claim':s['outcome'] in ('finished','draw'),'search_useful_trials_claimed':0}


def snapshot(value,state):
    require(set(value)=={'snapshot_bytes','snapshot_sha256','state_sha256','snapshot_ns','restore_ns','dispose_ns'},'snapshot metric shape')
    require(type(value['snapshot_bytes']) is int and value['snapshot_bytes']>0 and hexsha(value['snapshot_sha256']) and value['state_sha256']==state,'snapshot exact identity/count')
    require(all(natural(value[k]) for k in ['snapshot_ns','restore_ns','dispose_ns']),'snapshot timing fields')
    return {k:value[k] for k in ['snapshot_bytes','snapshot_sha256','state_sha256']}


def validate_artifacts(r,build,fixture,cases):
    require(r['schema']=='ordinary-artifact-batch-v1' and r['status']=='passed' and r['error'] is None and r['build_id']==build,'complete actual artifact report')
    require(r['trace_mode']=='off' and r['warmups']==1 and r['measured_repeats']==5 and len(r['roots'])==12 and len(cases)==12,'all twelve predeclared roots')
    load=catalog_load(r['catalog_load'],fixture)
    summaries=[]
    for i,root in enumerate(r['roots']):
        require(root['index']==i and root['case']==cases[i] and cases[i]['index']==i,'unchanged root order/case')
        require(hexsha(root['state_sha256']) and natural(root['prepare_ns']),'actual root preparation/State')
        journal(root['journal'])
        departures=cases[i]['departures_before_setup']
        require([c['player'] for c in root['journal']['concessions']]==departures and all(c['after_decisions']==0 for c in root['journal']['concessions']),'declared pre-setup departures only')
        require(root['live_players']==[p for p in range(4) if p not in departures],'real four/three/two live-seat fixture')
        ref=snapshot(root['snapshot_warmup'],root['state_sha256'])
        require(len(root['snapshots'])==5,'five snapshot samples')
        for value in root['snapshots']:
            require(snapshot(value,root['state_sha256'])==ref,'all complete snapshot byte identities retained')
        require([b['semantics']['size'] for b in root['batches']]==[1,64,1024],'exact native batch sizes')
        unit=root['batches'][0]['semantics']
        batch_summary=[]
        for batch in root['batches']:
            s=batch['semantics']; size=s['size']
            require(set(s)=={'size','query_calls','bytes','serialized_sha256'} and natural(s['query_calls']) and s['query_calls']>0 and type(s['bytes']) is int and s['bytes']>2 and hexsha(s['serialized_sha256']),'complete batch semantic fields')
            require(s['query_calls']==unit['query_calls']*size and s['bytes']==size*(unit['bytes']-1)+1,'exact identical-observation array bytes and query-call denominator')
            scalar_times(batch['warmup'],['query_ns','copy_ns','serialize_ns','dispose_ns'])
            require(len(batch['samples'])==5,'five batch samples')
            totals={key:[] for key in ['query_ns','copy_ns','serialize_ns','dispose_ns']}
            for rep,sample in enumerate(batch['samples']):
                require(sample['repetition']==rep and sample['semantics']==s,'full batch repetition equality')
                scalar_times(sample['timings'],totals)
                for key in totals:
                    totals[key].append(sample['timings'][key])
            require(all(n>0 for n in totals['query_ns']) and all(n>0 for n in totals['serialize_ns']),'actual work timing denominator')
            batch_summary.append({'size':size,'measured_observations':5*size,'measured_query_calls':5*s['query_calls'],'measured_bytes':5*s['bytes'],'observations_per_query_second':[size*1e9/n if n else None for n in totals['query_ns']],
                                  'encoded_bytes_per_serialization_second':[s['bytes']*1e9/n if n else None for n in totals['serialize_ns']],'raw_component_ns':totals,'serialized_sha256':s['serialized_sha256']})
        summaries.append({'root_index':i,'live_seats':len(root['live_players']),'journal_sha256':digest(root['journal']),
                          'snapshot':ref,'snapshot_ns':[v['snapshot_ns'] for v in root['snapshots']],'restore_ns':[v['restore_ns'] for v in root['snapshots']], 'batches':batch_summary})
    return {'status':'all12_native_artifact_batch_semantics_verified','catalog_load':load,'roots':summaries,
            'snapshot_samples':60,'batch_samples':180,'measured_observation_copies':12*5*(1+64+1024),
            'independent_games_claimed':0,'network_or_ffi_claim':False,'native_oracle_scope':'Native source checks complete snapshot/journal/State restore equality and actual observations/owned copies/encoded bytes. Host checks available digest/count identities and exact same-root batch formulas.'}


def source_guard(fixed,authority_path,pin,mode):
    a=fixed.authority_check(authority_path,pin)
    fixed.require_target(a,fixed.SIM_TARGET)
    require(Path(a['stage_root'])==STAGE,'performance stage only')
    require(a['build_settings'].get('profile')=='release' and (a['build_settings'].get('debug_assertions') is False or a['build_settings'].get('debug_assertions')=='false'),'approved release diagnostic executable')
    require(a['timeout_seconds']==120 and len(a['cpu_ids'])==1,'fixed120/single physical CPU')
    require(a['ordinary_mode']==mode,'approved mode binding')
    paths=[Path(__file__).absolute(),HERE/'measurement_host.py',HERE/'run-fixed-work.py']
    for path in paths:
        require(a['host_scripts'].get(str(path))==sha(path),'authenticated host script authority')
    fixture_path=STAGE/'packages/game-engine/tests/fixtures/expected-match-ir.json'
    fixture={'value':fixed.host.read_json(fixture_path),'sha256':sha(fixture_path),'bytes':fixture_path.stat().st_size}
    if mode=='artifacts':
        require(len(a['root_cases'])==12 and [c['index'] for c in a['root_cases']]==list(range(12)),'complete approved artifact roots')
    return a,fixture


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--fixed-driver-sha256',required=True)
    p.add_argument('--helper-sha256',required=True)
    p.add_argument('--authority',type=Path,required=True)
    p.add_argument('--authority-sha256',required=True)
    p.add_argument('--mode',choices=ENTRIES,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--execute',action='store_true')
    args=p.parse_args()
    require(args.execute,'prepared only until root-approved explicit execution')
    os.umask(0o077)
    fixed=load_fixed(args.fixed_driver_sha256)
    h=fixed.load_host(args.helper_sha256)
    a,fixture=source_guard(fixed,args.authority,args.authority_sha256,args.mode)
    h.real_path(args.output.parent,private=True)
    require(not args.output.exists(),'fresh output required')
    args.output.mkdir(mode=0o700)
    (args.output/'tmp').mkdir(mode=0o700)
    env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','TMPDIR':str(args.output/'tmp'),'MTG_ORDINARY_OUTPUT':str(args.output/'execution/native-report.json')}
    def audit():
        a2,fixture2=source_guard(fixed,args.authority,args.authority_sha256,args.mode)
        require(a2==a and fixture2==fixture,'source/authority endpoint changed')
        r=h.read_json(args.output/'execution/native-report.json')
        value=validate_legal(r,a['engine_build_id'],fixture) if args.mode=='legal' else validate_artifacts(r,a['engine_build_id'],fixture,a['root_cases'])
        return {**value,'raw_report_sha256':sha(args.output/'execution/native-report.json')}
    try:
        row=h.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',ENTRIES[args.mode]],cwd=STAGE,env=env,directory=args.output/'execution',label='ordinary-'+args.mode,cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
    except BaseException as e:
        row={'status':'failed','failure':{'type':type(e).__name__,'reason':str(e)[:2048]},'resource_report_created':False}
    passed=row['status']=='passed'
    result={'status':'diagnostic_semantics_verified' if passed else 'diagnostic_failed_or_truncated','mode':args.mode,'authority_sha256':args.authority_sha256,'resource_report_sha256':sha(args.output/'execution/host-report.json') if (args.output/'execution/host-report.json').exists() else None,'failure':row.get('failure'),'product_latency_claim':False,'result':row.get('audit')}
    try:
        source_guard(fixed,args.authority,args.authority_sha256,args.mode)
    except Exception as e:
        passed=False
        result.update(status='diagnostic_failed_or_truncated',failure={'type':type(e).__name__,'reason':str(e)[:2048]})
    h.save_json(args.output/'result.json',result)
    print(json.dumps({'status':result['status']}))
    return 0 if passed else 1


if __name__=='__main__':
    sys.exit(main())
