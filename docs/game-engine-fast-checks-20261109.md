# Bounded engine development checks — 2026-11-09

`check:engine` now runs a deliberately smaller development workload. The corrected
candidate passes 107 compiler/contracts library tests, 31 Node tests, formatting
and all-target workspace compilation. One warm run took **3.058 seconds overall**;
its measured child phases totaled 2.694 seconds. This replaces the former default
workflow, not its correctness coverage or a same-workload engine implementation.

Implementation checkpoint: `c958faf7badd43a8a3a9c6a9aa2a3169da87227c`.
The bounded check exposed two stale future-IR negatives. Coordinator correction
`9966026`, cherry-picked as `c6b4b155e5ebc11241b078bf0d227e4da7f590bf`, changes only
the unsupported test value 25 to 26; all lower-version and runtime guards remain.
`Catalog::validate` and the schema already admit IR25. No failure was skipped.

## Coverage and repeat-cost audit

The historical check ran format, Node preflight, Clippy, all workspace Rust tests,
then all Node tests. The latter repeated the cheap source/schema cohort and ran
one memoized full demo (it was not rerun separately per Node test). Existing
historical evidence at `394301d` shows its test binaries were warm: Cargo reported
0.09s compilation. Expensive tests, not just compilation, dominated the old path.

| Measured child phase | Historical default, 394301d | New default, corrected first run | New default, warm repeat |
| --- | ---: | ---: | ---: |
| Formatting | 0.927s | 0.807s | 0.800s |
| Contract preflight | 8.953s | omitted | omitted |
| Clippy | 15.459s | omitted | omitted |
| Full Rust tests | 1,317.073s | omitted | omitted |
| Full Node/demo tests | 679.657s | omitted | omitted |
| Tooling behavior/negative tests | included above | 1.057s | 1.044s |
| Archived-source/parser agreement | included above | 0.470s | 0.494s |
| All-target Rust compilation | included in broader work | 8.536s | 0.054s |
| Compiler/contracts library tests | included in full Rust | 16.024s | 0.301s |
| Sum of child phase wall time | 2,022.069s | 26.894s | 2.694s |

Historical whole invocation: 2,024.611s; new warm whole invocation: 3.058s.
The corrected first run includes 15.72s test compilation; it is **not a cold build**:
dependency artifacts were reused. The warm repeat exists specifically to measure
cache qualification; subsequent documentation-only changes do not justify another
Rust run. Largest single-process RSS was 13,404,172,288 bytes for the historical
whole check and 246,534,144 bytes for the new warm whole check. These are not
simultaneous process-tree memory peaks.

Default coverage sacrificed explicitly:

- All core, simulator and server runtime unit/integration tests; compiler/contracts
  integration tests and Rust doc tests. Their Rust targets are compiled where
  `cargo check --all-targets` covers them, not executed.
- Clippy and Node executable schema/catalog/projection contracts, including the
  demo's historical scenario, checkpoint, restore and semantic replay assertions.
- Separate release test runs, SDK tests/typechecking, profile/seed matrices,
  explicit CLI replays, offline retention and ignored diagnostics remain optional.
  These were also additional work beyond the old `check:engine` command itself.

Cheap source/parser behavior, compiler lowering/strict-data negatives, subprocess
error propagation, cancellation, advisory locking and retention-host safety tests
remain in the default. `--contracts` offers executable schema/catalog negatives;
`--full` retains the former broad Rust/Node workload but removes the duplicated
preflight. Neither deeper mode was executed for this gardening change. The two
suggested simulator source-collector tests for fixture deletion were not linked/run;
the fixture thread's source audit and archive/parser check cover that small change.
See [workflow commands and ownership](game-engine-verification-workflow.md).

## Machine, source, cache and failure qualifications

Same physical local host, AMD Ryzen 9 9950X, 32 logical CPUs. Both retained and new
runs use Node v26.11.1 and rustc 1.99.0 (b940084d7, Arch package 1:1.99.0-3).
Historical source is 394301d, before the later IR25 work; new source is c6b4b15 plus
uncommitted workflow/package-manual documentation edits. The two successful new
runs share inventory SHA256
`9ac79c33a42fdc3fc985d96cbc6ae3099c38f6b369ffc143f325009a3ded596e`
and both recorded unchanged source before/after. The report itself was written
afterward. This is an honest before/after **workflow** observation, not equivalent
source, equivalent coverage, a statistical benchmark or a quiet search claim.

Reused target: `/home/josh/.t3/worktrees/mtg-ecosystem/perf-engine-verification-loop/target`.
Storage ownership was notified; no cache or retained evidence was deleted here.
New checks use `CARGO_INCREMENTAL=0`; first native run rebuilt affected local
artifacts under that setting. Historical incremental setting was not recorded.
New phases are serialized under the shared advisory Cargo lock; internal Cargo
concurrency is its default. Host activity was uncontrolled: historical starting
phase load was [8.99, 5.04, 2.26], corrected [1.23, 1.42, 1.12], warm
[1.82, 1.56, 1.18]. Lock wait was negligible, recorded separately.

Preserved failures: system rustc 1.99.1 was rejected before running phases; the
first extracted-toolchain attempt lacked `LD_LIBRARY_PATH` and also rejected.
With the correct pinned environment, the first default run returned 101 after
95 compiler tests passed and two stale IR25 negatives failed. It took 38.672s in
child phases, including 25.84s test compilation; contracts tests were not reached.
After the two-line test correction, all 97 compiler and 10 contracts tests passed.
Focused TypeScript checking of all three changed scripts and 9 driver tests pass.
The 31 Node total includes those driver tests and one archive/parser test.

## Retained evidence locators

Historical directory (unchanged):
`/home/josh/.t3/worktrees/mtg-ecosystem/perf-engine-verification-loop/generated/final-engine-verification/d2cd-lifetime-394301d-20261104T014230Z/`.
`check-engine-timing.jsonl` SHA256
`1cb78067668ecd4889a3dbd4f790be042214884ec509329f282573a4cb456510`;
whole `check-engine.json` SHA256
`d1278ba3198bcc13bb7c1902c575233d94d1768d53cb0b04fe9575ef9f6dff65`.

New worktree:
`/home/josh/.t3/worktrees/mtg-ecosystem/cleanup-engine-fast-che-f4c36440f463ef9e/`.
Ignored evidence under `generated/engine-fast-checks-20261109/` contains logs,
the whole warm measurement and a small timing audit with exact input hashes.
Driver receipts under `generated/engine-check-timing/`:

- `2026-11-10T00-07-08.563Z-64860.jsonl`: original failed default.
- `2026-11-10T00-09-29.173Z-67910.jsonl`: corrected default with recompilation.
- `2026-11-10T00-10-01.679Z-69684.jsonl`: warm default, same inventory.

No full gate, profile games, explicit CLI replay, release test matrix or retained
rebuild was launched. No new broad engine acceptance claim is made.
