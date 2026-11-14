# Engine storage lifecycle

Storage classes have different owners. An ignored path is not permission to
remove it. In particular, historical `generated/final-engine-verification/`
contains source designs, phase logs and acceptance evidence referenced by docs.

| Class | Examples | Policy |
| --- | --- | --- |
| Source and reviewed fixtures | Git-tracked parser, Rust, tests and manifests | Preserve; change through reviewed commits. |
| Downloaded pinned inputs | `.scryfall-cache`, retained rules, source archives | Keep while referenced; a fresh download is not the same historical corpus. |
| Reproducible build caches | `target/**/incremental`, compiler intermediates | Reclaim only after establishing ownership; incremental caches are the first choice. |
| Temporary generation | In-flight retention captures and fresh rebuild directories | Run-scoped; remove on success, error and ordinary cancellation. |
| Release evidence | External `engine-evidence`, selected generated reports/binaries | Never age-delete automatically; promotion/replacement requires explicit review of references. |
| Toolchains | External `engine-toolchains` | Shared execution inputs; preserve while used by verification. |

## Inventory and bounded cache cleanup

From the repository root:

```sh
python3 scripts/engine-storage.py inventory
python3 scripts/engine-storage.py prune-cargo --worktree /absolute/worktree
# Review the JSON and establish ownership, then repeat with --apply.
python3 scripts/engine-storage.py prune-cargo --worktree /absolute/worktree --apply
```

Inventory includes all Git-registered worktrees and the standard external engine
stores. Symlinks are reported rather than followed. It does not traverse arbitrary
home directories or read environment/secret files. Allocated-byte rows de-duplicate
hard links within each row; sums across rows can double count shared inodes.

`prune-cargo` has a **2 GiB aggregate incremental-cache budget per selected
worktree**. Above that budget it proposes removing all that worktree's host and
explicit-target debug/release incremental directories; below it does nothing.
Use `--max-bytes 0` to propose every incremental directory. It never removes
`deps`, `build`, final executables, source, generated reports or retained evidence.
It does not invoke Cargo. This is an explicit maintenance command, not a daemon.
The development check runner disables new incremental output by default
(`CARGO_INCREMENTAL=0`, unless deliberately overridden).

Dry-run is the default. Apply checks exact Git worktree registration, tracked-file
exclusion, path containment, directory and ancestor symlinks, symlinks inside each
candidate, the existing shared advisory Cargo lock, and same-user Linux `/proc`
`cwd`, `exe` and open file descriptors. It refuses known active references.
If some processes cannot be inspected, apply also refuses unless
`--allow-uninspectable` explicitly acknowledges an independent ownership check.
The report lists those PIDs. Never use that switch to bypass an actual reference.
The lock is cooperative: stop other work or coordinate ownership before apply;
this does not protect against a noncooperating writer starting after the scan.

## Retention defaults

`npm run retain:engine` and `npm run retain:server` now run through
`scripts/run-retention.py`. The existing replay, hash, offline dependency and
server/client verification guards still execute in the workers. Storage supervision
adds no claim that those checks passed: a storage failure or worker failure exits
nonzero. This change was tested with small process/file fixtures, **not** a new
full retention rebuild or engine acceptance run. The simulator artifact-path
change is source-inspected here; the coordinator owns its integrated compile check.

The default bounds are **32 GiB scratch**, **128 GiB total evidence store**, and
**16 GiB free space** on both filesystems. The supervisor checks admission and
samples allocated usage every two seconds while the worker runs, stopping its
process group on overflow. These are practical monitored bounds, not filesystem
quotas: a fast writer can overshoot between samples, and a concurrent unrelated
writer can consume disk space. Scanning also has some overhead. Explicit overrides
are available when a reviewed capture needs more space:

```sh
npm run retain:engine -- /absolute/evidence-store --scratch-gib 48 --store-gib 160
npm run retain:server -- --scratch-gib 48
```

Server retention derives the evidence root from its verified predecessor pointer.
No old evidence is automatically deleted to make room. A full store refuses new
work until an owner reviews and removes/replaces specific retained runs, moves the
store with references updated, or deliberately raises its budget. Review pinned
source archives, rules, bundles, binaries, logs and downstream documentation before
retiring a run. Age alone is not a retention decision.

The supervisor holds the existing shared Cargo lock. It creates one fixed ignored
`generated/.retention-work` workspace for bundle capture, JSON stdout, Cargo homes
and fresh rebuild targets. The initial engine build also uses this
owned scratch target, overriding inherited `CARGO_TARGET_DIR` so a shared cache
cannot bypass the storage bound. The host-only `MTG_ENGINE_ARTIFACT_ROOT` override
routes demo batch captures and live journals to scratch as well; an invalid relative
value fails explicitly rather than spilling into the default directory. Outside
supervised retention, the simulator keeps its existing generated-directory default. `CARGO_INCREMENTAL=0` is the default unless the
caller deliberately overrides it. This matches the development runner policy.

The exact rebuilt executable is copied exclusively into retained `bin/mtg-sim`
or `bin/mtg-server`, checked by hash, and executed/verified from that retained
path before the usual evidence manifest is written. Source closure, vendor,
policy inputs, bundles, server/client evidence and failed retained output stay in
the evidence store. The fresh compiler intermediates and duplicate temporary
bundle captures are discarded. Temporary failure captures are not debug archives;
this trades some failure diagnostics for bounded disk use.

Success, ordinary errors, SIGINT and SIGTERM stop descendants and remove only the
supervisor's own scratch directory. A small latest storage-status file records the
outcome at `generated/retention-storage-latest.json`. SIGKILL, machine failure or
filesystem errors may leave scratch. A new run **refuses an existing scratch
workspace**, rather than adding another or guessing it is abandoned. Inspect its
`owner.json`, establish that the supervisor and descendants are stopped under the
Cargo lock, and review its contents before manually removing that exact directory.
No generated/evidence-tree sweep is part of this workflow. Directly invoking the
TypeScript worker is no longer a supported retention entry point.

Cheap lifecycle checks:

```sh
python3 scripts/test-engine-storage.py
python3 scripts/test-retention-workspace.py
node --test scripts/retention-workspace.test.ts scripts/retain-server-evidence.test.ts scripts/capture-json-output.test.ts scripts/retain-policy-sources.test.ts
```

Parser ingestion and archive publication already replace their named outputs
atomically and remove `.tmp` output in `finally` on ordinary failures. Their exact
content-addressed corpus/paper snapshots remain pinned inputs, not age-based
cache candidates. No grammar or archive wire format changed. Existing ignore
rules already cover generated output, targets, corpora and Python bytecode; no
broader ignores were added to conceal source changes.

## Worktree lifecycle

Start new independent work from current `origin/main`. The engine/platform and
architecture work are integrated there; `feat/engine-plan` is retired. In T3,
use the thread/worktree tool with `origin/main` and `startFromOrigin: true` for
independent work. Dependent unpublished work uses its explicitly assigned local
base and `startFromOrigin: false`. Record base SHA and owned files. Do not create
an unrelated shell worktree for a T3 thread. The retirement helper defaults to
local `main` and the live remote `origin/main` publication proof.

Commit a coherent change after its focused checks, then push the checkpoint.
Push before a handoff, a long native measurement, or ending a work session.
Report the exact commit, checks performed, omissions, active processes, and
retained evidence paths. A failed broader gate does not prevent a clearly marked
implementation checkpoint; never claim that failure passed. Avoid combining
unrelated task changes merely to produce a clean checkout.

After integration, retire a checkout only when its owner has finished and will
not resume there. Keep source branches as inexpensive recovery references. Use:

```sh
python3 scripts/engine-worktrees.py --worktree /absolute/task/worktree
# Review the report and confirm the owner has finished, then:
python3 scripts/engine-worktrees.py --worktree /absolute/task/worktree \
  --apply --inactive --expect-head FULL_SHA_FROM_REPORT
```

The default is a dry-run under the shared Cargo lock. The helper requires exact
registered paths, no changed/untracked/ignored files, no documented branch/path
references, no active process references, and ancestry in the integration base.
It checks the remote server's current branch head against that exact base rather
than trusting a stale tracking ref. Primary/invoking/protected/locked/detached
trees are blocked. Apply rechecks everything and calls `git worktree remove`
without force. Local and remote branches remain. It never runs `git clean` or
removes generated evidence, dependencies, caches, or another repository's tree.

Use `--base BRANCH --remote REMOTE --remote-branch BRANCH` for a different
published integration branch. A moving remote blocks removal until the local
base is reconciled. The tool is for cooperative local work: the Cargo lock
serializes native work, but it cannot stop a separate editor from creating files.
The owner must stop before retirement. If Linux denies inspection of same-user
processes, inspect ownership independently before using `--allow-uninspectable`;
actual detected process references always block removal.

Ignored files are deliberately a blocker, including `node_modules`, `target`,
and `generated`. Review those separately using the
[storage policy](game-engine-storage.md); an old worktree can hold the only copy
of an accepted binary or evidence. A path documented outside this repository
also needs review even though the helper cannot discover it. Preserve or relocate
that evidence and update references before trying retirement again.
