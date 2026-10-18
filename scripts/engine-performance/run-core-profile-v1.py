#!/usr/bin/env python3
"""Prepared host only: bounded gprofng CPU+heap capture, never builds native code.
GNU primary manual: https://sourceware.org/binutils/docs/gprofng.html
-y USR2 starts paused; -H on enables heap tracing; -F off excludes descendants.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time

HERE=Path(__file__).absolute().parent
ORDINARY_ENTRY='match_games::ordinary_diagnostic::ordinary_core_profile_diagnostic'
ATTRIBUTION='match_games::ordinary_diagnostic::raw_core_prefix'
COLLECTIONS={'cpu':['-F','off','-a','off','-H','off','-p','on'],
             'heap':['-F','off','-a','off','-H','on','-p','off']}
DISPLAYS=['header','functions','heapstat']
MAX_TEXT=64*1024**2
MEMORY=2*1024**3

def require(ok,why):
    if not ok:raise ValueError(why)

def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()

def hexsha(value):return type(value) is str and re.fullmatch('[0-9a-f]{64}',value) is not None

def plain(path):
    p=Path(path);require(p.is_absolute() and '..' not in p.parts,'absolute plain path required')
    for q in (p,*p.parents):require(not q.is_symlink(),'symlink ancestry')
    require(p.is_file(),'plain file required');return p

def pairs(items):
    d={}
    for k,v in items:require(k not in d,'duplicate JSON key');d[k]=v
    return d

def read(path):
    p=plain(path);require(p.stat().st_size<=MAX_TEXT,'bounded JSON')
    return json.loads(p.read_text(),object_pairs_hook=pairs,parse_constant=lambda _:(_ for _ in ()).throw(ValueError('nonfinite JSON')))

def save(path,value):
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'w') as f:json.dump(value,f,sort_keys=True,indent=2,allow_nan=False);f.write('\n');f.flush();os.fsync(f.fileno())

def load(name,a):
    p=plain(HERE/name);require(sha(p)==a['host_scripts'].get(str(p)),'script SHA before import')
    spec=importlib.util.spec_from_file_location('profile_'+name.replace('-','_').replace('.','_'),p)
    module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module);return module

def tool_guard(a):
    tools=a['profile_tools'];require(type(tools) is dict and tools,'explicit tool pins required')
    required={str(Path(sys.executable).resolve()),'/usr/bin/gprofng','/usr/bin/gprofng-collect-app','/usr/bin/gprofng-display-text','/usr/bin/kill','/etc/gprofng.rc'}
    library_root=Path('/usr/lib/gprofng')
    library_paths={str(p) for p in library_root.iterdir()}
    require(library_paths and all(plain(p) for p in library_paths),'plain complete profiler library directory')
    required |= library_paths
    aliases=a['profile_aliases'];require(set(aliases)=={'/usr/lib/libgprofng.so.0'},'explicit gprofng loader alias')
    for name,target in aliases.items():
        p=Path(name);require(p.is_symlink() and str(p.resolve(strict=True))==target,'loader alias changed');required.add(target)
    require(required<=set(tools),'missing actual tools/config/libraries')
    require({p for p in tools if str(Path(p).parent)==str(library_root)}==library_paths,'profiler library inventory changed')
    for p,pin in tools.items():require(hexsha(pin) and sha(plain(p))==pin,'tool/config/library identity changed')
    return tools

def mode_guard(a):
    p=a['profiling'];require(set(p)=={'mode','family','case','collection','collection_flags'},'exact profiling mode shape')
    require(p['collection'] in COLLECTIONS and p['collection_flags']==COLLECTIONS[p['collection']],'exact separate CPU-only or heap-only collection flags required')
    if p['mode']=='ordinary_core':require(p['family'] is None and p['case'] is None,'ordinary core has no substituted dense case')
    else:require(p['mode']=='dense' and p['family'] in ('kernel','interaction') and type(p['case']) is str,'finite profile mode')
    require(a['timeout_seconds']==120 and len(a['cpu_ids'])==1 and type(a['cpu_ids'][0]) is int,'one declared physical CPU and120s')
    return p

def authority_guard(path,pin):
    require(hexsha(pin) and sha(plain(path))==pin,'approved authority SHA')
    a=read(path);p=mode_guard(a)
    for name in ['run-core-profile-v1.py','run-fixed-work.py','measurement_host.py','run-ordinary-diagnostic-v1.py','run-dense-diagnostic-v1.py']:
        require(sha(plain(HERE/name))==a['host_scripts'].get(str(HERE/name)),'all host source pins required')
    fixed=load('run-fixed-work.py',a);h=fixed.load_host(a['host_scripts'][str(HERE/'measurement_host.py')])
    ordinary=load('run-ordinary-diagnostic-v1.py',a);dense=load('run-dense-diagnostic-v1.py',a)
    if p['mode']=='ordinary_core':
        actual=fixed.authority_check(path,pin);fixed.require_target(actual,fixed.SIM_TARGET)
        require(actual==a,'authority decode equality')
        fixture_path=Path(a['stage_root'])/'packages/game-engine/tests/fixtures/expected-match-ir.json'
        context={'fixture':{'value':h.read_json(fixture_path),'sha256':sha(fixture_path),'bytes':fixture_path.stat().st_size}}
    else:
        actual,contracts,refs,instruction=dense.source_guard(fixed,path,pin,p['family'],p['case'])
        context={'contracts':contracts,'refs':refs,'instruction':instruction}
    tool_guard(a)
    return a,fixed,h,ordinary,dense,context

def native_command(a,profile):
    p=mode_guard(a)
    entry=ORDINARY_ENTRY if p['mode']=='ordinary_core' else {'kernel':'engine::dense_kernel_tests::diagnostic::dense_kernel_diagnostic_host_adapter','interaction':'engine::dense_kernel_tests::interactions::dense_interaction_host_diagnostic'}[p['family']]
    cmd=['/usr/bin/gprofng','collect','app',*COLLECTIONS[p['collection']]]
    if p['mode']=='ordinary_core':cmd+=['-y','USR2']
    cmd+=['-o',str(profile/'capture.er'),a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1',entry]
    return cmd,entry

def capture_text(raw,experiment,pid):
    require(type(pid) is int and pid>0,'owned collector PID required')
    prefix=f'Creating experiment directory {experiment} (Process ID: {pid}) ...\n'
    require(raw.startswith(prefix),'exact owned CPU banner/path/PID required')
    rest=raw[len(prefix):]
    require('Creating experiment directory ' not in rest,'duplicate/foreign banner')
    return rest

def validate_profile_native(value,a,ordinary,context):
    require(value.get('schema')=='ordinary-core-profile-v1' and value.get('status')=='passed' and value.get('error') is None,'completed native core profile required')
    require(value['build_id']==a['engine_build_id'] and value['attribution_function']==ATTRIBUTION,'profile Build/attribution root')
    require('signal-bookend' in value['scope'] and 'precloned' in value['scope'],'no false pure-core total allocation scope')
    s=value['reference_semantics'];ordinary.legal_semantics(s);load=ordinary.catalog_load(value['catalog_load'],context['fixture'])
    require(type(value['accepted_inputs']) is int and value['accepted_inputs']==s['inputs']==1024,'full unchanged1024input work')
    require(type(value['advance_calls']) is int and value['advance_calls']>=1024,'automatic advance denominator')
    require(type(value['core_ns']) is int and 0<value['core_ns']<=120_000_000_000,'core phase clock')
    return {'status':'core_profile_native_semantics_verified','inputs':1024,'core_ns':value['core_ns'],'final_state_sha256':s['final_state_sha256'],'journal_sha256':s['journal_sha256'],'catalog_load':load,'allocation_scope':'whole collected phase includes signal bookends; no guessed subtraction. raw_core_prefix function attribution only if collector resolves it; not an all-Rust-allocation count.','reference_preparation_profiled':False}

def display_review(profile,collection):
    require(collection in COLLECTIONS, 'known separate collection')
    rows={};warnings=[]
    for kind in DISPLAYS:
        out=plain(profile/(kind+'.stdout'));err=plain(profile/(kind+'.stderr'))
        require(out.stat().st_size<=MAX_TEXT and err.stat().st_size<=MAX_TEXT,'bounded display text')
        text=out.read_text();stderr=err.read_text();require(text.strip(),'empty display is not successful profiling evidence')
        for line in (text+'\n'+stderr).splitlines():
            if re.search(r'warning|unreliable|truncated|lost samples|error',line,re.I):warnings.append({'display':kind,'line':line[:4096]})
        rows[kind]={'stdout_sha256':sha(out),'stderr_sha256':sha(err),'stdout_bytes':out.stat().st_size}
    if collection=='heap':
        require(not re.search(r'no (?:heap )?data|heap.*not collected', (profile/'heapstat.stdout').read_text(),re.I),'missing heap trace is not allocator evidence')
    return {'collection':collection,'heap_tracing_requested':collection=='heap','cpu_sampling_requested':collection=='cpu','reports':rows,'warnings':warnings,'coverage_status':'raw CPU/heap capture requires independent function/API/sample-coverage review; warnings never suppressed','complete_Rust_allocation_count_claim':False,'causal_ratio_or_acceptance_claim':False}

def group_caps(cpus):
    line=next(s for s in Path('/proc/self/cgroup').read_text().splitlines() if s.startswith('0::'))
    raw=line[3:];require(raw.startswith('/') and '..' not in Path(raw).parts and raw!='/','owned limited group required')
    group=Path('/sys/fs/cgroup')/raw.lstrip('/')
    caps={k:(group/k).read_text().strip() for k in ['memory.max','memory.swap.max','pids.max']}
    require(caps=={'memory.max':str(MEMORY),'memory.swap.max':'0','pids.max':'128'} and sorted(os.sched_getaffinity(0))==cpus,'no unenforced child-mode fallback')
    return {'pid':os.getpid(),'control_group':raw,'caps':caps,'affinity':cpus}

def phase(command,env,cwd,profile,name,deadline):
    remaining=min(120 if name=='collect' else 15,deadline-time.monotonic());require(remaining>0,'one shared deadline exhausted')
    begin=time.monotonic();child=None
    with open(profile/(name+'.stdout'),'xb',opener=lambda p,f:os.open(p,f,0o600)) as out,open(profile/(name+'.stderr'),'xb',opener=lambda p,f:os.open(p,f,0o600)) as err:
        try:
            child=subprocess.Popen(command,env=env,cwd=cwd,stdin=subprocess.DEVNULL,stdout=out,stderr=err,start_new_session=True)
            save(profile/(name+'-pid.json'),{'pid':child.pid,'command':command})
            while True:
                pid,status,usage=os.wait4(child.pid,os.WNOHANG)
                if pid:
                    child.returncode=os.waitstatus_to_exitcode(status);break
                if time.monotonic()-begin>=remaining:raise TimeoutError('finite owned profiler subprocess')
                time.sleep(.01)
            require(child.returncode==0,'profiler subprocess nonzero exit')
            try:os.killpg(child.pid,0)
            except ProcessLookupError:pass
            else:raise RuntimeError('unexpected live descendants after profiler leader exit')
            return {'pid':child.pid,'exit_code':child.returncode,'wall_seconds':time.monotonic()-begin,'user_seconds':usage.ru_utime,'system_seconds':usage.ru_stime,'wait4_maxrss_bytes':usage.ru_maxrss*1024,'scope':'owned tool with waited descendants; process maximum, not tree-summed RSS'}
        finally:
            if child is not None:
                try:os.killpg(child.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                if child.returncode is None:child.wait(timeout=3)

def profile_child(path,pin):
    os.umask(0o077);require(sha(plain(path))==pin,'private child spec hash')
    spec=read(path);require(spec['schema']=='bounded-profile-child-spec-v1','child spec schema');a=spec['authority'];p=mode_guard(a)
    require(sha(plain(__file__))==spec['self_sha256'],'profile child source identity')
    tool_guard(a);require(sha(plain(a['test_executable']['path']))==a['test_executable']['sha256'],'target changed')
    profile=Path(spec['profile']);require(profile.parent.name=='execution' and not profile.exists(),'fresh private profile directory');profile.mkdir(mode=0o700)
    report={'schema':'bounded-profile-supervisor-v1','status':'running','actual_group':group_caps(a['cpu_ids']),'phases':{}}
    result=profile/'supervisor-result.json';deadline=time.monotonic()+120
    env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','HOME':spec['home'],'TMPDIR':spec['tmp']}
    if p['mode']=='ordinary_core':env.update(MTG_ORDINARY_OUTPUT=str(profile/'native-report.json'),MTG_GPROFNG_PAUSED='USR2')
    else:env.update(DENSE_KERNEL_OUTER_GUARD='120s-2gib-v1',**{('DENSE_KERNEL_CASE' if p['family']=='kernel' else 'DENSE_INTERACTION_CASE'):p['case']})
    try:
        command,entry=native_command(a,profile);report['native_entry']=entry
        report['phases']['collect']=phase(command,env,profile,profile,'collect',deadline)
        for kind in DISPLAYS:
            command=['/usr/bin/gprofng','display','text','-limit','0','-'+kind,str(profile/'capture.er')]
            report['phases'][kind]=phase(command,env,profile,profile,kind,deadline)
        tool_guard(a);require(sha(path)==pin and sha(plain(__file__))==spec['self_sha256'],'child source/spec endpoint')
        report['status']='collector_and_displays_completed'
    except BaseException as e:report.update(status='failed',failure={'type':type(e).__name__,'reason':str(e)[:2048]})
    save(result,report);return 0 if report['status']=='collector_and_displays_completed' else 1

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--profile-child',type=Path);parser.add_argument('--spec-sha256');parser.add_argument('--authority',type=Path);parser.add_argument('--authority-sha256');parser.add_argument('--output',type=Path);parser.add_argument('--execute',action='store_true');args=parser.parse_args()
    if args.profile_child:return profile_child(args.profile_child,args.spec_sha256)
    require(args.execute and args.authority is not None and args.output is not None,'prepared only until explicit root authorization')
    os.umask(0o077);a,fixed,h,ordinary,dense,context=authority_guard(args.authority,args.authority_sha256)
    h.real_path(args.output.parent,private=True);require(not args.output.exists(),'fresh output');args.output.mkdir(mode=0o700)
    for n in ['home','tmp']:(args.output/n).mkdir(mode=0o700)
    spec={'schema':'bounded-profile-child-spec-v1','authority':a,'self_sha256':sha(plain(__file__)),'profile':str(args.output/'execution/profile'),'home':str(args.output/'home'),'tmp':str(args.output/'tmp')}
    specpath=args.output/'profile-spec.json';save(specpath,spec);specpin=sha(specpath);profile=Path(spec['profile'])
    def audit():
        again=authority_guard(args.authority,args.authority_sha256);require(again[0]==a,'authority endpoint')
        supervisor=read(profile/'supervisor-result.json');require(supervisor['status']=='collector_and_displays_completed' and list(supervisor['phases'])==sorted(['collect',*DISPLAYS]),'all owned phases completed')
        require(all(v['exit_code']==0 for v in supervisor['phases'].values()),'all tool phases successful')
        pid=supervisor['phases']['collect']['pid'];raw=plain(profile/'collect.stdout');require(raw.stat().st_size<=MAX_TEXT,'bounded native stdout')
        native=capture_text(raw.read_text(),profile/'capture.er',pid)
        stripped=profile/'native.stdout';fd=os.open(stripped,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'w') as f:f.write(native)
        p=mode_guard(a)
        if p['mode']=='ordinary_core':semantic=validate_profile_native(read(profile/'native-report.json'),a,ordinary,context)
        else:semantic=dense.validate_output(dense.parse_stdout(stripped,p['family'],p['case']),p['family'],p['case'],a['engine_build_id'],context['contracts'],context['refs'],context['instruction'])
        evidence=display_review(profile,p['collection']);require(sha(specpath)==specpin,'spec endpoint')
        return {'native_semantics':semantic,'profiler':evidence,'supervisor_sha256':sha(profile/'supervisor-result.json'),'scope':'ordinary paused core phase+bookends' if p['mode']=='ordinary_core' else 'dense whole native harness including setup/oracles/replay/reporting; not per timed segment','no_new_performance_acceptance':True}
    try:
        python=Path(sys.executable).resolve();row=h.run_bounded(command=[str(python),str(Path(__file__).absolute()),'--profile-child',str(specpath),'--spec-sha256',specpin],cwd=Path(a['stage_root']),env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C'},directory=args.output/'execution',label='core-cpu-heap',cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['profile_tools'][str(python)],audit=audit)
    except BaseException as e:row={'status':'failed','failure':{'type':type(e).__name__,'reason':str(e)[:2048]},'resource_report_created':False}
    passed=row['status']=='passed';failure=row.get('failure')
    try:
        if passed:
            actual=read(profile/'supervisor-result.json')['actual_group']
            require('/sys/fs/cgroup'+actual['control_group']==row['control_group'] and actual['affinity']==row['cpu_ids'],'supervisor and actual outer group binding')
        require(authority_guard(args.authority,args.authority_sha256)[0]==a and sha(specpath)==specpin,'final source/spec endpoint')
    except Exception as e:passed=False;failure={'type':type(e).__name__,'reason':str(e)[:2048]}
    result={'schema':'bounded-core-profile-result-v1','collection':a['profiling']['collection'],'status':'capture_and_native_semantics_verified_independent_coverage_review_pending' if passed else 'failed_or_truncated_profile','authority_sha256':args.authority_sha256,'resource_report_sha256':sha(args.output/'execution/host-report.json') if (args.output/'execution/host-report.json').exists() else None,'failure':failure,'audit':row.get('audit'),'full_Rust_allocation_count_claim':False,'acceptance_or_causal_comparison_claim':False}
    h.save_json(args.output/'result.json',result);print(json.dumps({'status':result['status']}));return 0 if passed else 1

if __name__=='__main__':sys.exit(main())
