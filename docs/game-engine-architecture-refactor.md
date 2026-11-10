# Engine architecture refactor

This work was reconciled against local `feat/engine-plan` at `364cbdc`, then
integrated with platform head `3cd24f3`. The original review used an older
`main`; recommendations already implemented on the engine branch were preserved.

## What changed

| Area | Result |
| --- | --- |
| State ownership | Separate rules, player knowledge and audit groups. Input and concession journals share immutable history entries across forks. |
| Transactions | One enclosing rule-step transaction owns ordinary batch rollback; direct batches still stage independently. Simultaneous validation and trigger before-state captures remain. |
| Event conflicts | Accumulated read/write sets replace pairwise footprint comparisons. Typed faults survive failed prevalidation. |
| Queries | Zero-budget startup defers projection/request construction. Completed results move out of work instead of being cloned. |
| Lookups | Validated catalogs use binary search; player handles have a derived reverse index; visibility refresh uses membership sets. |
| API and errors | Viewer-bound `PlayerSession`; sticky execution faults; payment insufficiency distinguished from corrupt state; creature membership named separately from targeting legality. |
| Rules laboratory | Two bounded public-API explorations cover casting, payment, reversal, budget equivalence, forks, private hands and selected replay/restore checkpoints. |

The existing data-driven profiles, compiler admission, event/replacement
continuations, derived characteristics, persistent permitted histories and
prepared catalog sharing remain the foundation. This change adds no card
mechanic and expands no hosting/search capability set.

## Contracts and limits

- State ownership groups serialize as the original **flat** object. Snapshot19,
  bundle20 and public observation/context16 are unchanged. Full audit and
  permitted history remain bound by semantic hashes; no transcript is discarded.
- Query work sidecars advance **15 → 16**. Every query has one additional charged
  initialization stage. Old sidecars reject; published query results are unchanged.
  Initialization can still do work proportional to a header. Work units are
  deterministic accounting, not a wall-clock or allocation bound.
- `PlayerSession` is a native borrowing facade. The host authenticates its viewer
  and alone applies validated responses. Sidecars/receipts remain host objects.
- Native `Game::journal()` now borrows `History<AcceptedInput>`. Length, last-entry
  and iterator access never flatten the transcript; callers explicitly collect
  owned arrays when required. Simulator suffix scans use iterators.
- Successful steps retain existing behavior. Decision-query failure and rule-step
  overflow now persist a fault and invalidate outstanding work/receipts, like
  other execution failures. Invalid private payment state faults rather than
  masquerading as a normal failed payment.
- The exploration grammar is finite: one cast attempt, at most 12 responses,
  256 nodes and 512 resumptions. It is not exhaustive Magic validation.
- The lookup/conflict changes improve algorithmic work; no whole-game latency
  or memory benchmark is claimed here.

## Verification

Production checkpoint **`a5ea0be`** passed workspace all-target compilation and **169 focused
Rust checks**: 157 selected core tests, six search reconstruction tests, two finite
explorations, three catalog lookup tests and one compile-fail facade doctest.
The selected core suite includes a frozen flat-state oracle, the former nested
transaction mode, every-unit query reference comparisons, history hashing,
strict recovery, privacy and payment/turn behavior. The 4,096 footprint sequences
and full-capacity conflict case passed. Formatting and the structural guard pass.

The source gate also passed 48 tooling Node tests, one archive/parser Node check,
38 Python tests and the dependency/artifact guard. The explicit saved smoke and
its replay both passed (two accepted inputs, three recorded checkpoints), with
unchanged source inventories. Core build identity:
`6f124983bad351d1b50d8d3f8486d64d86cdfb912cd3297c983ee327f40cac16`.

Compilation initially found test aliases left behind by the field migration;
those were corrected. Review also caught accidental journal flattening in hot
callers; the final borrowing API and pointer-identity regression address it.
Raw logs, including development failures, are retained outside the checkout under
`~/.local/share/mtg-ecosystem/engine-verification/architecture-20261110/`.
The full core suite, server runtime, release/profile matrices, SDK and retention
checks were not run. No combined acceptance or new performance claim is implied.

## Integration audit

The coordinator merged `f4e224c` into the engine line at `7ddba6b`. The resulting
Git tree exactly matched the reviewed checkpoint
`fd62708ea2332389aff71adb55dfd21a77f5b535`; all 18 retained evidence files matched
their SHA-256 manifest, and source/smoke receipts recorded unchanged inventories.
Two separate read-only reviews found no blockers in state/rollback/fault behavior
or query/recovery/privacy/indexing. They did not execute Cargo or review every
mechanic. No fixture bytes changed relative to platform `3cd24f3`.

At Josh's request, the engine line is integrated with main's `4dfcb50`, preserving
both histories. Conflicts were limited to root/parser manuals; the resolution
keeps current parser instructions and the deck-builder public compatibility
policy alongside current engine/agent instructions. Deck-builder files and parser
source/tests match that main checkpoint byte-for-byte. Cargo inputs, engine Rust
and server files match `f4e224c` byte-for-byte, so the native checks above remain
qualified to that same source without an unnecessary rebuild.

The combined working inventory passed the source gate (48 tooling Node tests,
one archive/parser test and 38 Python tests, plus structural guard). This checks
the current main parser against retained engine source inputs. The integration
adds documentation and changes worktree-helper defaults to main; the former
engine-plan branch is retired. The historical engine checkout is preserved
detached because it contains referenced generated evidence and dependencies.
No new full-runtime, SDK, release/profile or retention acceptance is claimed.
