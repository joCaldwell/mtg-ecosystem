#!/usr/bin/env python3
"""Phase 1 inner loop: targeted tests, selected deck coverage, one final gate.

  test ARGS...     npm run engine:dev -- test ARGS (e.g. -p mtg-compiler --lib FILTER)
  coverage         selected deck coverage on R4 (~1-3 s after the build) with a delta
                   against the saved baseline; --save-baseline records the new report
  final            check:engine:fast, one whole-archive scan, selected/full parity and,
                   with --base-records, whole-archive regressions against base receipts

Run `coverage --save-baseline` once before editing. Run `final` once, on the
commit you publish. Outputs live under ignored generated/phase1-fast-inner-loop/.
"""
import argparse
import csv
from datetime import datetime, timezone
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / "generated" / "phase1-fast-inner-loop"
REPORT = runpy.run_path(str(ROOT / "scripts" / "phase1-deck-coverage.py"))


def shared_generated(name):
    """Prefer this checkout's generated/ input, else the primary checkout's."""
    local = ROOT / "generated" / name
    if local.exists():
        return local
    common = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], cwd=ROOT,
                            capture_output=True, text=True).stdout.strip()
    return Path(common).parent / "generated" / name if common else local


def run(command, **kwargs):
    print("+ " + " ".join(map(str, command)), file=sys.stderr, flush=True)
    return subprocess.run(list(map(str, command)), cwd=ROOT, **kwargs).returncode


def mtg_sim(args):
    if args.mtg_sim:
        return args.mtg_sim
    # Like engine:dev: engine-env selects this worktree's target and
    # engine-cache.py names that target's lock.
    target = subprocess.run(["python3", "-B", "scripts/engine-cache.py", "path"], cwd=ROOT,
                            capture_output=True, text=True, check=True).stdout.strip()
    binary = Path(target) / "release" / "mtg-sim"
    if not args.no_build:
        # Release scans the whole archive in seconds; quick (opt-level 0) does not.
        code = run(["sh", "scripts/engine-env.sh", "sh", "-ec", 'lock=$(python3 -B scripts/engine-cache.py lock); '
                    'mkdir -p "$(dirname -- "$lock")"; exec flock "$lock" cargo build --locked --release -p mtg-sim'])
        if code:
            sys.exit(code)
    if not binary.exists():
        sys.exit(f"phase1-fast-loop: missing {binary}; build it or pass --mtg-sim")
    return binary


def run_dir(kind):
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    return STATE / "runs" / f"{stamp}-{kind}"


def deck_report(args, binary, coverage, out, compare):
    argv = ["--decks", args.decks, "--archive", args.archive, "--coverage", coverage]
    if binary:
        argv += ["--mtg-sim", binary]
    if compare and compare.exists():
        argv += ["--compare", compare, "--top", str(args.top)]
    REPORT["main"](list(map(str, argv)), out)
    return json.loads((out / "deck-gap-report.json").read_bytes())


def coverage(args):
    binary = mtg_sim(args)
    directory = run_dir("selected")
    started = time.perf_counter()
    deck_report(args, binary, directory / "coverage", directory, args.baseline)
    elapsed = time.perf_counter() - started
    if args.save_baseline:
        args.baseline.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(directory / "deck-gap-report.json", args.baseline)
    print(f"\nSelected coverage and report: {elapsed:.2f} s; receipts in {directory}", file=sys.stderr)


def report_body(report):
    return {key: value for key, value in report.items() if key != "inputIdentity"}


def read_records(path):
    with path.open(newline="", encoding="utf-8") as records:
        rows = csv.reader(records, delimiter="\t", quoting=csv.QUOTE_NONE)
        next(rows)
        return {row[0]: row[2] for row in rows}


def final(args):
    if not args.skip_check_fast and run(["sh", "scripts/engine-env.sh", "npm", "run", "check:engine:fast"]):
        sys.exit("phase1-fast-loop: check:engine:fast failed")
    binary = mtg_sim(args)
    directory = run_dir("final")
    full = directory / "full-coverage"
    started = time.perf_counter()
    if run([binary, "catalog", "coverage", args.archive, "--out", full], stdout=subprocess.DEVNULL):
        sys.exit("phase1-fast-loop: whole-archive coverage failed")
    full_seconds = time.perf_counter() - started
    full_report = deck_report(args, None, full, directory / "full-report", args.baseline)
    started = time.perf_counter()
    selected_report = deck_report(args, binary, directory / "selected-coverage", directory / "selected-report", None)
    selected_seconds = time.perf_counter() - started
    parity = report_body(full_report) == report_body(selected_report)
    summary = json.loads((full / "summary.json").read_bytes())
    lines = [f"Whole archive: {summary['lowered_validated']}/{summary['commander_legal']} Commander-legal "
             f"lowered_validated ({full_seconds:.1f} s); selected deck run {selected_seconds:.2f} s; "
             f"selected/full deck report parity: {'exact' if parity else 'MISMATCH'}"]
    regressions = []
    if args.base_records:
        base, head = read_records(args.base_records), read_records(full / "records.tsv")
        regressions = sorted(i for i, status in base.items()
                             if status == "lowered_validated" and head.get(i) != "lowered_validated")
        gained = sum(status == "lowered_validated" != base.get(i) for i, status in head.items())
        lines.append(f"Against base receipts: {gained} newly lowered, {len(regressions)} regressed")
        lines += [f"  regressed {i}" for i in regressions[:50]]
    print("\n".join(lines) + f"\nReceipts: {directory}")
    if not parity or regressions:
        sys.exit(1)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    test = commands.add_parser("test", help="targeted Cargo test through engine:dev")
    test.add_argument("cargo_args", nargs=argparse.REMAINDER)
    for name in ("coverage", "final"):
        command = commands.add_parser(name)
        command.add_argument("--archive", type=Path, default=shared_generated("oracle-ast-phase1-r4.jsonl"))
        command.add_argument("--decks", type=Path, default=shared_generated("phase1-target-decks.json"))
        command.add_argument("--baseline", type=Path, default=STATE / "baseline-deck-gap-report.json")
        command.add_argument("--mtg-sim", type=Path, help="existing release mtg-sim; skips the build")
        command.add_argument("--no-build", action="store_true", help="reuse the existing release mtg-sim")
        command.add_argument("--top", type=int, default=20)
    commands.choices["coverage"].add_argument("--save-baseline", action="store_true")
    commands.choices["final"].add_argument("--base-records", type=Path,
                                           help="whole-archive records.tsv produced from the base commit")
    commands.choices["final"].add_argument("--skip-check-fast", action="store_true",
                                           help="check:engine:fast already passed on this exact commit")
    args = parser.parse_args(argv)
    if args.command == "test":
        sys.exit(run(["npm", "run", "engine:dev", "--", "test", *args.cargo_args]))
    {"coverage": coverage, "final": final}[args.command](args)


if __name__ == "__main__":
    main()
