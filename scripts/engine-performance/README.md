# Engine performance diagnostics

These Linux host adapters run explicit ignored Rust diagnostics. They do not
build the engine, widen its admitted profile, or establish product search
acceptance. The deterministic production core has no dependency on them.

Use the pinned Rust toolchain and an optimized release build with overflow
checks enabled. Capture Cargo compiler-artifact JSON with `cargo test --locked
--release --target x86_64-unknown-linux-gnu --no-run --message-format=json`.
An authority must bind the actual executable hashes, compiler settings,
`mtg-sim native-source-manifest` output, source inventory, contracts and
host-script hashes; review it before supplying its SHA to an execution command.
The shared guard recomputes the Build identity from current source; Cargo's
artifact separately proves overflow checks, which the Build hash excludes. A
successful build alone authorizes no measurement against stale sources.

Python discovery omits hyphenated test names, so run each test script explicitly
by filename, for example `python3 scripts/engine-performance/test-final-hosts-v1.py -v`.
Tests use small fixtures and mocked children; actual enforcement also needs
`probe-host-enforcement.py` success and timeout runs on user-systemd/cgroup v2.

Measurement wrappers (`run-*.py`, `host-run-*.py`, `review-*.py`) require an
explicit execution option, an approved authority hash and a fresh output
directory beneath a private (0700) parent; `--help` and each source guard list
exact arguments and authority fields. The shared host enforces CPU affinity, a
2 GiB cgroup charged-memory limit, zero swap, 128 tasks and a 120-second
deadline, checks process RSS independently, and retains failures. A completed
native test can still fail the resource envelope or report audit. Keep raw
output, partial attempts and audits together outside Git.

Simulator wrappers require an empty Cargo feature list.
`host-run-rust-allocation-phase-v1.py` instead requires a separately identified
executable built with `--features diagnostic-allocations`, plus a feature-off
reference; retain both variants' artifacts because the Build hash omits
features. Its counts are allocation requests, not live heap or production timing.

`run-final-pod-series-v1.py` runs the unchanged eighteen-pod batch once as warmup
and ten times measured, stopping at the first failure. `run-final-search-primary-v1.py`
and `review-final-search-primary-v1.py` bind a current release to the original
sixty-job quiet measurement and its independent semantic replays; there is no
overwrite, resume or retry. Guard success alone makes no performance claim. See
[engine evidence](../../docs/game-engine-evidence.md#search-hosting-and-performance)
and [search workload](../../docs/game-engine-search.md#sampler-roots-and-outcome-accounting).
