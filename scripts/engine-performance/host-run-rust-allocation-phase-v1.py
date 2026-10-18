#!/usr/bin/env python3
"""SOURCE-ONLY thin host for one exact process-global Rust allocation phase."""
import argparse, hashlib, importlib.util, json, os, re, sys, tarfile, tomllib
from pathlib import Path, PurePosixPath

ID='stats-alloc-system-phase-v1'
ENTRY='match_games::ordinary_diagnostic::ordinary_core_allocation_diagnostic'
COUNTERS={'allocations','deallocations','reallocations','bytes_allocated','bytes_deallocated','bytes_reallocated'}
REGISTRY='registry+https://github.com/rust-lang/crates.io-index'

def require(value,reason):
    if not value:raise ValueError(reason)
def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
def plain(path):
    p=Path(path);require(p.is_absolute() and '..' not in p.parts,'absolute plain path')
    for q in (p,*p.parents):require(not q.is_symlink(),'symlink ancestry')
    require(p.is_file(),'plain file');return p
def strict(path):
    def pairs(rows):
        d={}
        for k,v in rows:require(k not in d,'duplicate JSON key');d[k]=v
        return d
    p=plain(path);require(p.stat().st_size<=64*1024**2,'bounded JSON')
    return json.loads(p.read_text(),object_pairs_hook=pairs,parse_constant=lambda _:(_ for _ in ()).throw(ValueError('nonfinite JSON')))
def pin(row):
    p=plain(row['path']);require(sha(p)==row['sha256'],'changed pinned file');return p
def module(name,path,pin_value):
    p=plain(path);require(sha(p)==pin_value,'source hash BEFORE import')
    spec=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(spec);sys.modules[name]=m;spec.loader.exec_module(m);return m
def features(artifact,expected):
    require(artifact.get('reason')=='compiler-artifact' and artifact.get('target',{}).get('name')=='mtg-sim' and artifact['target'].get('kind')==['bin'],'SIM artifact required')
    require(artifact.get('features')==expected,'exact actual Cargo features required; engine Build alone is insufficient')
def safe_relative(name):
    p=PurePosixPath(name);require(bool(p.parts) and name not in ('','.') and not p.is_absolute() and p.as_posix()==name and '\\' not in name and all(x not in ('.','..') for x in p.parts),'safe package relative path');return p

def package_guard(row,lockfile,dependency_artifact):
    m=strict(pin(row))
    require(m['schema']=='test-instrumentation-package-source-v1' and (m['name'],m['version'],m['registry'])==('stats_alloc','0.1.10',REGISTRY) and m['default_features'] is False,'exact package identity/default features')
    lock=tomllib.loads(plain(lockfile).read_text())
    packages=[p for p in lock['package'] if p['name']=='stats_alloc']
    require(len(packages)==1 and packages[0]['version']=='0.1.10' and packages[0]['source']==REGISTRY and packages[0]['checksum']==m['crate_sha256'],'resolved lock binds registry package checksum')
    require(sha(plain(m['crate_path']))==m['crate_sha256'],'actual registry crate checksum')
    rows=m['source_files'];names=[r['path'] for r in rows]
    require(names==sorted(set(names)) and 'src/lib.rs' in names and 'Cargo.toml' in names,'complete ordered source inventory')
    root=Path(m['source_root']);require(root.is_absolute() and root.is_dir() and not root.is_symlink(),'retained source root')
    for r in rows:
        p=plain(root/safe_relative(r['path']));require(p.stat().st_size==r['bytes'] and sha(p)==r['sha256'],'retained package source row')
    actual={str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}
    require(actual==set(names) and not any(p.is_symlink() for p in root.rglob('*')),'exact retained package tree')
    # Read-only archive comparison: no extraction or source/dependency mutation.
    with tarfile.open(m['crate_path'],'r:gz') as archive:
        members=[x for x in archive.getmembers() if not x.isdir()]
        expected={f'stats_alloc-0.1.10/{name}':name for name in names}
        require(len(members)==len(expected) and {x.name for x in members}==set(expected) and all(x.isfile() for x in members),'exact plain registry archive members')
        for x in members:
            raw=archive.extractfile(x).read();r=rows[names.index(expected[x.name])]
            require(len(raw)==r['bytes'] and hashlib.sha256(raw).hexdigest()==r['sha256'],'archive equals retained package bytes')
    cargo=tomllib.loads((root/'Cargo.toml').read_text());require(cargo['package']['name']=='stats_alloc' and cargo['package']['version']=='0.1.10' and cargo['features']['default']==[],'primary package metadata')
    dep=strict(pin(dependency_artifact));require(dep.get('reason')=='compiler-artifact' and dep.get('target',{}).get('name')=='stats_alloc' and dep['target'].get('kind')==['lib'] and dep.get('features')==[],'actual stats dependency artifact/nightly disabled')
    used=plain(dep['manifest_path']).parent
    require(dep['package_id']==REGISTRY+'#stats_alloc@0.1.10','actual package artifact registry identity')
    for r in rows:
        p=plain(used/r['path']);require(p.stat().st_size==r['bytes'] and sha(p)==r['sha256'],'actual compiled dependency source equals retained archive')
    used_files={str(p.relative_to(used)) for p in used.rglob('*') if p.is_file()}
    require(used_files-set(names)<={'.cargo-ok','.cargo-checksum.json'} and not any(p.is_symlink() for p in used.rglob('*')),'no unreviewed dependency source additions')
    return {'package_manifest_sha256':row['sha256'],'crate_sha256':m['crate_sha256'],'source_files':len(rows),'dependency_artifact_sha256':dependency_artifact['sha256']}

def baseline_receipt(host,baseline):
    require(host.get('status')=='passed' and host.get('executable_sha256')==baseline['test_executable']['sha256'],'actual successful feature-off executable receipt')
    require(host.get('audit',{}).get('raw_report_sha256')==baseline['report']['sha256'],'feature-off host binds complete raw report')
    command=host.get('command',[])
    require(command and command[-1]=='match_games::ordinary_diagnostic::ordinary_legal_pod_diagnostic' and '--exact' in command and '--test-threads=1' in command,'actual exact full-prefix baseline invocation')
    require(host.get('cleanup')=={'errors':[],'launcher_reaped':True,'unit_empty':True} and host.get('workload',{}).get('exit_code')==0,'feature-off baseline completed and cleaned')

def known_vector_receipt(host,stdout,exe_sha):
    require(host.get('status')=='passed' and host.get('executable_sha256')==exe_sha and host.get('workload',{}).get('exit_code')==0,'successful same-counter-executable known vector')
    entry='diagnostic_allocations::diagnostic_allocation_known_vector_requests'
    command=host.get('command',[])
    tails=[['--exact','--ignored','--test-threads=1',entry],['--exact','--ignored','--nocapture','--test-threads=1',entry]]
    require(type(command) is list and len(command)>1 and command[1:] in tails,'exact fresh ignored known-vector invocation without skip/extra filters')
    require(host.get('cleanup')=={'errors':[],'launcher_reaped':True,'unit_empty':True},'known-vector cleanup')
    require(host.get('audit',{}).get('raw_stdout_sha256')==hashlib.sha256(stdout.encode('utf-8')).hexdigest(),'known-vector in-deadline receipt binds actual stdout bytes')
    require(stdout.endswith('\n') and '\0' not in stdout,'complete known-vector stdout bytes')
    lines=[line for line in stdout.splitlines() if line]
    require(len(lines)==3 and lines[0]=='running 1 test' and lines[1]=='test '+entry+' ... ok' and re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(?:\.[0-9]+)?s',lines[2]),'one actual complete known-vector test, no skipped/duplicate/trailing test results')

def guard(path,pin_value):
    require(sha(plain(path))==pin_value,'approved authority SHA');a=strict(path)
    fixed_row=a['counter_host']['fixed_driver'];fixed=module('counter_fixed',fixed_row['path'],fixed_row['sha256'])
    helper_pin=a['host_scripts'][str(Path(fixed_row['path']).with_name('measurement_host.py'))];h=fixed.load_host(helper_pin)
    require(fixed.authority_check(path,pin_value)==a,'shared native/exe/Cargo/source authority');fixed.require_target(a,fixed.SIM_TARGET)
    features(strict(pin(a['compiler_artifact'])),['diagnostic-allocations'])
    require(a['cpu_ids'] and len(a['cpu_ids'])==1 and a['timeout_seconds']==120,'one physical CPU/fixed120s')
    require(a['counter_host']['id']==ID and a['counter_host']['schema']=='ordinary-core-allocation-v1','fixed counter instrumentation/phase')
    own=Path(__file__).absolute();require(a['host_scripts'].get(str(own))==sha(plain(own)),'counter host script pin')
    counter_module=a['counter_host']['allocator_module'];pin(counter_module)
    require(Path(counter_module['path'])==Path(a['stage_root'])/'packages/game-engine/crates/sim/src/diagnostic_allocations.rs','actual allocator source path')
    text=plain(counter_module['path']).read_text();require(f'pub(crate) const ID: &str = "{ID}";' in text and 'pub(crate) const CRATE_VERSION: &str = "0.1.10";' in text,'compiled counter constants')
    ordinary=module('counter_ordinary',a['counter_host']['ordinary_driver']['path'],a['counter_host']['ordinary_driver']['sha256'])
    fixture=Path(a['stage_root'])/'packages/game-engine/tests/fixtures/expected-match-ir.json';fixture={'value':h.read_json(fixture),'sha256':sha(fixture),'bytes':fixture.stat().st_size}
    known=a['counter_host']['known_vector'];known_text=plain(pin(known['stdout']));require(known_text.stat().st_size<=64*1024**2,'bounded known-vector stdout');known_vector_receipt(strict(pin(known['host_report'])),known_text.read_bytes().decode('utf-8'),a['test_executable']['sha256'])
    dependency=package_guard(a['counter_host']['package_source'],Path(a['stage_root'])/'Cargo.lock',a['counter_host']['dependency_artifact'])
    baseline=a['counter_host']['feature_off_baseline'];report=strict(pin(baseline['report']));artifact=strict(pin(baseline['compiler_artifact']));pin(baseline['test_executable'])
    baseline_receipt(strict(pin(baseline['host_report'])),baseline)
    features(artifact,[]);fixed.validate_settings(a['build_settings'],artifact,baseline['test_executable']['sha256'],fixed.SIM_TARGET)
    ordinary.validate_legal(report,a['engine_build_id'],fixture)
    require(baseline['test_executable']['sha256']!=a['test_executable']['sha256'],'baseline and counter executables independently identified')
    return a,h,ordinary,fixture,report['reference_semantics'],dependency

def integer(v,signed=False):
    return type(v) is int and (-(1<<63)<=v<(1<<63) if signed else 0<=v<(1<<64))
def counter_vector(row):
    require(type(row) is dict and set(row)==COUNTERS and all(integer(v,k=='bytes_reallocated') for k,v in row.items()),'complete exact integer raw counters')
    return row

def validate_report(r,a,ordinary,baseline):
    require(r.get('schema')=='ordinary-core-allocation-v1' and r.get('status')=='passed' and r.get('error') is None and r['build_id']==a['engine_build_id'],'complete exact counter report')
    i=r['instrumentation'];require(i['id']==ID and i['crate']=='stats_alloc' and i['version']=='0.1.10' and i['required_SIM_features']==['diagnostic-allocations'] and i['allocator']=='StatsAlloc<System>' and i['cfg']=='test AND feature diagnostic-allocations','native instrumentation declaration')
    require(r['separate_instrumentation_authority_required'] is True and r['latency_acceptance_or_production_allocator_claim'] is False,'honest counter scope')
    ordinary.legal_semantics(r['reference_semantics']);require(r['reference_semantics']==baseline,'ALL same-source feature-off semantics/journal/State/events/paid casts unchanged')
    require(type(r['accepted_inputs']) is int and r['accepted_inputs']==1024 and type(r['advance_calls']) is int and r['advance_calls']>=1024 and type(r['instrumented_core_ns']) is int and 0<r['instrumented_core_ns']<=120_000_000_000,'complete unchanged phase/clock')
    before=counter_vector(r['raw_before']);after=counter_vector(r['raw_after']);delta=counter_vector(r['checked_difference'])
    for k in COUNTERS:require(after[k]-before[k]==delta[k] and integer(delta[k],k=='bytes_reallocated'),'independently checked raw difference')
    require(delta['allocations']+delta['reallocations']>0,'actual intercepted Rust requests')
    require(len(r['empty_controls'])==2 and all(all(v==0 for v in counter_vector(x).values()) for x in r['empty_controls']),'two exact zero empty controls; never subtract')
    g=r['thread_guard'];require(set(g)=={'process','current_thread','observed_threads'} and type(g['process']) is int and type(g['current_thread']) is int and g['process']>0 and g['current_thread']>0 and g['process']!=g['current_thread'] and all(type(x) is int for x in g['observed_threads']) and g['observed_threads']==sorted([g['process'],g['current_thread']]),'exact two stable declared Linux threads')
    denominator=r['request_rate_denominator']
    require(type(denominator) is dict and set(denominator)=={'accepted_inputs','no_rule_boundary_claim','no_control_subtraction'} and type(denominator['accepted_inputs']) is int and denominator['accepted_inputs']==1024 and denominator['no_rule_boundary_claim'] is True and denominator['no_control_subtraction'] is True,'exact typed request denominator/no noise subtraction')
    return {'status':'rust_global_request_phase_verified','requests':delta,'accepted_inputs':1024,'requests_per_accepted_input':{k:{'numerator':v,'denominator':1024} for k,v in delta.items()},'counter_scope':'process-global allocator requests including any libtest-main interference during phase; controls are diagnostics, not TLS proof','clock_scope':'raw_core_prefix plus atomic counter snapshot/bookend overhead; instrumented timing not baseline latency','full_feature_off_reference_equal':True,'complete_allocations_live_bytes_or_latency_acceptance_claim':False}

def main():
    p=argparse.ArgumentParser();p.add_argument('--authority',type=Path,required=True);p.add_argument('--authority-sha256',required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--execute',action='store_true');args=p.parse_args();require(args.execute,'explicit parent authorized execution only');os.umask(0o077)
    a,h,ordinary,fixture,baseline,dependency=guard(args.authority,args.authority_sha256)
    h.real_path(args.output.parent,private=True);require(not args.output.exists(),'fresh output');args.output.mkdir(mode=0o700);(args.output/'tmp').mkdir(mode=0o700)
    report=args.output/'execution/native-report.json';env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','TMPDIR':str(args.output/'tmp'),'MTG_DIAGNOSTIC_ALLOCATIONS':ID,'MTG_ORDINARY_OUTPUT':str(report)}
    def audit():
        require(guard(args.authority,args.authority_sha256)[0]==a,'source/feature/dependency/baseline endpoint')
        native=h.read_json(report);value=validate_report(native,a,ordinary,baseline);ordinary.catalog_load(native['catalog_load'],fixture)
        require(guard(args.authority,args.authority_sha256)[0]==a,'postsemantic endpoint')
        return {**value,'raw_report_sha256':sha(report),'dependency':dependency}
    try:
        row=h.run_bounded(command=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',ENTRY],cwd=Path(a['stage_root']),env=env,directory=args.output/'execution',label='rust-alloc-phase',cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
    except BaseException as e:row={'status':'failed','failure':{'type':type(e).__name__,'reason':str(e)[:2048]},'resource_report_created':False}
    passed=row['status']=='passed';failure=row.get('failure')
    try:require(guard(args.authority,args.authority_sha256)[0]==a,'final source/dependency endpoint')
    except Exception as e:passed=False;failure={'type':type(e).__name__,'reason':str(e)[:2048]}
    h.save_json(args.output/'result.json',{'schema':'rust-allocation-phase-host-result-v1','status':'verified' if passed else 'failed_or_truncated','authority_sha256':args.authority_sha256,'engine_build_id':a['engine_build_id'],'failure':failure,'audit':row.get('audit'),'resource_report_sha256':sha(args.output/'execution/host-report.json') if (args.output/'execution/host-report.json').exists() else None,'no_production_allocator_or_latency_claim':True})
    return 0 if passed else 1
if __name__=='__main__':sys.exit(main())
