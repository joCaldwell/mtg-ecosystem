# Engine branch and worktree lifecycle

Start dependent engine work from the latest published `feat/engine-plan`, not
root `main`: the engine implementation has not yet been merged there. In T3,
create a worktree with the thread/worktree tool using this local base and
`startFromOrigin: false`. Record the base SHA and owned files with the coordinator.
Do not create an unrelated shell worktree for a T3 thread.

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

Main integration remains a separate reviewable change. At the gardening baseline,
main and the engine branch diverged by 205 and 249 commits respectively. Git's
synthetic merge was conflict-free; that does not establish cross-package runtime
compatibility. Preserve the engine checkpoint and review a dedicated merge
before changing the default branch. Avoid force-pushing either history.
