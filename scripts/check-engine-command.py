"""Measure one command without claiming a process-group memory high-water mark.

Linux wait4 CPU includes the command's waited-for descendants. ru_maxrss is the
largest individual process high-water mark, not simultaneous tree/group memory.
The Node caller forwards cancellation to our entire dedicated process group.
"""
import json
import os
import signal
import sys
import time


def main():
    output, command, *arguments = sys.argv[1:]
    received = []
    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, lambda number, _frame: received.append(number))
    started = time.perf_counter()
    pid = os.fork()
    if pid == 0:
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(signum, signal.SIG_DFL)
        try:
            os.execvpe(command, [command, *arguments], os.environ)
        except OSError as error:
            print(f"{command} could not run: {error}", file=sys.stderr)
            os._exit(127)
    _, status, usage = os.wait4(pid, 0)
    terminating_signal = os.WTERMSIG(status) if os.WIFSIGNALED(status) else None
    exit_code = os.WEXITSTATUS(status) if os.WIFEXITED(status) else None
    report = {
        "wall_seconds": time.perf_counter() - started,
        "user_seconds": usage.ru_utime,
        "system_seconds": usage.ru_stime,
        "cpu_scope": "wait4_command_and_its_reaped_descendants",
        "peak_rss_bytes": usage.ru_maxrss * 1024,
        "peak_rss_scope": "linux_wait4_largest_single_process_not_process_group_peak",
        "exit_code": exit_code,
        "signal": terminating_signal,
        "wrapper_received_signals": received,
    }
    with open(output, "x", encoding="utf-8") as stream:
        json.dump(report, stream, sort_keys=True)
        stream.write("\n")
    # A cancelled gate must never continue just because the command handles a
    # signal and exits successfully.
    if received:
        return 128 + received[-1]
    return (128 + terminating_signal) if terminating_signal else exit_code


if __name__ == "__main__":
    sys.exit(main())
