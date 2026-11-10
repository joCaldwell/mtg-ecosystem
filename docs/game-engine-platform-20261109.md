# Engine development platform checkpoint

The platform work is integrated on `feat/engine-plan`. Verified code checkpoint:
`1e7424eb617c2006062f0a2bbd8a60f640e201d1` (tree
`4ef0d4ea759ec4ba935bb310a158f8e32b68f20f`). This report is a later documentation-only
checkpoint. No new mechanics or broader engine acceptance is claimed.

## Delivered workflow

- [Current engine map](game-engine-current.md), concise package manuals and
  indexed invariants/catalog instructions. Historical manuals and release
  reports retain their original evidence qualifications.
- [Bootstrap and doctor](game-engine-development.md): pinned existing tools,
  minimal locked npm dependencies, read-only diagnostics and process-local
  environment setup. No toolchain downloads or system modifications.
- [Small runtime smoke and replay](game-engine-smoke.md), exposed as
  `npm run engine:smoke` and `npm run engine:smoke:replay -- FILE`. Both share
  the check driver's cancellation, Cargo ownership, cache and timing controls.
- [Version/dependency/artifact guards and precise builds](game-engine-builds.md):
  current IR authority, staged generated/binary checks, dependency boundaries,
  narrower fixture watches and stable repository/toolchain cache selection.
- [Worktree lifecycle](game-engine-worktrees.md): explicit dry-run/retirement
  with live publication and local-data checks. No historical worktree removed;
  ignored evidence and unconfirmed ownership remain blockers.
- Selected Node tests now fail on empty, skipped, TODO-only or renamed selections.
  Direct Python suite execution avoids silently empty discovery results.

The default remains deliberately bounded. Smoke is explicit: first native codegen
can be expensive even when the scenario itself is tiny. Native cache checks run
under `cargo.lock` before work. Source-only checks do not take that lock or reject
an oversized cache. Cache admission defaults are 16 GiB allocated / 4 GiB free,
with explicit overrides; they do not continuously enforce a hard quota.

## Integrated validation

At the verified code checkpoint, `npm run check:engine` passed:

- 97 compiler + 12 contracts Rust library tests;
- 46 Node tooling/environment/negative tests + 1 archive/parser test;
- 38 Python storage/retention/worktree/structure/cache safety tests;
- dependency/index artifact guard, Rust formatting, workspace/all-target compile.

Focused strict TypeScript passed for the runners, environment doctor and their
tests. Doctor reported healthy with exact tools/dependencies. All 179 relative
links checked across current onboarding/topical entry documents resolved.

A final launcher correction also tolerates an installed rustup proxy whose pinned
toolchain is absent: source-only commands and doctor can still start without a
native cache identity. Its regression test and the full source-only gate passed
(48 Node tests including archive/parser, 38 Python tests); strict TypeScript passed
after a test environment type annotation correction. Native checks remain those
of `1e7424e`; this shell/test-only correction did not change engine source.

The first integrated attempt at `efa1ca3` correctly failed formatting in the new
simulator CLI dispatch before native compilation. Its receipt is preserved.
The formatting-only correction is `1e7424e`; the subsequent complete bounded
check passed. Native phase times were 11.136 s for all-target compilation and
26.422 s for compiler/contracts compilation plus tests. Total child phase time
was 41.149 s, including all guards. This includes recompilation after integration
and worktree/source changes; it is not a warm-loop or equal-workload speedup claim.
No repeat full gate was run merely to obtain a warmer number.

The integrated smoke command saved a 2,037-byte artifact and passed its two public
inputs / three checkpoints / deterministic replay. Internal scenario execution
was 34.366 ms; its Cargo-run phase was 17.837 s including debug recompilation.
The separate saved-artifact replay passed in a 0.056 s Cargo-run phase.
A deliberate zero cache budget propagated exit 1 with only the cache-preflight
phase recorded: no Cargo/smoke phase launched. Source inventories were unchanged
across both successful and deliberately rejected runs.

Tools: Node 26.11.1, Rust/Cargo 1.99.0 from the preserved user-scoped bundle;
`CARGO_INCREMENTAL=0`, explicit preserved `perf-engine-verification-loop/target`
and `ENGINE_CACHE_MAX_GIB=32`. The historical cache was already over 16 GiB;
no migration or deletion was performed. Host load was uncontrolled; receipts
record it. RSS measurements concern the largest single waited process, not the
aggregate process group. These are developer feedback checks, not quiet search
performance measurements.

Core/simulator/server full runtime suites, Clippy, full executable Node contracts,
SDK, demo, release/profile matrices and offline retention were deliberately not
run for final integration. The smoke owner's focused three tests and CLI-negative
checks are separately qualified in its report; they are not counted as a repeated
integrated Rust suite. Architecture refactoring runs in its own branch and is not
part of this checkpoint.

## Retained evidence and handoff

Logs, exact JSONL receipts, healthy doctor output, the smoke artifact and SHA-256
manifest (about 87 KB total, before manifest) are copied outside the worktree to:

```text
/home/josh/.local/share/mtg-ecosystem/engine-verification/platform-integration-1e7424e/
```

Successful run inventory SHA-256:
`92be808a2355934579baf8320a00dd87095571bdfb9debcdb0dd0a6a4094c0c2`.
The smoke core build identity is
`384b5cbbf0013b2468d7f44e18937fc31a5463ea945c5093c259ebf3bd3a66e2`.
The shared cache binary can later be replaced by another build; replay deliberately
rejects a different identity. The published code, toolchain, compile profile and
artifact identities establish the reconstruction inputs. No extra binary copy
was created for this small development probe.

All implementation threads pushed coherent checkpoints. The native slot was
released to the separately authorized architecture thread after integrated checks;
that thread has merged `1e7424e` into its isolated checkout. Main remains separate.
A dedicated reviewable main integration after architecture stabilizes is preferable
to changing the default branch during concurrent refactoring. No user decision
is required to use these completed platform improvements.
