#!/usr/bin/env python3
"""Direct-native intercepted allocation API counts. No builds or paused-signal entry.
Installed /usr/bin/memusage (GNU libc) is the primary environment authority.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

HERE=Path(__file__).absolute().parent
MAX_TEXT=64*1024**2
REQUIRED_APIS={'malloc','calloc','realloc','free'}
APIS=REQUIRED_APIS|{'mmap','mmap64','mmap(r)','mmap(w)','mmap(a)','mremap','munmap'}

def require(ok,why):
    if not ok:raise ValueError(why)

def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()

def plain(path):
    p=Path(path);require(p.is_absolute() and '..' not in p.parts,'absolute plain path required')
    for q in (p,*p.parents):require(not q.is_symlink(),'symlink ancestry')
    require(p.is_file(),'regular plain file required');return p

def load(name,pin):
    p=plain(HERE/name);require(re.fullmatch('[0-9a-f]{64}',pin or '') and sha(p)==pin,'host SHA before import')
    s=importlib.util.spec_from_file_location('allocation_'+name.replace('.','_').replace('-','_'),p)
    m=importlib.util.module_from_spec(s);sys.modules[s.name]=m;s.loader.exec_module(m);return m

def tools_guard(a):
    pins=a['allocation_tools'];require(type(pins) is dict,'tool map required')
    required={'/usr/bin/memusage','/usr/lib/libmemusage.so','/usr/lib/libc.so.6',str(Path('/usr/lib64/ld-linux-x86-64.so.2').resolve())}
    require(required<=set(pins),'memusage primary launcher, interposer and loader dependencies required')
    for p,pin in pins.items():require(re.fullmatch('[0-9a-f]{64}',pin or '') and sha(plain(p))==pin,'allocation tool changed')
    require(a['allocation_mode']=={'family':'kernel','case':'dense','scope':'whole-native-harness','no_timer':True,'trace_mmap':True},'only whole unchanged kernel/dense; no paused USR2 entry')
    return pins

def guard(path,pin):
    require(sha(plain(path))==pin,'approved authority SHA')
    # Decode only after pin; parser then reused from authenticated stdlib-only fixed source.
    raw=json.loads(path.read_text());scripts=raw['host_scripts']
    fixed=load('run-fixed-work.py',scripts.get(str(HERE/'run-fixed-work.py')))
    h=fixed.load_host(scripts.get(str(HERE/'measurement_host.py')))
    dense=load('run-dense-diagnostic-v1.py',scripts.get(str(HERE/'run-dense-diagnostic-v1.py')))
    a,contracts,refs,instruction=dense.source_guard(fixed,path,pin,'kernel','dense')
    require(sha(plain(__file__))==a['host_scripts'].get(str(Path(__file__).absolute())),'allocation host source pin')
    tools_guard(a)
    return a,h,dense,contracts,refs,instruction

def parse_api_counts(raw):
    require(type(raw) is str and len(raw.encode())<=MAX_TEXT and '\0' not in raw,'bounded raw API stderr')
    text=re.sub(r'\x1b\[[0-9;]*m','',raw)
    require('\x1b' not in text,'unknown terminal escape')
    rows={}
    for line in text.splitlines():
        if not re.match(r'^\s*[A-Za-z][A-Za-z0-9_()]*\s*\|',line):continue
        if re.match(r'^\s*function\s*\|',line):continue
        match=re.fullmatch(r'\s*([A-Za-z][A-Za-z0-9_()]*)\s*\|\s*([0-9]+)(?:\s*\|\s*|\s+)([0-9]+).*',line)
        require(match is not None,'malformed allocation API row')
        name,count,total_bytes=match.groups();require(name in APIS and name not in rows,'unknown or duplicate allocation API row')
        rows[name]={'calls':int(count),'total_memory_bytes_as_reported':int(total_bytes),'raw_row':line}
    require(REQUIRED_APIS<=set(rows),'four unique actual allocation API rows required')
    require(sum(rows[k]['calls'] for k in REQUIRED_APIS)>0,'no intercepted allocation work')
    warnings=[s for s in text.splitlines() if re.search(r'warning|error|cannot|failed|ignored',s,re.I)]
    return {'intercepted_api_rows':rows,'unemitted_api_rows':sorted(APIS-set(rows)),'warnings':warnings,
            'scope':'whole native harness including setup, warmup, five measured repetitions, proof/replay/reporting; intercepted ABI calls only',
            'complete_Rust_allocation_count_claim':False,'isolated_core_or_per_segment_count_claim':False}

def command_environment(a,output):
    tools_guard(a)
    execution=output/'execution';trace=execution/'allocation-api.bin'
    env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','HOME':str(output/'home'),'TMPDIR':str(output/'tmp'),
         'LD_PRELOAD':'/usr/lib/libmemusage.so','MEMUSAGE_OUTPUT':str(trace),'MEMUSAGE_NO_TIMER':'yes','MEMUSAGE_TRACE_MMAP':'yes',
         'DENSE_KERNEL_OUTER_GUARD':'120s-2gib-v1','DENSE_KERNEL_CASE':'dense'}
    cmd=[a['test_executable']['path'],'--exact','--ignored','--nocapture','--test-threads=1','engine::dense_kernel_tests::diagnostic::dense_kernel_diagnostic_host_adapter']
    return cmd,env,trace

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--authority',required=True,type=Path);ap.add_argument('--authority-sha256',required=True);ap.add_argument('--output',required=True,type=Path);ap.add_argument('--execute',action='store_true');args=ap.parse_args()
    require(args.execute,'root-authorized explicit execution only');os.umask(0o077)
    a,h,dense,contracts,refs,instruction=guard(args.authority,args.authority_sha256)
    h.real_path(args.output.parent,private=True);require(not args.output.exists(),'fresh output required');args.output.mkdir(mode=0o700)
    for n in ('home','tmp'):(args.output/n).mkdir(mode=0o700)
    command,env,trace=command_environment(a,args.output)
    def audit():
        require(guard(args.authority,args.authority_sha256)[0]==a,'authority/source endpoint')
        semantic=dense.validate_output(dense.parse_stdout(args.output/'execution/workload.stdout','kernel','dense'),'kernel','dense',a['engine_build_id'],contracts,refs,instruction)
        err=plain(args.output/'execution/workload.stderr');require(err.stat().st_size<=MAX_TEXT,'bounded API stderr')
        counts=parse_api_counts(err.read_text());require(plain(trace).stat().st_size>0,'actual API trace required')
        require(guard(args.authority,args.authority_sha256)[0]==a,'post-audit source endpoint')
        return {'native_semantics':semantic,'allocation_counts':counts,'stderr_sha256':sha(err),'binary_trace':{'sha256':sha(trace),'bytes':trace.stat().st_size},'no_acceptance_or_causal_claim':True}
    try:
        row=h.run_bounded(command=command,cwd=Path(a['stage_root']),env=env,directory=args.output/'execution',label='dense-api-counts',cpus=a['cpu_ids'],timeout_seconds=120,expected_executable_sha256=a['test_executable']['sha256'],audit=audit)
    except BaseException as e:row={'status':'failed','failure':{'type':type(e).__name__,'reason':str(e)[:2048]},'resource_report_created':False}
    passed=row['status']=='passed';failure=row.get('failure')
    try:require(guard(args.authority,args.authority_sha256)[0]==a,'final source/tool endpoint')
    except Exception as e:passed=False;failure={'type':type(e).__name__,'reason':str(e)[:2048]}
    result={'schema':'bounded-api-allocation-count-result-v1','status':'native_semantics_and_intercepted_API_counts_verified' if passed else 'failed_or_truncated','engine_build_id':a['engine_build_id'],'authority_sha256':args.authority_sha256,'failure':failure,'audit':row.get('audit'),'resource_report_sha256':sha(args.output/'execution/host-report.json') if (args.output/'execution/host-report.json').exists() else None,'whole_harness_only':True,'complete_Rust_allocation_count_claim':False,'acceptance_or_causal_comparison_claim':False}
    h.save_json(args.output/'result.json',result);print(json.dumps({'status':result['status']}));return 0 if passed else 1

if __name__=='__main__':sys.exit(main())
