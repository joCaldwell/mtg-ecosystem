# Agent development workflow

All engine development is performed by AI agents. Josh owns product direction and
choices that require his input; agents own implementation, routine decisions,
verification, Git checkpoints and coordination. Source and docs should stay clear
and easy to inspect: a successor agent starts with limited context and must be
able to reconstruct the work from the repository and evidence.

## Enter an existing task

1. Read root and package `AGENTS.md`, then the [current map](game-engine-current.md).
   Read only the invariant/contract sections relevant to the change. The historical
   ledger is supporting evidence, not required onboarding for every task.
2. Inspect `git status --short --branch`, HEAD, the assigned branch/worktree and
   existing changes. Verify the supplied base against actual Git state. While the
   engine is absent from main, dependent work starts from `feat/engine-plan` or a
   newer explicitly assigned engine base; a root checkout is not proof of currency.
3. Inspect the environment with the direct JSON doctor below. Reuse a healthy
   dependency installation. Run bootstrap only when dependencies are missing or
   broken, after checking that no other agent owns an installation in that tree.
4. Establish owned files and the current native-work owner with the coordinator
   when peers are active. Use an orchestrator-bound isolated worktree where
   available. Parallel agents must have distinct file ownership or coordinate
   the specific shared edit before writing it.

The agent can resolve ordinary repairs and implementation choices within the
user's scope. Ask Josh when intent or product semantics cannot be inferred, a
necessary destructive action lacks authorization/ownership, or access only he
can supply is required. Another agent's ownership confirmation is coordination,
not a reason to ask Josh to approve routine work. A failed check is evidence to
investigate, not permission to change an expected outcome.

## Commands and machine output

Run from repository root. These direct entry points avoid npm's display output:

| Purpose | Command / response |
| --- | --- |
| Inspect environment | `sh scripts/engine-env.sh node scripts/engine-doctor.ts --json` — versioned JSON with `healthy`, `checks`, optional `error`; exits 0/1/2 |
| Inspect verification plan | `sh scripts/engine-env.sh node scripts/check-engine.ts --plan` — JSON commands, coverage and omissions; no test execution |
| Source/tooling checks | `sh scripts/engine-env.sh node scripts/check-engine.ts --source` — streaming results, exit status, exact JSONL receipt path |
| Bounded default | `sh scripts/engine-env.sh node scripts/check-engine.ts` — shared lock/cache preflight, results and JSONL receipt |
| Small runtime probe | `sh scripts/engine-env.sh node scripts/run-engine-smoke.ts smoke --artifact generated/NEW.json` — create-new bounded artifact, runtime JSON and timing receipt |
| Replay fixed probe | `sh scripts/engine-env.sh node scripts/run-engine-smoke.ts smoke-replay FILE` — same-source identity checks and first recorded divergence |
| Cache inventory | `sh scripts/engine-env.sh python3 scripts/engine-cache.py status` — JSON, no writes/eviction |
| Retirement review | `python3 scripts/engine-worktrees.py --worktree ABSOLUTE_PATH` — dry-run JSON; see [lifecycle](game-engine-worktrees.md) |

Doctor JSON currently uses `format_version: 1`. Always inspect the process exit
status. A launcher failure can precede JSON output; diagnose its stderr. An
unhealthy doctor normally reports failed checks; configuration/argument failures
have an `error.code` instead. Advisory warnings return 0 and remain worth reading.
Do not treat a version probe, build-only result or `--plan` as an executed test.

Check and smoke runners produce streaming subprocess output, so their entire
stdout is not a single JSON document. Consume their timing JSONL for command exit,
source identity and coverage. Smoke's small artifact contains trusted diagnostic
data, not public player information. Allocate a new artifact name per intentional
capture; retain meaningful failures without repeatedly creating full bundles.
Existing artifacts are not overwrite targets. See [storage](game-engine-storage.md).

## Execute and verify

Choose the smallest checks that answer the actual edit. Use `--plan` when coverage
is uncertain, then run the chosen checks once on stable source. Inspect test counts
and omissions. After a failure, fix its cause and repeat affected checks; source
or documentation changes alone do not justify repeating an unrelated full gate.
For gameplay edits, behavioral assertions and replay/privacy negatives matter even
though the broader matrix is optional. See [verification](game-engine-verification-workflow.md).

Native work has one owner across worktrees and uses the shared advisory Cargo
lock. The standard runners own it internally. Do not change a measured worktree's
source while a check is running. Continue independent source work in another
owned checkout while the native slot is occupied. Use process-local environment
and cache selection; avoid unattended prompts, global tool configuration changes
or toolchain installation as an implicit repair.

Commit each coherent change after relevant focused checks and push meaningful
checkpoints and handoffs. Keep the branch recoverable even when a broader check
failed: label that failure and its unverified scope. Update the current contract
or command documentation when behavior changes; preserve historical report scope.
Do not add repetitive agent transcripts or transient task status to permanent
manuals. Small durable findings belong in docs; raw receipts belong in ignored or
external storage with references from the handoff.

## Hand off verifiable state

Send the successor/coordinator one concise handoff at a meaningful checkpoint.
Include these facts in the message (structured JSON is convenient, not mandatory):

```json
{
  "branch": "task branch",
  "worktree": "/absolute/task/checkout",
  "base_commit": "full base SHA",
  "head_commit": "full checkpoint SHA",
  "published": true,
  "owned_paths": ["task-owned paths"],
  "working_tree": "clean, or exact remaining modifications and their owners",
  "changes": ["concrete resulting behavior"],
  "checks": [{"command": "exact invocation", "result": "passed/failed/not-run", "counts": "actual tests", "source_commit": "SHA", "receipt": "/path/if-retained"}],
  "omitted_checks": ["explicitly unrun scope"],
  "native_slot": "released, or owner/process/lock and next handoff",
  "retained_artifacts": ["paths and identity/recovery references"],
  "remaining_work": [],
  "decisions_needed": []
}
```

Use actual values, not the illustrative placeholders. Publishing is confirmed by
push success and the intended remote ref; local tracking refs alone can be stale.
A successor verifies branch/head, dirty state, process ownership and receipt
identities before resuming. Chat assertions and test counts from a different
source do not become fresh acceptance evidence. If context is lost, reconstruct
from Git, relevant docs, receipts and the task owner before making assumptions.

Finish by reporting the pushed checkpoint, changed behavior, meaningful checks,
omissions and any real decision needed. Release the native slot and leave enough
information for the next agent to proceed. Retirement requires completed ownership,
published ancestry and preservation of local evidence; see the lifecycle helper.
