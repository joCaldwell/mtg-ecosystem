#!/usr/bin/env python3
"""Source-only prepared plan for real tiny kernel enforcement; no native workload."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

HERE=Path(__file__).absolute().parent
# Authentication is explicit BEFORE importing shared execution machinery.
def load_helper(expected):
    path=HERE/'measurement_host.py'
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
        raise ValueError('helper SHA mismatch before import')
    spec=importlib.util.spec_from_file_location('measurement_host_probe',path)
    h=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(h)
    return h

SUCCESS_CODE = '''import json,os,pathlib
raw=pathlib.Path('/proc/self/cgroup').read_text().strip().splitlines()
line=next(line for line in raw if line.startswith('0::'))
group=pathlib.Path('/sys/fs/cgroup')/line[3:].lstrip('/')
print(json.dumps({'scope':'tiny Python host-enforcement probe only; no native performance measurement',
'pid':os.getpid(),'affinity':sorted(os.sched_getaffinity(0)),'cgroup':line[3:],
'memory_max':(group/'memory.max').read_text().strip(),'swap_max':(group/'memory.swap.max').read_text().strip(),
'pids_max':(group/'pids.max').read_text().strip()}),flush=True)
'''
TIMEOUT_CODE='import time; time.sleep(30)'


def require(value,reason):
    if not value:
        raise ValueError(reason)


def guard(plan,h):
    require(plan['schema']=='measurement-host-kernel-probe-v1','plan schema')
    require(plan['status']=='prepared_only','prepared plan required')
    require(h.sha(Path(__file__).absolute())==plan['driver_sha256'],'probe source changed')
    require(h.sha(HERE/'measurement_host.py')==plan['helper_sha256'],'helper changed')
    exe=Path(plan['python']['path'])
    h.real_path(exe)
    require(h.sha(exe)==plan['python']['sha256'],'Python executable changed')
    require(plan['available_physical_cpu_ids']==h.physical_cpus(),'topology/allowedmask changed')
    require(plan['cpus']==plan['available_physical_cpu_ids'][:1],'probe must select exactly first available physical core')
    require(plan['success_seconds']==10 and plan['timeout_seconds']==1 and plan['memory_bytes']==h.MEMORY_BYTES,'fixed tiny probe bounds')
    return exe


def success_audit(raw,cpus,h):
    require(raw['scope']=='tiny Python host-enforcement probe only; no native performance measurement','unexpected probe output')
    require(type(raw['pid']) is int and raw['pid']>1 and raw['affinity']==cpus,'actual descendant affinity mismatch')
    require(raw['cgroup'].startswith('/') and 'mtg-measure-kernel-success-' in raw['cgroup'],'unexpected child cgroup')
    require((raw['memory_max'],raw['swap_max'],raw['pids_max'])==(str(h.MEMORY_BYTES),'0',str(h.TASKS)),'descendant kernel cap mismatch')
    return {'actual_child_kernel_caps_and_affinity_verified':True}


def validate_outcomes(success,negative):
    require(success['status']=='passed','positive enforcement probe failed')
    require(negative['status']=='failed' and negative.get('failure',{}).get('type')=='TimeoutError','forced timeout did not fail explicitly')
    require('kernel_initial' in success and 'kernel_initial' in negative,'both workloads must have verified caps before release')
    require(success.get('audit',{}).get('actual_child_kernel_caps_and_affinity_verified') is True,'positive descendant audit missing')
    for row in [success,negative]:
        initial=row['kernel_initial']
        require((initial.get('memory_max'),initial.get('memory_swap_max'),initial.get('pids_max'))==(str(2*1024**3),'0','128'),'initial actual caps missing/mismatched')
        require(row['cleanup']['unit_empty'] is True and row['cleanup']['launcher_reaped'] is True,'probe process group not closed')
        require(row['kernel_final']['memory_peak_bytes']<=2*1024**3,'kernel memory upper bound')
        require(all(row['kernel_final']['memory_events'].get(k,0)==0 for k in ('oom','oom_kill','oom_group_kill','max')),'unexpected memory cap event')
    return {'status':'tiny_kernel_enforcement_and_timeout_verified','native_measurement_claim':False}


def execute(planpath,expected,h):
    require(h.sha(planpath)==expected,'plan SHA changed')
    plan=h.read_json(planpath)
    exe=guard(plan,h)
    output=Path(plan['output'])
    h.real_path(output.parent,private=True)
    require(not output.exists(),'new private probe output required')
    output.mkdir(mode=0o700)
    result={'status':'running','scope':'tiny OS mechanism integration only; not native timing or product acceptance','plan_sha256':expected,'attempts':[]}
    h.save_json(output/'status.json',result)
    try:
        env={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C'}
        def audit():
            require(h.sha(planpath)==expected,'plan mutated')
            guard(plan,h)
            return success_audit(h.read_json(output/'success/workload.stdout'),plan['cpus'],h)
        success=h.run_bounded(command=[str(exe),'-c',SUCCESS_CODE],cwd=output,env=env,directory=output/'success',label='kernel-success',cpus=plan['cpus'],timeout_seconds=10,expected_executable_sha256=plan['python']['sha256'],audit=audit)
        result['attempts'].append({'kind':'success','status':success['status'],'report_sha256':h.sha(output/'success/host-report.json')})
        h.save_json(output/'status.json',result)
        require(success['status']=='passed','positive failed; do not launch further work')
        guard(plan,h)
        negative=h.run_bounded(command=[str(exe),'-c',TIMEOUT_CODE],cwd=output,env=env,directory=output/'forced-timeout',label='kernel-timeout',cpus=plan['cpus'],timeout_seconds=1,expected_executable_sha256=plan['python']['sha256'])
        result['attempts'].append({'kind':'forced_timeout','status':negative['status'],'report_sha256':h.sha(output/'forced-timeout/host-report.json')})
        require(h.sha(planpath)==expected,'plan mutated')
        guard(plan,h)
        observed=h.read_json(output/'success/workload.stdout')
        require(success['control_group']=='/sys/fs/cgroup'+observed['cgroup'] and success['payload']['pid']!=observed['pid'],'actual child and gated group binding')
        result.update(validate_outcomes(success,negative))
    except BaseException as e:
        result.update(status='failed',failure={'type':type(e).__name__,'reason':str(e)[:2048]})
    h.save_json(output/'status.json',result)
    return result


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--helper-sha256',required=True)
    p.add_argument('--prepare-plan',type=Path)
    p.add_argument('--output',type=Path)
    p.add_argument('--execute-plan',type=Path)
    p.add_argument('--plan-sha256')
    args=p.parse_args()
    os.umask(0o077)
    h=load_helper(args.helper_sha256)
    if args.execute_plan:
        result=execute(args.execute_plan,args.plan_sha256,h)
        print(json.dumps({'status':result['status']}))
        return 0 if result['status']=='tiny_kernel_enforcement_and_timeout_verified' else 1
    require(args.prepare_plan is not None and args.output is not None,'explicit prepare inputs; no automatic execution')
    h.real_path(args.prepare_plan.parent,private=True)
    h.real_path(args.output.parent,private=True)
    require(not args.prepare_plan.exists() and not args.output.exists(),'new plan/output required')
    cpus=h.physical_cpus()
    exe=Path(sys.executable).resolve()
    plan={'schema':'measurement-host-kernel-probe-v1','status':'prepared_only','output':str(args.output),
          'driver_sha256':h.sha(Path(__file__).absolute()),'helper_sha256':args.helper_sha256,
          'python':{'path':str(exe),'sha256':h.sha(exe)},'available_physical_cpu_ids':cpus,'cpus':cpus[:1],
          'success_seconds':10,'timeout_seconds':1,'memory_bytes':h.MEMORY_BYTES}
    guard(plan,h)
    h.save_json(args.prepare_plan,plan)
    print(json.dumps({'status':'prepared_only','plan_sha256':h.sha(args.prepare_plan)}))
    return 0


if __name__=='__main__':
    sys.exit(main())
