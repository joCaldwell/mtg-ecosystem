# Engine review and verification ownership

Josh changed this workflow on 2026-11-03 to shorten feedback and avoid repeated
successful end-to-end runs for one change. This applies to engine/server work;
other packages retain their own instructions.

## One reviewed freeze, one full gate

1. **Implementation:** run formatting, compilation/lints and focused tests for
   affected capabilities. Run cheap compiler/schema/contract negatives early.
   Commit the candidate and report exact focused results. Implementers do not
   run the full engine/demo/all-profile acceptance suite unless explicitly given
   verification ownership.
2. **Source review:** independently inspect semantics, scope, guards, source
   records and expected-result changes before expensive acceptance. Return
   blockers without launching the full gate. Fixes get focused checks and a
   scoped review; do not spend another full run to discover a cheap defect.
3. **Freeze:** identify the final reviewed code commit, required phases and one
   verification owner. The reviewer may own execution after approving source,
   or a dedicated verifier may do it. Reuse the implementation worktree's build
   cache when safe; do not create a fresh checkout merely to repeat compilation.
4. **Acceptance:** that owner runs the required full gate once on the reviewed
   freeze. Include SDK/release/batch/replay phases required by the actual change.
   A successful read-only review never triggers another full run. Combine
   independently reviewed compatible changes into one acceptance candidate when
   practical, then verify the combined source closure.
5. **Integration:** the parent checks evidence, source/executable identity and
   exact tree equality, then merges. Documentation-only changes do not invalidate
   compiled-source evidence. A code change invalidates affected phases; identify
   the dependency/reason before repeating a full suite. Failed/interrupted runs
   remain preserved and distinct from passed phases.

Focused tests do not replace final acceptance. This changes timing and ownership,
not workload, rules, proof, rejection or resource requirements. Preserve independent
semantic replay, all required profile seeds/pods, limits and exact outcomes.
Never reuse another build's pass merely because a profile or branch has the same
name. Reuse requires the same frozen compiled bytes and intact artifacts/guards.

## Parallel work and shared execution

Use separate worktrees/owned files for independent implementation and review.
Agents can think, audit, write and run cheap independent checks while a frozen
candidate is verified. Do not mutate the source used by an active verification.
Assign one expensive acceptance owner per integration candidate; agents must not
independently start duplicate full gates. Serialize Cargo compilation and heavy
acceptance until measured CPU/memory headroom justifies bounded overlap.
Coordinate through the primary task; a busy verifier is a reason to continue
independent source work, not to rerun its checks elsewhere.

For cooperating local threads, shared advisory lock paths are:

- `/home/josh/.local/share/mtg-ecosystem/engine-verification/cargo.lock`
- `/home/josh/.local/share/mtg-ecosystem/engine-verification/full-gate.lock`

Use `flock` for execution ownership and record lock waiting separately from child
execution. Every participant must honor the same locks. Do not change a workload
or proof limit to obtain concurrency. Parallel runtime jobs require measured
memory headroom; timings must identify concurrency and other host load.

## Verification-speed optimization loop

The separate optimization thread profiles a frozen baseline, makes a small
change, runs focused equivalence/performance checks, obtains independent source
review, and reports a candidate with wall/CPU/memory evidence. It repeats after
integration. One centrally owned full gate validates the final reviewed candidate;
implementation subagents do not each run it. Avoid speculative core rewrites.

First targets identified by the read-only runtime audit:

- Reuse the detailed replay already checked by `verify_replay` for interaction
  reporting, retaining all checkpoint/loss/terminal/digest checks and standalone
  explicit CLI replay.
- Serialize each immutable trace-run snapshot once instead of three times.
- Run cheap schema/contract preflight before slow runtime checks.
- Record child wall/user/system time, peak RSS, exit status, source/executable
  identity and verification coverage. Existing elapsed-only logs cannot distinguish
  CPU from I/O or other blocked time precisely.
- After measuring memory, consider longest-first batches, more runtime workers,
  and overlap of a verified prebuilt release CLI's Node checks with Rust tests.
  Preserve debug and release evidence; avoid concurrent Cargo builds.

Representative A/B checks come before a full matrix. Full correctness comparison
includes unchanged interaction/site summaries, manifests except build identity,
accepted inputs, winners/outcomes, state hashes, decision/query/rule-step counters,
historical references and all required independent replay results. Record gains
only after measuring them. Development wall-time measurements are not quiet P7
search-performance claims.
