# Engine development verification

Josh's 2026-11-09 preference is substantially faster development verification,
including explicitly reduced coverage. This supersedes the 2026-11-03 requirement
for one mandatory full acceptance matrix. It applies to engine/server work and
related parser tooling; other packages retain their own instructions. Historical
release reports remain evidence for their named source, not future gate mandates.

## Choose the work that answers the change

Run from the repository root. `npm run check:engine -- --plan` prints the selected
commands and omissions without taking locks or running tests. The npm launcher
selects the existing pinned environment; direct `node scripts/check-engine.ts --plan`
can inspect plans without that launcher.
Every executed mode prints its coverage and writes those omissions into its timing
report; a successful fast check is not a full-engine acceptance result.

| Command | Runs | Deliberately omits |
| --- | --- | --- |
| `npm run check:engine` | Tooling behavior and failure tests; archived-source/parser agreement; Rust formatting; `cargo check --workspace --all-targets --locked`; contracts/compiler library unit tests | Core/simulator/server runtime tests, Clippy, executable schema/catalog Node checks, demo, SDK, release tests, profile matrices, explicit CLI replays, retention |
| `npm run check:engine:source` | Tooling tests and archived-source/parser agreement, without Cargo or its lock | All Rust work and executable contracts/runtime work |
| `npm run check:engine:contracts` | Tooling plus existing source/schema/catalog Node preflight and downgrade/invalid-shape negatives | Runtime-backed Node contracts/demo and Rust unit tests/lints; may **compile release mtg-sim** on a cold cache |
| `npm run check:engine:full` | Former check-engine workload: formatting, Clippy, workspace Rust tests, tooling and complete Node contracts/demo | SDK, ignored diagnostics, separate release tests, profile matrix, explicit CLI replays and offline retention |

`--fast`, `--source`, `--contracts` and `--full` are mutually exclusive. The full
mode runs the cheap contract cohort once, removing its former duplicate early
preflight. Source and fast use one exact named archive/parser test; keep that
selection synchronized if the test is renamed. Rust test optimization remains
level 2 with debug assertions and overflow checks; production guards are unchanged.
Compiler/contracts integration tests and Rust doc tests are also omitted from
the default. The compiler/contracts library tests exercise lowering and strict data validation,
but do not establish that games, host behavior or replay work.

The selected Node test wrapper fails if no runnable test matched, including
empty/skipped/TODO-only selections. Every mode also runs environment, cache,
worktree and dependency/artifact guards. Python suites execute their files
directly; unittest discovery can silently skip hyphenated filenames.

`npm run engine:smoke` separately compiles and runs the tiny public-action/replay
probe; add `-- --artifact generated/NEW.json` to save a bounded reproduction.
`npm run engine:smoke:replay -- FILE` checks it against the same source/build.
Both commands own the Cargo lock and cache preflight. A first debug build can be
expensive even though warm scenario execution takes milliseconds. See
[the smoke contract](game-engine-smoke.md); this is not a full gameplay gate.

Every mode includes the Python storage/retention safety suites and the Node
executable-preservation checks. These use small temporary fixtures and fake
workers, not a real offline engine rebuild. Python 3 and Linux process inspection
are required for these tooling checks.

For a gameplay change, add the smallest relevant Rust test filter, for example
`cargo test --locked -p mtg-core FILTER`, under the shared Cargo lock. For parser
changes, add the affected parser tests. Use executable contracts when IR/schema
changes need cross-language coverage. Run SDK checks when the public API changes.
Use release, full demo, a selected profile/seed, independent replay or offline
reconstruction only when the actual change or an explicitly requested release
claim warrants them. No default obligation exists to run twelve profiles, all
216 games and explicit replays, duplicate debug/release suites, historical golden
matrices, or fresh offline rebuilds. State which obligations you did not exercise.
Do not silently remove runtime correctness guards to make a selected test pass.

Select new behavior and relevant negative cases first, then the smallest adjacent
regressions that could be affected. A complete neighboring module is not the
default when only a few cases exercise the changed path. Check that each selector
actually executes tests before treating it as evidence. After a test-only repair,
rerun the affected tests and required compilation, rather than restarting every
passed phase. Preserve earlier receipts and explain which unchanged inputs allow
reuse. Review test assumptions against existing source/contracts before spending
another native pass on a guessed diagnostic or unsupported baseline case.

Review and integration need focused evidence proportionate to the change, not a
mandatory full gate. Read-only review and documentation edits do not invalidate
compiled-source checks. After a code change, repeat affected checks with the reason
recorded; do not rerun passed unrelated work. Preserve exact failed/interrupted
results and distinguish an unrun check from a pass. Avoid huge generated reports.

## Git checkpoint cadence

Commit each coherent change after relevant focused checks; push immediately after
each meaningful checkpoint and before a handoff/end of session. Do not hold commits
or pushes for the full acceptance matrix. WIP checkpoints are allowed if labeled
clearly and exact failures/unrun checks are recorded; make no claim of acceptance.
Stage explicit task-owned paths, never blanket generated artifacts; never
force-push shared branches. See [the gardening coordination record](game-engine-gardening.md)
for the current integration scope and Git audit.

## Shared Cargo execution and caches

Use separate worktrees and owned files for independent work. One designated
verifier coordinates expensive Cargo commands; other threads can inspect source
and run `check:engine:source`. Honor these shared advisory locks:

- `/home/josh/.local/share/mtg-ecosystem/engine-verification/cargo.lock`
- `/home/josh/.local/share/mtg-ecosystem/engine-verification/full-gate.lock` when an
  explicitly chosen multi-phase acceptance needs one owner.

The runner acquires `cargo.lock` for fast/contracts/full, including nested Node
Cargo calls. **Do not wrap it in a second acquisition of the same lock.** Wrap
standalone Cargo commands with `flock` on that path. Record waiting separately
from execution. Never edit the source inventory being measured. A busy verifier
is a reason to continue independent work, not start a duplicate build elsewhere.

The runner defaults `CARGO_INCREMENTAL=0` to avoid repeated multi-GiB incremental
cache growth. `CARGO_INCREMENTAL=1` explicitly opts in; this trades disk for
potentially faster Rust edits. Ordinary Cargo dependency/final outputs still cache.
The effective setting and `CARGO_TARGET_DIR` are recorded. The runner never deletes
an existing cache. Inspect active processes and coordinate with storage ownership
before reusing or removing a target directory. Reuse is a compilation aid, not
permission to attribute another source's test result to the current source.

Native modes select the same repository/toolchain cache used by the environment
launcher and check its budget under the Cargo lock before executing phases.
Defaults are 16 GiB allocated cache and 4 GiB free reserve; explicit
`ENGINE_CACHE_MAX_GIB`/`ENGINE_CACHE_MIN_FREE_GIB` override these preflight limits.
They are admission checks, not a monitored runtime quota. Historical performance
caches exceed the default: this platform integration uses the preserved target
with an explicit 32 GiB budget. No automatic migration or eviction occurs.

Rust phases require the pinned 1.99.0 toolchain. On this host a preserved copy is
available at `/home/josh/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1`:
prepend its `usr/bin` to `PATH` and its `usr/lib` to `LD_LIBRARY_PATH`. The system
Rust can differ; rejection is an environment failure, not a test pass. Source-only
mode needs Node/dependencies but deliberately does not require Rust.

## Measurement and coverage reporting

The driver records source inventories before/after, executable/driver identities,
phase commands, child wall/user/system time, largest single-process RSS, lock wait,
exit/cancellation status, host load, selected mode and omitted coverage. Use
`MTG_ENGINE_TIMING_FILE` for an explicit new JSONL path (existing files are never
overwritten). A source change during execution fails the run.

Compare like workloads/cache states where practical; if a full baseline is too
costly, use retained timings and label the different source, cache and coverage.
Distinguish first run/recompilation from warm repeat. Reduced work is the intended
benefit here; never call it a same-workload engine speedup. CPU is command plus
waited-for descendants; RSS is the largest individual process, not aggregate group
peak. Other host activity is uncontrolled and recorded as such. These are developer
feedback measurements, not quiet search-performance claims. See
[the fast-check measurement report](game-engine-fast-checks-20261109.md).
