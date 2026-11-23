#!/usr/bin/env python3
"""Build Rust test binaries once, then run several concurrently under one budget.

`cargo test` runs test binaries one after another, so one long proof binary
idles the machine while later binaries wait. This builds the same selection
with `cargo test --no-run` (arguments before `--`), then runs the test
executables in their package directories, at most `--max-binaries` at a time
(default 4), largest executable first. Each binary receives
`--test-threads=N` so binaries x threads stays within `--workers` (default
min(CPUs, 16)); libtest's thread pool is the only concurrency governor, which
bounds resident games and memory. A caller's own `--test-threads` wins and
lowers the binary cap instead. Arguments after `--` go to every binary
(filters, --ignored). Output is buffered per binary: passing binaries print
their summary line, failing ones print everything. The run reports wall time
and the peak total resident memory of concurrently running binaries. Exits
nonzero if the build or any binary fails, if the whole selection executes zero
tests, or (without a filter) if a binary runs zero tests and declares no
ignored opt-in test. Run under the
target's native lock (check-engine owns it).
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time


def split_arguments(argv):
    """(runner options, cargo arguments, libtest arguments)."""
    options = {'max_binaries': 4, 'workers': min(os.cpu_count() or 4, 16)}
    argv = list(argv)
    while argv and argv[0] in ('--max-binaries', '--workers'):
        options[argv[0][2:].replace('-', '_')] = int(argv[1])
        argv = argv[2:]
    if '--' in argv:
        index = argv.index('--')
        return options, argv[:index], argv[index + 1:]
    return options, argv, []


def plan(options, test_args):
    """(concurrent binaries, libtest arguments) within the worker budget."""
    workers = max(1, options['workers'])
    binaries = max(1, min(options['max_binaries'], workers))
    explicit = [a for a in test_args if a == '--test-threads' or a.startswith('--test-threads=')]
    if explicit:
        arg = test_args[test_args.index('--test-threads') + 1] if '--test-threads' in test_args else explicit[0].split('=', 1)[1]
        return max(1, min(binaries, workers // max(1, int(arg)))), list(test_args)
    return binaries, [*test_args, f'--test-threads={max(1, workers // binaries)}']


def executables(messages):
    """Test executables from `cargo --message-format=json` lines, largest first."""
    found = []
    for line in messages:
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if message.get('reason') == 'compiler-artifact' and message.get('executable') and message['profile']['test']:
            found.append((message['target']['name'], message['executable'], str(Path(message['manifest_path']).parent)))
    size = lambda path: os.path.getsize(path) if os.path.exists(path) else 0
    return sorted(found, key=lambda binary: -size(binary[1]))


def summary(output):
    lines = [line for line in output.splitlines() if line.startswith('test result:')]
    return lines[-1] if lines else 'no test result line'


def counts(output):
    """(passed, ignored) from libtest's final summary, or None."""
    line = summary(output)
    if not line.startswith('test result:'):
        return None
    try:
        fields = dict(reversed(part.strip().split(' ', 1)) for part in line.split('.', 1)[1].split(';')[:3])
        return int(fields['passed']), int(fields['ignored'])
    except (KeyError, IndexError, ValueError):
        return None


def passed(output):
    """Executed (passed) tests from libtest's final summary, or None."""
    found = counts(output)
    return found and found[0]


def empty_selection(results, test_args):
    """Binaries that executed no test; any filter allows per-binary zeros."""
    found = {name: counts(output) or (0, 0) for name, (code, output, _) in results.items() if not code}
    # A selection that executes nothing fails as a whole, even if every binary
    # only declares ignored tests.
    if found and sum(ran for ran, _ in found.values()) == 0:
        return sorted(found)
    if any(not arg.startswith('-') for arg in test_args):
        return []
    # An all-ignored binary is an explicit opt-in witness beside executing ones.
    return sorted(name for name, (ran, ignored) in found.items() if ran == 0 and ignored == 0)


def resident_bytes(pid):
    try:
        with open(f'/proc/{pid}/status') as status:
            for line in status:
                if line.startswith('VmRSS:'):
                    return int(line.split()[1]) * 1024
    except OSError:
        pass
    return 0


def main(argv):
    options, cargo_args, test_args = split_arguments(argv)
    build = subprocess.run(['cargo', 'test', '--no-run', '--message-format=json-render-diagnostics', *cargo_args],
                           stdout=subprocess.PIPE, text=True)
    if build.returncode:
        print(f'engine-test-binaries: build failed ({build.returncode})', file=sys.stderr)
        return build.returncode
    binaries = executables(build.stdout.splitlines())
    if not binaries:
        print('engine-test-binaries: selection built no test binaries', file=sys.stderr)
        return 2
    concurrent, binary_args = plan(options, test_args)
    queue = list(binaries)
    lock = threading.Lock()
    running = {}
    results = {}
    peak = [0]

    def worker():
        while True:
            with lock:
                if not queue:
                    return
                name, path, directory = queue.pop(0)
            start = time.monotonic()
            process = subprocess.Popen([path, *binary_args], cwd=directory, stdout=subprocess.PIPE,
                                       stderr=subprocess.STDOUT, text=True)
            with lock:
                running[name] = process.pid
            output, _ = process.communicate()
            with lock:
                running.pop(name)
                results[name] = (process.returncode, output, time.monotonic() - start)

    def sample():
        while any(thread.is_alive() for thread in threads):
            with lock:
                pids = list(running.values())
            peak[0] = max(peak[0], sum(resident_bytes(pid) for pid in pids))
            time.sleep(0.25)

    started = time.monotonic()
    threads = [threading.Thread(target=worker) for _ in range(min(concurrent, len(binaries)))]
    for thread in threads:
        thread.start()
    sampler = threading.Thread(target=sample)
    sampler.start()
    for thread in threads:
        thread.join()
    sampler.join()
    failed = sorted(name for name, (code, _, _) in results.items() if code)
    empty = empty_selection(results, test_args)
    for name, _, _ in binaries:
        code, output, wall = results[name]
        if code:
            print(output)
        print(f'{"FAIL" if code else "ok  "} {name} ({wall:.1f}s): {summary(output)}')
    print(f'{len(binaries)} binaries, {concurrent} at a time with {" ".join(binary_args)}, '
          f'in {time.monotonic() - started:.1f}s; peak total RSS {peak[0] / 2**30:.2f} GiB; '
          f'failed: {", ".join(failed) or "none"}; executed no tests: {", ".join(empty) or "none"}')
    return 1 if failed or empty else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
