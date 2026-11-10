# Engine branch and worktree lifecycle

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

The 2026-11-09 inventory found 42 older worktrees whose commits were integrated;
40 still contained ignored local material. Two had no local material, but were
not retired automatically: registered T3 threads may still own them. The tool's
nine safety tests exercise clean retirement and rejection of unique commits,
stale publication, modified/untracked/ignored files, process references,
documented evidence, incorrect identity, protected paths and symlinks.

The 2026-11-09 main integration preserves both histories: the engine branch and
main had diverged by 205 and 249 commits at the gardening baseline. Integration
retains newer parser/deck-builder work and the engine's current implementation.
The former engine checkout contains historical evidence and installed dependencies;
its path is retained as a detached evidence checkout, not an active development
branch. Start new work from main. Never force-push either published history.
