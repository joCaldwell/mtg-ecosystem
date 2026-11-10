# Tiny engine runtime smoke and bounded reproduction

`mtg-sim smoke` is a focused developer check, not engine acceptance. It compiles
five exact archived source fixtures into the existing base catalog and admits
`ScenarioConfig::land_return()` (three physical cards, four seats). It activates
Island's mana ability and passes priority through public observations, decision
tokens, response validation and submission. It asserts the initial state, then
Island tapped with one blue mana, then priority at player 1. Each input prefix is
replayed through the existing engine journal API and compared by semantic digest.
Stale public tokens must reject without mutating state. Trace retention is off.

Run the current executable, with the pinned toolchain runtime libraries available:

```sh
/path/to/current/mtg-sim smoke
/path/to/current/mtg-sim smoke --artifact /new/path/smoke.json
/path/to/current/mtg-sim smoke-replay /new/path/smoke.json
```

Build via the coordinated Cargo workflow (`cargo build --locked -p mtg-sim`);
do not mistake another worktree's cached binary for the current source. Focused
Rust tests are `cargo test --locked -p mtg-sim smoke::tests`. Standalone Cargo
commands must acquire the shared lock documented in
[the verification workflow](game-engine-verification-workflow.md).
The coordinator owns npm/check-runner integration.

The default writes only a small JSON result to stdout; failure returns nonzero.
The result includes an exact shell-quoted reproduction command with working
directory and runtime library path. After a successful artifact write it reads
that artifact with `smoke-replay`; an IO failure retains the original command
so the failed write can be reproduced without replacing existing evidence.
The optional artifact preserves both successful and failed probes, including
partial journals/checkpoints when ordinary errors occur. It uses create-new
semantics: existing files and symlinks are never replaced, and missing parent
directories are errors. A write failure remains a failure; it cannot mask a failed
probe as passed. Input and output artifacts are limited to 65,536 bytes. A process
panic or kill can prevent artifact creation; this is not a crash recorder.

Artifacts are explicitly **trusted host diagnostics, not player payloads**.
They contain raw journal identities, a version, a fixed scenario name, null seed
(no random setup), core build/toolchain settings, smoke/CLI source digests,
source-manifest/record digests, catalog/configuration digests, and three tiny
checkpoints. They do not include a source archive, binary, snapshots, hidden
libraries, a full trace, or a source-retention closure. Preserve the corresponding
source and executable separately for long-term reproduction.

Replay reruns the fixed probe and its ordinary engine journal replays, then
compares the saved identity, input journal, checkpoints and failure. It reports
one stable JSON pointer with bounded expected/actual values. Pathological long
field names are truncated and marked explicitly. Input and checkpoint
indices are zero-based; `after_inputs` counts accepted decisions. A different
build is reported as an identity mismatch before state comparisons. Saved
arbitrary journals are never executed: this tool only reproduces this fixed
scenario. A changed internal state digest does not reveal which internal state
field changed, and no universal mismatch locator is claimed. A saved failing
probe remains a failing command even if its failure reproduces exactly.

`runtime_us` measures time inside the CLI, including compilation of source data,
admission, public queries, semantic digest/replay work and optional artifact IO.
It excludes process startup and Cargo compilation. Measure wall-clock process
startup/run separately from Cargo build and from warm repeats. No arbitrary
latency threshold, full game completion, match profile, search, server, SDK,
release matrix or retention rebuild is part of this check.

## Focused verification and development cost (2026-11-09)

The initial implementation checkpoint is `552651e`; a subsequent small CLI
correction selects `smoke-replay FILE` after saving an artifact, retaining the
original command when writing fails. This correction changes reporting only.

Three focused Rust tests passed, with 131 simulator tests deliberately filtered
out. They cover source compilation, public inputs, state outcomes and replay;
first input/checkpoint/identity differences; strict/bounded artifact reads and
non-overwriting writes. The final test-bearing source run took 86.834 seconds
including recompilation; the tests themselves took 0.01 seconds. The earlier
implementation run took 90.104 seconds and also passed. These are separate source
runs, not a speed comparison. The six-line final CLI correction was verified by
building and rerunning process-level checks rather than repeating the optimized
Rust suite.

The first debug executable build took 19.604 seconds. It needed separate
unoptimized library outputs from the optimized test profile; the target directory
was reused, not deleted or cold-started. A subsequent no-change debug build took
0.0261 seconds. Simulator edits still invalidate the broad core build identity.
Thus the runtime probe is cheap once compiled, but it does not remove codegen cost.
All Cargo commands used the shared advisory lock, pinned Rust 1.99.0, the preserved
`perf-engine-verification-loop/target`, and `CARGO_INCREMENTAL=0`.

The process checks include successful smoke/write/replay and real nonzero exits
for mutated input, checkpoint, state digest, build identity, artifact version,
recorded failure, unknown fields, long diagnostic keys, malformed/oversized or
missing files, invalid CLI usage and missing output parents. Existing files and
symlink destinations remain byte-identical after rejected writes. Each output
was also checked below 16 KiB. These are targeted tooling checks; no demo, full
profile game, server, SDK, Clippy, release matrix or retention build was run.

Measurements are local developer feedback on an uncontrolled host, not a quiet
benchmark or a same-workload engine speedup. Process wall time includes startup;
reported internal runtime excludes it. Repeats are fresh processes with warm
filesystem caches, not iterations inside a single long-lived engine process.

The final CLI rebuild took **15.681 seconds**. All **24 process checks** passed
(8 successful executions, including repeats, and 16 expected failures). The first
process took 26.836 ms wall / 25.217 ms internal. Five warm-cache fresh-process
repeats had median **25.298 ms wall / 24.186 ms internal**. The saved artifact was
**2,037 bytes**. Final executable SHA256:
`5cbf94a32264d3d0794c35c45842c7165815c4857d0b41c770cdbc10a6d22f5a`;
core build identity:
`e57b70ee34b38ad6a02b9d624c9e257433ba815f544274e331f9d770e2934fe5`;
smoke source SHA256:
`56dbd123d9f2ef9c52e5404afdcdf40ff29dded000dc35bb999160340a7c66f7`.

Exact commands, exit statuses, timings, stdout/stderr, immutable positive/negative
artifacts and the process-check harness are retained outside the worktree at
`/home/josh/.local/share/mtg-ecosystem/engine-verification/smoke-platform-20261110T003902.327783Z/`.
Earlier CLI evidence remains in the sibling
`smoke-platform-20261110T003615.742355Z/` directory. These directories do not retain
an executable or offline source/dependency closure; the shared cache executable
may subsequently change. They are scoped development evidence, not release
retention bundles. Two environment launch attempts (missing Rust runtime library
path for rustfmt, unavailable `/usr/bin/time`) failed before verification;
corrected invocations produced the results above.
