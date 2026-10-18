# Engine performance diagnostics

These Linux host adapters run explicit ignored Rust diagnostics. They do not
build the engine, widen its admitted profile, or establish product search
acceptance. The deterministic production core has no dependency on these tools.

Use the repository's pinned Rust toolchain and an optimized release build with
overflow checks enabled. Capture actual Cargo compiler-artifact JSON with
`cargo test --locked --release --target x86_64-unknown-linux-gnu --no-run
--message-format=json`. Keep the exact executable hashes, compiler settings,
`mtg-sim native-source-manifest` output and complete current source inventory.
An authority must bind those actual inputs, contracts and host-script hashes.
Review its contents before supplying its SHA to an execution command. Each
wrapper documents its additional authority fields in its source guard.

The shared guard recomputes the compiled Build identity from current source
bytes and the historically hashed settings. Cargo's actual artifact separately
proves overflow checks; that setting is not included in the historical Build
hash. A successful build alone does not authorize a measurement against stale
sources or a different executable.

Run every test script explicitly; Python discovery omits the hyphenated names:

```sh
python3 scripts/engine-performance/test_measurement_host.py
python3 scripts/engine-performance/test_kernel_probe.py
python3 scripts/engine-performance/test-ordinary-driver-v1.py
python3 scripts/engine-performance/test-omniscient-driver-v1.py
python3 scripts/engine-performance/test-dense-driver-v1.py
python3 scripts/engine-performance/test-core-profile-v1.py
python3 scripts/engine-performance/test-allocation-count-v1.py
python3 scripts/engine-performance/test-legal-branch-v1.py
python3 scripts/engine-performance/test-persistent-work-v1.py
python3 scripts/engine-performance/test-current-pods-v1.py
python3 scripts/engine-performance/test-host-rust-allocation-phase-v1.py
```

These tests use small fixtures and owned tiny Python children. Actual
enforcement also needs the separately prepared `probe-host-enforcement.py`
success and timeout runs against a working user-systemd/cgroup-v2 service.

The measurement wrappers require an explicit execution option, an approved
authority hash and a fresh output directory beneath a real private (0700)
parent. `run-fixed-work.py` first prepares an immutable plan and requires its
hash for execution. Other entry points are `run-ordinary-diagnostic-v1.py`,
`run-dense-diagnostic-v1.py` and `run-omniscient-diagnostic-v1.py`; use `--help`
for their exact arguments. Keep raw native output, resource reports, partial
attempts and independent audits together outside Git.

The shared host verifies actual CPU affinity, a 2 GiB cgroup charged-memory
limit, zero swap, 128 tasks and a 120-second lifecycle deadline. It independently
checks process RSS after exit, retains failures, and closes only its own unique
service. Charged memory and process RSS are different metrics. A completed
native test can still fail the resource envelope or semantic report audit.

`run-legal-branch-v1.py` measures six genuine admitted roots (ordinary and
naturally developed dense boards with four, three and two living seats), in
separate selected-detail and every-branch-first-write sweeps. Retained handles
are counted separately from completed, fully verified mutations. Keep setup
history, each partial stage, and explicit resource shortfalls; a synthetic board
cannot replace a failed legal root.

`run-persistent-work-v1.py` retains workers across one warmup and five measured
generations at each declared physical-worker count. It separates pool startup,
steady execution and shutdown, and requires exact same-build trial semantics.
`run-current-pods-v1.py` runs the released eighteen complete pod configurations
with their existing policies, ceilings, detailed trace and checkpoints. A failed
warmup or incomplete pod supplies no complete-game throughput denominator.

These simulator wrappers require the actual Cargo feature list to be empty.
`host-run-rust-allocation-phase-v1.py` instead requires a separately identified
test executable built with `--features diagnostic-allocations`, plus a pinned
feature-off ordinary reference and a bounded known-vector run of that same
instrumented executable. The Build hash does not identify Cargo features:
retain the actual Cargo artifacts and executable hashes for both variants.
The feature is off by default, and the allocator is installed only in test
builds. Checked raw counter differences cover Rust allocation requests inside
the unchanged ordinary-core interval, including reallocations and deallocations;
they are not live-heap measurements or production-allocator timing. Zero
controls are required and are never subtracted.

`run-core-profile-v1.py` collects CPU samples and heap traces in separate
processes. Keep every collector warning; successful collection does not prove
complete attribution. `run-allocation-count-v1.py` provides a lighter, direct
GNU libc allocation-API capture for its declared dense workload. Those counts
include the entire native harness and its correctness proofs. They are not
isolated core counts or complete Rust allocations. An unreliable tool summary
must not replace the independently recorded process and cgroup memory metrics.

Fixed-work speedups require identical complete candidate/world trials, counts,
outcomes, utility vectors and State hashes. Dense synthetic boards, pure
modifier evaluators, repeated observations and repeated actual-world rollouts
retain their declared scope. They are not legal-pod, network, strategic-strength
or independent-hidden-world results. See
[the measurement report](../../docs/game-engine-performance.md) and
[the full performance plan](../../docs/game-engine-plan.md#6-performance-and-search-plan).


Definition diversity and shrinking-choice diagnostics use
`run-definition-diversity-v1.py` and `run-shrinking-choices-v1.py`. Their paired
`test-definition-diversity-v1.py` and `test-shrinking-choices-v1.py` scripts contain
49 pure validator guards. Real runs require an explicitly supplied authority
SHA binding the actual compiled native inventory, Cargo artifact, executable,
all host sources and the exact workload. Preparation never starts a build or
benchmark. Both drivers preserve the shared 120-second, 2 GiB charged-memory
and independent RSS, swap-zero and one-CPU guards through audit and cleanup.

The definition workload retains all 48 rows for synthetic payload counts
0/8/64/256 and four source-backed interaction controls. The shrinking-choice
workload retains all 105 attack/Edict cells, one warmup and five repetitions
(630 measured-driver executions), plus every declared native reference and
budget/replay oracle. Synthetic payloads are not authentic card definitions;
a partial matrix produces no complete-work throughput claim. Current actual
results and remaining resource gaps are in `docs/game-engine-performance.md`.
