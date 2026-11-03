# Reviewed verification optimizations, 2026-11-03

Optimization freeze `8eb303f8f006b0ea03fb7e0fb3aeab8144e65f22`, tree
`89680598238a7661c0ebb4306c3d3c845c25cca9`, passed its one designated acceptance
run and independent parent evidence audit. This applies to that freeze; the later
source-integrated D2c-c candidate still needs combined acceptance.

Successful reporting reuses the independently checked Detailed Game. Every
checkpoint, loss, survivor, terminal and digest guard remains; failed/nonterminal
diagnostics and standalone CLI replay remain independent. Each immutable trace
execution retains one snapshot/digest. The driver adds cheap contract preflight,
command wall/CPU/RSS evidence, cancellation ownership and shared Cargo locking.
Normal checks remain complete. Proxy invocation and cancellation-fixture defects
found in source review were fixed and independently reviewed before acceptance.

## Accepted results

- 979 Rust tests passed; 20 diagnostics ignored.
- 65 normal Node checks and 24 repeated preflight cases passed.
- 134 focused release checks passed; two opt-in timing diagnostics ignored.
- Production simulator/server builds and 47 SDK tests passed, with strict
  TypeScript and all 33 response variants.
- Twelve unchanged profiles produced 216 winners, zero faults/truncations,
  and 216 passing explicit retained-CLI replays.
- Complete reports, interaction/site summaries, configurations, journals,
  checkpoints, losses, outcomes, hashes and deterministic counters equal sealed
  D2c-b. Exceptions are exactly 456 named timing/build fields and the leading
  build ID in 48 checkpoint strings; every remaining checkpoint byte is equal.
- All 73 baseline files and 173 historical hashes remain unchanged. Coverage
  JSON/TSV is byte-identical: 1,929/32,116, zero additions or losses.

The parent independently hashed all 1,560 sealed artifacts (4,407,770,267 bytes),
checked 1,336 Git archive files against committed blobs, verified all 556 compiled
native inputs/archive members, reconstructed engine/server identities and checked
retained executable introspection. It separately repeated complete comparisons
for all 216 games/reports and checked every explicit replay output. Its initial
log parser assumed TAP for the main Node reporter; the corrected read-only audit
passed. No acceptance tests were repeated for that audit-only correction.

## Retained identity

Immutable evidence directory:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/optimization-8eb303f8-20261103T224406Z/`

| Artifact | SHA256 / identity |
| --- | --- |
| Evidence seal | `73ef7e6aa3a388183d82b620dc5c8313ab448102f4e58b5f14afd1d770edf0e7` |
| Engine build | `687d0915833f8136a421a907440537ec78cd9de44f33692b93f7a5ef41c0cac4` |
| Server build | `45253724cfe42c4365fbead0f23ebead8edb87e37c7cc6287da671a685a48490` |
| Simulator executable | `e6e25a7bcc9c8ac4f8ec1441ea5db2236b72624960543063bba719d0ba6c9414` |
| Server executable | `7ddfa71045aee8c36804d13e40a36f0743220a4eff9421681271c5f15a4fc955` |
| Native source archive | `15c233600209e2553111c378d838d2faf50bdc3811b4197c53349662adcd1c42` |
| Full Git source archive | `379bf61a3a7d9a40c43b885c4699bd7a230b26ea1635550c33d602ec9fde9f1b` |

Original logs, command/driver pins, source inventories, baseline pins, exact
comparison proof and SDK output are sealed. Parent audit/script are retained in
the primary worktree under `generated/final-engine-verification/resumed-20261103/`
as `optimization-parent-audit.json` and `audit-optimization-seal.py`. Sealed evidence
remains unchanged. SDK setup omitted `--ignore-scripts`; the locked dependency set
has no install hooks, and source/lockfiles remained exact.

## Observed runtime

| Phase | Earlier D2c-b run | Optimization run |
| --- | ---: | ---: |
| Engine check | 37m19.5s | 36m04.3s |
| Release/build/SDK/identity/coverage and orchestration | 2m55.4s | 2m49.4s |
| Twelve batches + 216 explicit replays | 44m11.1s | 34m12.4s |
| Successful execution total | 84m26.0s | 73m06.1s |

These are separate shared-host observations with build/cache differences, not a
controlled speedup measurement. Both profile matrices use the same workloads and
two workers. Per-command wall/user/system time, host load, lock waiting and
largest-single-process RSS are recorded; that RSS is not a simultaneous group
peak. No quiet P7 claim is made. The next receipt-reuse candidate remains separate
until its focused production-equivalence probe and source review complete; it
was not included in this accepted freeze.
