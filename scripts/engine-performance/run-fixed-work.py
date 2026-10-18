#!/usr/bin/env python3
"""Prepared-only by default. Fixed-work scaling DIAGNOSTIC, not product gates."""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import sys
import time
import importlib.util

host = None

def load_host(expected):
    global host
    path=Path(__file__).absolute().with_name("measurement_host.py")
    require(not path.is_symlink() and hexsha(expected) and hashlib.sha256(path.read_bytes()).hexdigest()==expected, "helper SHA mismatch before import")
    spec=importlib.util.spec_from_file_location("authenticated_measurement_host",path)
    loaded=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    host=loaded
    return loaded

FILTER = 'search_jobs::fixed_work::fixed_work_diagnostic_row'
SCHEMA = 'fixed-work-host-plan-v1'
SIM_TARGET={'name':'mtg-sim','kind':['bin']}
CORE_TARGET={'name':'mtg_core','kind':['lib']}


def canonical(value):
    # Canonical fixture/proposal data are ASCII keys, exact integers and strings;
    # no floats/escaped supplementary Unicode occur in native frozen work.
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False).encode()


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def hexsha(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def natural(value):
    return type(value) is int and value >= 0


def pinned(path, expected):
    host.real_path(path)
    require(hexsha(expected) and host.sha(path) == expected, 'pinned file changed: '+str(path))


def inventory(root):
    paths = []
    for relative in ['packages/game-engine/crates', 'packages/game-server/src', 'packages/game-server/tests', 'packages/game-engine/tests/fixtures']:
        for base, dirs, files in os.walk(root/relative, followlinks=False):
            require(not Path(base).is_symlink(), 'native symlink directory')
            for d in dirs:
                require(not (Path(base)/d).is_symlink(), 'native symlink directory')
            dirs[:] = [d for d in dirs if d not in ('generated','target','node_modules','.git')]
            for name in files:
                p = Path(base)/name
                if p.suffix in ('.rs','.json') or name == 'Cargo.toml':
                    require(not p.is_symlink(), 'native symlink file')
                    paths.append(str(p.relative_to(root)))
    paths.extend(['Cargo.toml','Cargo.lock','rust-toolchain.toml','packages/game-server/Cargo.toml'])
    return sorted(paths)


def require_target(authority, expected):
    require(authority["test_target"]==expected,"caller test-target authority mismatch")


def validate_settings(settings, artifact, executable_sha256, test_target):
    keys={'rustc','profile','target','rustflags','opt_level','debug','debug_assertions','overflow_checks'}
    require(set(settings)==keys,'complete typed effective build settings required')
    require(all(type(settings[k]) is str for k in keys-{'overflow_checks'}),'build environment settings must be strings')
    require(settings['rustc'].startswith('rustc ') and '\n' not in settings['rustc'] and '\r' not in settings['rustc'] and '\0' not in settings['rustc'],'canonical rustc version line')
    require(settings['profile']=='release' and settings['opt_level']=='3' and settings['debug']=='false' and settings['debug_assertions']=='false' and settings['overflow_checks'] is True,'exact reviewed release/opt/check settings')
    require(re.fullmatch('[A-Za-z0-9_.-]+',settings['target']) and '\0' not in settings['rustflags'],'canonical target/encoded rustflags')
    require(test_target in (SIM_TARGET,CORE_TARGET),'finite allowed Cargo test target')
    require(artifact.get('reason')=='compiler-artifact' and artifact.get('target',{}).get('name')==test_target['name'] and artifact['target'].get('kind')==test_target['kind'],'actual Cargo test-target compiler artifact')
    profile=artifact.get('profile',{})
    require(profile.get('test') is True and profile.get('opt_level')=='3' and type(profile.get('debuginfo')) is int and profile['debuginfo']==0 and profile.get('debug_assertions') is False and profile.get('overflow_checks') is True,'Cargo artifact proves test/optimization/assertion/overflow profile')
    pinned(Path(artifact['executable']),executable_sha256)


def compiled_build_id(root,files,settings):
    # EXACT crates/core/build.rs algorithm. Overflow checks are intentionally NOT
    # in that historical hash; the actual Cargo artifact proves them separately.
    h=hashlib.sha256()
    for row in files:
        h.update(row['path'].encode());h.update(b'\0')
        with (root/row['path']).open('rb') as f:
            for block in iter(lambda:f.read(1024*1024),b''):
                h.update(block)
        h.update(b'\0')
    h.update((settings['rustc']+'\n').encode())
    for key in ['profile','target','rustflags','opt_level','debug','debug_assertions']:
        h.update(b'\0');h.update(settings[key].encode())
    return h.hexdigest()


def authority_check(path, expected):
    """Caller supplies an independently reviewed manifest SHA, not auto-trust."""
    pinned(path, expected)
    a = host.read_json(path)
    require(a.get('schema') == 'performance-host-authority-v1', 'authority schema')
    root = host.real_path(Path(a['stage_root']))
    exe = Path(a['test_executable']['path'])
    pinned(exe, a['test_executable']['sha256'])
    require(hexsha(a['engine_build_id']) and isinstance(a['build_settings'], dict), 'missing compiled identity/settings')
    artifact_row=a['compiler_artifact']
    pinned(Path(artifact_row['path']),artifact_row['sha256'])
    artifact=host.read_json(Path(artifact_row['path']))
    validate_settings(a['build_settings'],artifact,a['test_executable']['sha256'],a['test_target'])
    m = a['native_manifest']
    pinned(Path(m['path']), m['sha256'])
    native = host.read_json(Path(m['path']))
    require(native.get('status') == 'compiled-source-closure-verified' and native.get('build_id') == a['engine_build_id'], 'compiled manifest binding')
    files = native['files']
    require([f['path'] for f in files] == inventory(root), 'native inventory differs')
    for f in files:
        rel = Path(f['path'])
        require(not rel.is_absolute() and '..' not in rel.parts, 'unsafe native path')
        pinned(root/rel, f['sha256'])
        require((root/rel).stat().st_size == f['bytes'], 'native bytes differ')
    require(compiled_build_id(root,files,a['build_settings'])==a['engine_build_id'],'current source/toolchain/settings Build ID mismatch')
    require(a['contracts'], 'contracts required')
    for row in a['contracts']:
        pinned(Path(row['path']), row['sha256'])
    return a


def semantic_trials(work, trials):
    worlds = work['worlds']
    candidates = work['candidate_policy']['candidates']
    expected = [(w['world_index'], i) for w in worlds for i in range(len(candidates))]
    require([(t['world_index'], t['candidate_index']) for t in trials] == expected, 'missing/duplicate/reordered candidate-world pair')
    for t in trials:
        w = worlds[t['world_index']]
        require(t['assignment_digest'] == w['assignment_digest'] and t['candidate_id'] == candidates[t['candidate_index']]['candidate_id'], 'trial identity mismatch')
        counts = t['counts']
        require(set(counts) == {'accepted_inputs','advance_calls','query_calls','rule_steps'} and all(natural(v) for v in counts.values()), 'invalid trial counts')
        require(counts['accepted_inputs'] <= 256 and counts['advance_calls']+counts['query_calls'] < 100000, 'trial bound')
        result = t['result']
        require(result.get('outcome') in ('terminal','horizon') and hexsha(result.get('state_sha256')), 'incomplete/nonuseful trial')
        if result['outcome'] == 'horizon':
            require(counts['accepted_inputs'] == 256, 'shortened horizon')
        values = result['evaluations']
        require(sorted(v['player'] for v in values) == [0,1,2,3], 'incomplete utility vector')
        for v in values:
            require(isinstance(v['utility'], str) and str(int(v['utility'])) == v['utility'], 'noncanonical quantity')


def aggregates(work, trials):
    rows = []
    for i, c in enumerate(work['candidate_policy']['candidates']):
        selected = [t for t in trials if t['candidate_index'] == i]
        row = {'candidate_index': i, 'candidate_id': c['candidate_id'],
               'useful_distinct_worlds': len({t['assignment_digest'] for t in selected}), 'minimum_met': True,
               'terminal_games': 0, 'horizon_trials': 0, 'winner_counts': [0]*4, 'utility_sums': ['0']*4}
        sums = [0]*4
        for t in selected:
            r = t['result']
            if r['outcome'] == 'terminal':
                row['terminal_games'] += 1
                if r['winner'] is not None:
                    require(type(r['winner']) is int and 0 <= r['winner'] < 4, 'invalid winner')
                    row['winner_counts'][r['winner']] += 1
            else:
                row['horizon_trials'] += 1
            for v in r['evaluations']:
                sums[v['player']] += int(v['utility'])
        row['utility_sums'] = list(map(str, sums))
        row['minimum_met'] = row['useful_distinct_worlds'] >= 8
        rows.append(row)
    return rows


def validate_schedule(result, work, cpus, reference):
    require(result['workers'] == len(cpus) and result['complete_useful'] is True and result['worker_faults'] == [], 'incomplete schedule')
    require(natural(result['elapsed_micros']) and result['elapsed_micros'] > 0, 'missing elapsed')
    measurements = result['worker_measurements']
    require([m['worker_index'] for m in measurements] == list(range(len(cpus))) and [m['cpu_id'] for m in measurements] == cpus, 'worker affinity coverage')
    require(sorted(w for m in measurements for w in m['completed_world_indices']) == list(range(len(work['worlds']))), 'incomplete worker visits')
    worlds = result['worlds']
    require([w['world_index'] for w in worlds] == list(range(len(work['worlds']))), 'incomplete worlds')
    raw = []
    for world in worlds:
        require(world['assignment_digest'] == work['worlds'][world['world_index']]['assignment_digest'] and natural(world['build_calls']) and world['build_calls'] > 0, 'world build identity')
        raw.extend({k: t[k] for k in ['world_index','assignment_digest','candidate_index','candidate_id','counts','result']} for t in world['trials'])
    semantic_trials(work, raw)
    actual = {'work_sha256': digest(work), 'world_build_calls': [[w['world_index'],w['build_calls']] for w in worlds], 'trials': raw, 'aggregates': aggregates(work, raw)}
    require(result['semantics'] == actual == reference, 'raw/semantic/reference mismatch')
    return result['elapsed_micros']


def validate_row(report, *, root, cpus, available, build, world_count=8):
    require(report.get('schema') == 'foundations-fixed-search-row-v1' and report.get('status') == 'fixed_row_verified', 'row not complete')
    require(report['engine_build_id'] == build and report['root_index'] == root and report['horizon'] == 256, 'row authority/horizon mismatch')
    require(report['workers'] == len(cpus) and report['worker_cpu_ids'] == cpus and report['available_physical_cpu_ids'] == available, 'CPU inventory drift')
    work = report['frozen_work']
    require(work['public_job_seed'] == list(hashlib.sha256(f'commander-open-search-v1/job/{root}'.encode()).digest()), 'public producer seed drift')
    require(work['version'] == 'foundations-fixed-search-work-v1' and work['horizon'] == 256, 'work version/horizon')
    worlds = work['worlds']
    require(world_count in (8,32) and len(worlds) == world_count and [w['world_index'] for w in worlds] == list(range(len(worlds))) and len({w['assignment_digest'] for w in worlds}) == len(worlds), 'work diversity/order')
    c = work['candidate_policy']
    require(c['truncated_at_limit'] is False and 0 < len(c['candidates']) <= 32 and len({v['candidate_id'] for v in c['candidates']}) == len(c['candidates']), 'candidate worklist')
    require(report['frozen_work_sha256'] == digest(work), 'work digest')
    reference = report['reference_semantics']
    semantic_trials(work, reference['trials'])
    require(reference['work_sha256'] == digest(work) and reference['aggregates'] == aggregates(work,reference['trials']) and report['reference_semantics_sha256'] == digest(reference) and report['reference_complete_useful'] is True, 'reference proof')
    require(len(report['samples']) == 5, 'five samples required')
    all_samples = [report['warmup'], *report['samples']]
    times = []
    for i, sample in enumerate(all_samples):
        require(sample['repetition'] == i and sample['warmup'] is (i == 0) and sample['semantic_equality'] is True, 'warmup/sample labels')
        elapsed = validate_schedule(sample['result'],work,cpus,reference)
        if i:
            times.append(elapsed)
    return {'work': work, 'semantics': reference, 'median_pool_micros': statistics.median(times),
            'timed_pool_micros': times, 'useful_trials_per_sample': len(reference['trials'])}


def comparison(rows, expected_workers):
    require([r['workers'] for r in rows] == expected_workers, 'all worker-count rows required before ratios')
    require(all(r['host_status'] == 'passed' for r in rows), 'failed row forbids speedups')
    base = rows[0]['validated']
    for row in rows:
        require(row['validated']['work'] == base['work'] and row['validated']['semantics'] == base['semantics'], 'cross-row COMPLETE semantics differ')
    return [{'workers': r['workers'], 'serial_median_pool_micros': base['median_pool_micros'],
             'median_pool_micros': r['validated']['median_pool_micros'],
             'observed_speedup': base['median_pool_micros']/r['validated']['median_pool_micros'],
             'useful_trials_per_second': r['validated']['useful_trials_per_sample']*1000000/r['validated']['median_pool_micros']}
            for r in rows]


def plan_guard(plan):
    require(plan['schema'] == SCHEMA and plan['status'] == 'prepared_only', 'plan schema/status')
    a = authority_check(Path(plan['authority_path']),plan['authority_sha256'])
    require_target(a,SIM_TARGET)
    for p, h in plan['host_sources'].items():
        pinned(Path(p),h)
    require(plan['available_physical_cpu_ids'] == host.physical_cpus(), 'physical topology/allowed affinity changed')
    expected = list(dict.fromkeys([1,2,4,len(host.physical_cpus())]))
    require(plan['worker_counts'] == expected and len(host.physical_cpus()) >= 4 and len(host.physical_cpus()) <= 64, 'physical worker count contract')
    require(plan['timeout_seconds'] == 120 and plan['memory_bytes'] == host.MEMORY_BYTES and 0 <= plan['root'] < 12 and plan['worlds'] in (8,32), 'limits/root/world contract')
    return a


def execute(plan_path, expected_sha):
    pinned(plan_path, expected_sha)
    plan = host.read_json(plan_path)
    a = plan_guard(plan)
    output = Path(plan['output'])
    host.real_path(output.parent, private=True)
    require(not output.exists(), 'run output must be new')
    output.mkdir(mode=0o700)
    status = {'schema': 'fixed-work-host-result-v1','status':'running','classification':'diagnostic only; not product latency/scaling acceptance', 'plan_sha256':expected_sha,'rows':[]}
    host.save_json(output/'status.json',status)
    try:
        for count in plan['worker_counts']:
            cpus = plan['available_physical_cpu_ids'][:count]
            row_dir = output/f'workers-{count}'
            raw_path = row_dir/'native-row.json'
            env = {k:v for k,v in os.environ.items() if not k.startswith('MTG_')}
            env.update(MTG_FIXED_SEARCH_OUTPUT=str(raw_path), MTG_FIXED_SEARCH_ROOT=str(plan['root']), MTG_FIXED_SEARCH_WORLDS=str(plan['worlds']), MTG_FIXED_SEARCH_CPUS=json.dumps(cpus))
            validated = {}
            def audit():
                pinned(plan_path,expected_sha)
                plan_guard(plan)
                validated.update(validate_row(host.read_json(raw_path),root=plan['root'],cpus=cpus,available=plan['available_physical_cpu_ids'],build=a['engine_build_id'],world_count=plan['worlds']))
                return {'raw_report_sha256':host.sha(raw_path),'full_semantics_sha256':digest(validated['semantics']), 'complete_useful_trials':len(validated['semantics']['trials'])}
            result = host.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',FILTER], cwd=Path(a['stage_root']),env=env,directory=row_dir,label=f'fixed-root-{plan["root"]}-w{count}', cpus=plan['available_physical_cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
            status['rows'].append({'workers':count,'host_status':result['status'],'host_report_sha256':host.sha(row_dir/'host-report.json'),'validated':validated})
            host.save_json(output/'status.json',status)
            require(result['status'] == 'passed','row failed; attempt retained, no speedup claim')
        pinned(plan_path,expected_sha)
        plan_guard(plan)
        status['comparisons'] = comparison(status['rows'],plan['worker_counts'])
        status['status'] = 'complete_semantics_verified_diagnostic'
    except BaseException as e:
        status.update(status='failed',failure={'type':type(e).__name__,'reason':str(e)[:2048]})
    host.save_json(output/'status.json',status)
    return status


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--helper-sha256',required=True)
    p.add_argument('--authority',type=Path)
    p.add_argument('--authority-sha256')
    p.add_argument('--prepare-plan',type=Path)
    p.add_argument('--output',type=Path)
    p.add_argument('--root',type=int,default=0)
    p.add_argument('--worlds',type=int,choices=[8,32],default=32)
    p.add_argument('--execute-plan',type=Path)
    p.add_argument('--plan-sha256')
    args = p.parse_args()
    os.umask(0o077)
    load_host(args.helper_sha256)
    if args.execute_plan:
        result = execute(args.execute_plan,args.plan_sha256)
        print(json.dumps({'status':result['status'],'rows':len(result['rows'])}))
        return 0 if result['status'] == 'complete_semantics_verified_diagnostic' else 1
    require(args.authority is not None and args.authority_sha256 and args.prepare_plan is not None and args.output is not None,'explicit prepare inputs required; no automatic execution')
    authority_check(args.authority,args.authority_sha256)
    host.real_path(args.prepare_plan.parent,private=True)
    host.real_path(args.output.parent,private=True)
    require(not args.prepare_plan.exists() and not args.output.exists(),'new plan/output required')
    cpus = host.physical_cpus()
    plan = {'schema':SCHEMA,'status':'prepared_only','authority_path':str(args.authority),'authority_sha256':args.authority_sha256,'output':str(args.output),'root':args.root,'worlds':args.worlds,'available_physical_cpu_ids':cpus,'worker_counts':list(dict.fromkeys([1,2,4,len(cpus)])),'timeout_seconds':120,'memory_bytes':host.MEMORY_BYTES,'host_sources':{str(Path(__file__).absolute()):host.sha(Path(__file__).absolute()),str(Path(host.__file__).absolute()):host.sha(Path(host.__file__).absolute())}}
    plan_guard(plan)
    host.save_json(args.prepare_plan,plan)
    print(json.dumps({'status':'prepared_only','plan_sha256':host.sha(args.prepare_plan)}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
