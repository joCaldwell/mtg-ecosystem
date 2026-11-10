# Engine gardening — November 2026

Josh requested a substantial reduction in verification cost, even at the expense
of test coverage, plus bounded generated storage and more frequent commits and
pushes. There is no arbitrary runtime target. This supersedes the earlier
requirement to preserve every acceptance workload during optimization. Runtime
failures must still be reported honestly; simplifying tests does not authorize
silently changing rules or blessing broken expectations.

## Baseline and ownership

The implementation baseline is `9257d1b` on `feat/engine-plan`, not `main`.
That branch contains 237 commits absent from the current `main`; `main` also has
independent changes. Do not reset either branch or merge engine changes into
`main` as an incidental cleanup operation.

The initial audit found no uncommitted entries in 58 registered worktrees
(including the three newly created cleanup worktrees). The engine branch diff
from its merge base with `main` was 746 files and 1,907,876 added lines. Large
tracked scenario reports explain much of that line count. This is distinct from
untracked build output and from unpublished commits.

Three isolated implementation branches start from that baseline:

- `cleanup/engine-fast-checks`: verification commands, check selection, measured
  feedback time, and the development/acceptance workflow.
- `cleanup/engine-artifact-storage`: generated output, reproducible cache cleanup,
  failure cleanup, inventory, and retention limits.
- `cleanup/engine-fixture-garden`: unused giant fixtures, their consumers, and
  focused source/test organization improvements.

The coordinating thread owns repository hygiene and integration onto
`feat/engine-plan`. Changes to shared files are kept small and reconciled during
integration. Only the verification thread runs expensive checks initially;
other threads use focused lightweight checks and coordinate Cargo execution.

## Commit and push cadence

Commit each coherent change after its relevant focused checks and push after
each meaningful checkpoint, before a handoff, and before ending a session.
Do not delay backup until the full acceptance matrix passes. An incomplete
checkpoint may be pushed when clearly labeled, with failed and unrun checks
recorded; pushing is not an acceptance claim.

Stage explicit task-owned paths. Do not add bulk caches, generated reports,
build output, secrets, or unrelated edits. Never force-push shared branches.
Check the upstream and remote result rather than assuming a local commit is
backed up. A branch being merged locally does not establish remote backup.

The initial integration baseline has been pushed as `origin/feat/engine-plan`.
Other engine branches with unique unpublished commits are preserved under their
original branch names; their publication does not integrate or approve them.
Historical worktrees and local branches are not deleted merely because they are
old. Confirm unique work is backed up and current execution has stopped first.

## Storage findings and cleanup boundaries

Initial allocated-size inventory across registered worktrees:

- Cargo target directories: 82,151,342,080 bytes.
- Generated directories: 68,311,638,016 bytes.
- Root Scryfall cache: 1,664,405,504 bytes.
- External engine evidence: 107,273,039,872 bytes.

These categories have different lifetimes. Reproducible compiler intermediates
can be reclaimed after checking active use. Downloaded source, unique retained
executables, and historical verification evidence require an explicit retention
decision. An ignored directory is not automatically disposable. Do not run
blanket `git clean -fdx` or delete every `generated` directory.

Preserve the performance worktree's build cache while it is used for the
verification changes. The storage thread owns exact cleanup paths, activity
checks, dry runs, and measured reclamation. Keep source and evidence manifests
small; bulk reports remain outside Git.

## Completion evidence

Each implementation reports its pushed commit, meaningful checks, omitted
coverage, and measured effect. Use comparable measurements where possible and
state cache/build qualifications. Integrate compatible changes and run the new
appropriate checks once; do not reintroduce the old full matrix as an automatic
requirement for this cleanup. The detailed command policy belongs in
`game-engine-verification-workflow.md`.
