#!/usr/bin/env python3
"""Eleven fixed18 batches, fail-stop, authenticated single-batch OS envelope."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import math
import os
import signal
import statistics
import sys

STOP = False


def stopped(_signum, _frame):
    global STOP
    STOP = True


def inputs(expected):
    path = Path(__file__).absolute().with_name('host_inputs.py')
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise ValueError('input guard SHA before import')
    spec = importlib.util.spec_from_file_location('authenticated_pod_inputs', path)
    out = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(out)
    return out


def dimensions(h, a):
    h.need(a['schema'] == 'final-pod-series-authority-v1' and
           type(a['warmup_batches']) is int and a['warmup_batches'] == 1 and
           type(a['measured_batches']) is int and a['measured_batches'] == 10 and
           a['batch_indices'] == list(range(11)) and
           all(type(x) is int for x in a['batch_indices']) and
           a['seed_indices'] == list(range(18)) and
           all(type(x) is int for x in a['seed_indices']) and
           a['retry_policy'] == 'fail-stop-no-retry-v1', 'exact1+10 x18 fail-stop worklist')


def guard(h, path, expected):
    h.pin({'path': str(path), 'sha256': expected})
    a = h.read(path)
    dimensions(h, a)
    for name, value in a['host_sources'].items():
        h.pin({'path': name, 'sha256': value})
    for name in [Path(__file__).absolute(), Path(h.__file__).absolute()]:
        h.need(a['host_sources'].get(str(name)) == h.sha(name), 'series/input source pin')
    fixed = h.module(a['fixed_driver'], 'pod_series_authenticated_fixed')
    helper = fixed.load_host(a['measurement_helper_sha256'])
    ordinary = h.module(a['ordinary_driver'], 'pod_series_authenticated_ordinary')
    batch = h.module(a['single_batch_driver'], 'pod_series_authenticated_batch')
    for role in ['fixed_driver', 'ordinary_driver', 'single_batch_driver']:
        h.need(a['host_sources'].get(a[role]['path']) == a[role]['sha256'], 'host dependency source-map pin')
    helper_path = str(Path(a['fixed_driver']['path']).with_name('measurement_host.py'))
    h.need(a['host_sources'].get(helper_path) == a['measurement_helper_sha256'], 'unchanged helper source-map pin')
    pin = a['single_batch_authority']
    actual, _decks, _limits = batch.auth(ordinary, fixed, h.pin(pin), pin['sha256'])
    h.need(actual['engine_build_id'] == a['engine_build_id'] and
           actual['timeout_seconds'] == 120 and len(actual['cpu_ids']) == 1 and
           actual['current_pod_cases'] == list(range(18)), 'current exact singlebatch Build/caps/cases')
    h.private_parent(a['run_parent'])
    return a, actual


def batch_command(a, output):
    return [sys.executable, a['single_batch_driver']['path'],
            '--ordinary-driver-sha256', a['ordinary_driver']['sha256'],
            '--fixed-driver-sha256', a['fixed_driver']['sha256'],
            '--helper-sha256', a['measurement_helper_sha256'],
            '--authority', a['single_batch_authority']['path'],
            '--authority-sha256', a['single_batch_authority']['sha256'],
            '--output', str(output), '--execute']


def semantic(h, path):
    value = h.read(path)
    elapsed = value.pop('elapsed_millis')
    h.need(type(elapsed) is int and elapsed >= 0, 'only declared elapsed field omitted')
    # One complete Evidence at a time, same full-field semantics as original.
    digest = hashlib.sha256()
    for part in json.JSONEncoder(sort_keys=True, separators=(',', ':'), ensure_ascii=False,
                                 allow_nan=False).iterencode(value):
        digest.update(part.encode())
    return digest.hexdigest()


def batch_result(h, value, bounded, build):
    h.need(bounded['status'] == 'passed' and bounded['direct_child_reaped'] is True and
           bounded['owned_group_empty'] is True and bounded['exit_code'] == 0,
           'controller exited successfully with complete owned cleanup')
    h.need(value['schema'] == 'bounded-current18-pod-batch-v1' and
           value['status'] == 'all18_complete_and_verified' and
           value['engine_build_id'] == build and value['failure'] is None and
           value['audit']['all18_complete_verified'] is True and
           type(value['audit']['planned_pods']) is int and value['audit']['planned_pods'] == 18 and
           type(value['audit']['completed_pods']) is int and value['audit']['completed_pods'] == 18,
           'all18 completed and source-bound replay verified')
    for name in ['generation_sum_ns', 'whole_batch_ns']:
        h.need(type(value['audit'][name]) is int and value['audit'][name] > 0, 'positive full work timing')
    for rate, nanos in [('generation_pods_per_second', 'generation_sum_ns'),
                        ('whole_batch_pods_per_second', 'whole_batch_ns')]:
        h.need(type(value[rate]) in (int, float) and math.isfinite(value[rate]) and
               value[rate] == 18e9 / value['audit'][nanos], 'exact18-pod rate denominator')


def resource_result(h, resource, actual):
    h.need(resource['schema'] == 'bounded-measurement-v1' and resource['status'] == 'passed' and
           resource['deadline_seconds'] == 120 and resource['cpu_ids'] == actual['cpu_ids'] and
           resource['executable_sha256'] == actual['test_executable']['sha256'], 'exact bounded singlebatch identity')
    h.need(h.canonical(resource['limits']) == h.canonical({'memory_bytes': 2**31, 'postexit_child_rss_guard_bytes': 2**31,
           'swap_bytes': 0, 'tasks': 128, 'systemd_runtime_max_seconds': 125}), 'unchanged singlebatch resource caps')
    h.need(resource['cleanup']['unit_empty'] is True and resource['cleanup']['launcher_reaped'] is True and
           resource['workload']['exit_code'] == 0, 'actual service cleanup/reap and workload exit')
    rss = resource['workload']['ru_maxrss_bytes']
    peak = resource['kernel_final']['memory_peak_bytes']
    h.need(type(rss) is int and 0 < rss <= 2**31 and type(peak) is int and 0 < peak <= 2**31,
           'actual independent RSS/charged peak')
    h.need(not any(resource['kernel_final']['memory_events'].get(k, 0)
                   for k in ['max', 'oom', 'oom_kill', 'oom_group_kill']), 'no pressure/OOM event')


def execute(h, args):
    a, original = guard(h, args.authority, args.authority_sha256)
    out = h.fresh(args.output, a['run_parent'])
    out.mkdir(mode=0o700)
    status = {'schema': 'final18-pod-repeat-result-v1', 'status': 'running',
        'engine_build_id': a['engine_build_id'], 'authority_sha256': args.authority_sha256,
        'runner_sha256': h.sha(Path(__file__).absolute()), 'planned_batches': 11,
        'planned_pods': 198, 'warmup_batches': 1, 'measured_batches': 10,
        'retry_policy': 'fail-stop-no-retry-v1', 'attempts': [],
        'unstarted_batch_indices': list(range(11))}
    h.save(out / 'status.json', status)
    reference = None
    try:
        for index in range(11):
            h.need(not STOP, 'cancelled series before next batch')
            h.need(guard(h, args.authority, args.authority_sha256) == (a, original), 'immutable authority/source before batch')
            directory = out / f'batch-{index:02}'
            attempt = {'index': index, 'warmup': index == 0, 'status': 'started',
                       'command': batch_command(a, directory), 'planned_pods': 18}
            status['attempts'].append(attempt)
            status['unstarted_batch_indices'] = list(range(index + 1, 11))
            h.save(out / 'status.json', status)
            attempt['controller'] = h.run_owned(attempt['command'], Path(original['stage_root']),
                out / f'batch-{index:02}-controller.stdout', out / f'batch-{index:02}-controller.stderr',
                150, lambda: STOP)
            # Retain the failed attempted row BEFORE parsing or raising.
            attempt['status'] = 'returned_pending_audit'
            h.save(out / 'status.json', status)
            value = h.read(directory / 'result.json')
            attempt['result_sha256'] = h.sha(directory / 'result.json')
            attempt['batch_status'] = value['status']
            batch_result(h, value, attempt['controller'], a['engine_build_id'])
            resource_path = directory / 'execution/host-report.json'
            resource = h.read(resource_path)
            h.need(resource['status'] == 'passed' and value['host_report_sha256'] == h.sha(resource_path), 'actual singlebatch bounded proof')
            resource_result(h, resource, original)
            signatures = [semantic(h, directory / 'games' / f'game-{game:02}.json') for game in range(18)]
            if reference is None:
                reference = signatures
            h.need(signatures == reference, 'ALL Evidence fields except elapsed equal warmup')
            h.need(guard(h, args.authority, args.authority_sha256) == (a, original), 'immutable authority/source after batch')
            attempt.update(status='passed', semantic_evidence_sha256=signatures,
                generation_sum_ns=value['audit']['generation_sum_ns'], whole_batch_ns=value['audit']['whole_batch_ns'],
                generation_pods_per_second=value['generation_pods_per_second'], whole_batch_pods_per_second=value['whole_batch_pods_per_second'])
            h.save(out / 'status.json', status)
        measured = [row for row in status['attempts'] if not row['warmup']]
        h.need(len(measured) == 10 and all(row['status'] == 'passed' for row in status['attempts']), 'all11 full batches, no hidden retry')
        h.need(guard(h, args.authority, args.authority_sha256) == (a, original), 'final authority/source barrier')
        def summarize(key):
            values = [row[key] for row in measured]
            return {'raw': values, 'median': statistics.median(values), 'min': min(values),
                    'max': max(values), 'sample_stddev': statistics.stdev(values),
                    'inclusive_quartiles': statistics.quantiles(values, n=4, method='inclusive')}
        status.update(status='all11_batches_semantically_verified', measured_complete_pods=180,
            warmup_complete_pods=18, summaries={key: summarize(key) for key in
            ['generation_pods_per_second', 'whole_batch_pods_per_second', 'generation_sum_ns', 'whole_batch_ns']})
    except BaseException as error:
        status.update(status='failed_or_incomplete', failure=type(error).__name__ + ': ' + str(error),
                      summaries=None, measured_complete_pods=None)
        if status['attempts'] and status['attempts'][-1]['status'] != 'passed':
            status['attempts'][-1]['status'] = 'failed_preserved_no_retry'
    if STOP:
        status.update(status='failed_or_incomplete', failure='interrupted through final publication',
                      summaries=None, measured_complete_pods=None)
    h.save(out / 'status.json', status)
    if STOP and status['status'] == 'all11_batches_semantically_verified':
        status.update(status='failed_or_incomplete', failure='interrupted during final publication',
                      summaries=None, measured_complete_pods=None)
        h.save(out / 'status.json', status)
    return status


def main():
    global STOP
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--authority', type=Path, required=True)
    parser.add_argument('--authority-sha256', required=True)
    parser.add_argument('--inputs-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--execute', action='store_true', required=True)
    args = parser.parse_args()
    os.umask(0o077)
    STOP = False
    previous = {sig: signal.getsignal(sig) for sig in [signal.SIGTERM, signal.SIGINT]}
    try:
        for sig in previous:
            signal.signal(sig, stopped)
        result = execute(inputs(args.inputs_sha256), args)
        print(json.dumps({'status': result['status']}), flush=True)
        return 0 if result['status'] == 'all11_batches_semantically_verified' else 1
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == '__main__':
    raise SystemExit(main())
