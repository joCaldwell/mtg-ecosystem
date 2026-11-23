# Engine storage and cache lifecycle

Ignored paths are not disposable: `generated/` may hold receipts and evidence
referenced by docs. Only per-worktree Cargo targets are caches.

| Class | Examples | Policy |
| --- | --- | --- |
| Source/reviewed fixtures | Tracked code, card records, manifests and small results | Reviewed changes only |
| Pinned inputs | `.scryfall-cache`, rules, source archives, paper snapshots | Preserve while referenced; fresh downloads cannot replace historical bytes |
| Build caches | `target/engine` per worktree | Reproducible; bounded by `engine-cache.py check` |
| Release evidence | External `engine-evidence`, selected reports/binaries | No age-based deletion; replacement/retirement requires reference review |
| Toolchains | External `engine-toolchains` | Preserve while verification uses them |

## Cache selection and lifecycle

After pinned tool selection, `python3 scripts/engine-cache.py path` prints an
absolute CARGO_TARGET_DIR without writes. Explicit overrides win; otherwise each
worktree uses its own ignored `<worktree>/target/engine`, so removing a checkout
reclaims its cache and branches never thrash one shared cache. `lock` prints that
target's advisory lock, `~/.local/share/mtg-ecosystem/engine-verification/locks/cargo-<hash>.lock`
(hash of the canonical target; `MTG_ENGINE_LOCK_DIR` overrides the directory).
Cargo owns its profile/flag/toolchain inputs inside a target.

`status` reports allocated bytes, free space, selection, lock and budgets; `list`
reports every registered worktree's target size and lock state (`absent`, `idle`,
`busy`) plus legacy shared caches. `check` admits native work. Above **16 GiB
allocated** (`ENGINE_CACHE_MAX_GIB`) it evicts inside the target, under the
target's lock, until it fits: every `incremental/` directory first, then profile
directories other than the gate's `debug/` (for example `release/`), then the
whole target except `CACHEDIR.TAG`. Runners that already hold the lock pass
`--locked`; otherwise `check` takes the lock without waiting and refuses when it
is busy. It only evicts a real directory carrying Cargo's `CACHEDIR.TAG`, never a
symlink, never outside the target, and never `generated/` or external evidence.
Below **16 GiB free** (`ENGINE_CACHE_MIN_FREE_GIB`) it fails without deleting.
A target is a cache: nothing may reference a file inside it. Copy any binary a
receipt needs into `generated/` or the evidence store.

Cargo [local-source freshness](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html)
uses mtimes; an explicitly shared target across worktree baselines has reused stale
local crates. Prefer per-worktree targets. The former shared
`$XDG_CACHE_HOME/mtg-ecosystem/cargo/<repo>/<rustc>` cache and global
`engine-verification/cargo.lock` are no longer selected; `list` reports both, and
reclaiming that cache requires ownership review.

## Inventory

```sh
python3 scripts/engine-storage.py inventory
```

Inventory visits Git-registered worktrees and standard external stores, reports
symlinks without following them and reads no arbitrary home/secret files. Allocated
rows deduplicate hard links internally, but summing rows can double count inodes.
Historical retention bundles stay in
`~/.local/share/mtg-ecosystem/engine-evidence/`; no tracked tool rebuilds or
deletes them.

## Worktree lifecycle

[Development](game-engine-development.md#enter-a-task) owns base selection,
checkpoints and handoff. After integration, retire only an inactive finished
checkout whose owner will not resume; preserve branches as recovery references:

```sh
python3 scripts/engine-worktrees.py --worktree /absolute/task/worktree
# Review ownership/report before applying its exact head:
python3 scripts/engine-worktrees.py --worktree /absolute/task/worktree \
  --apply --inactive --expect-head FULL_SHA_FROM_REPORT
```

Dry-run/apply hold the retiring worktree's Cargo lock. Requirements: exact registered path, no changed,
untracked or ignored files, no documented branch/path or active process references,
and ancestry in the integration base. Remote publication is checked against the
server's live head, not stale tracking refs. Primary/invoking/protected/locked/
detached trees block. Apply rechecks and calls `git worktree remove` without force;
no git clean, branch removal or generated/dependency/cache deletion follows.

Defaults are local main and live origin/main. Other integration branches use
`--base BRANCH --remote REMOTE --remote-branch BRANCH`; moving remotes require
reconciliation. Stop the owner before apply: cooperative locks cannot stop editors.
Uninspectable `/proc` references require independent checks before the explicit
`--allow-uninspectable`; detected actual references always block.

`scripts/play-deck.sh` evidence directories (`generated/deck-play.*`) hold one
detached HEAD checkout, which the helper refuses by design. Its staged fixture
files duplicate `stage/`; `compiler`, `mtg-sim`, `stage/`, the build/cache
receipts, `game.json`, `replay.json` and the match's live journal
(`live-journals/`, the only record of an interrupted match's accepted inputs)
sit beside it. Older runs may hold `p5-live-journals/` there, or under
`source/generated/p5-live-journals/`: move the latter beside the checkout first. After recording those receipts (or retaining the directory in the evidence
store) and confirming no process or document references the checkout, its owner
retires only the checkout with
`git worktree remove --force generated/deck-play.ID/source`, which also deletes its
target. Removing the remaining directory is evidence retirement and needs the
same reference review.

Ignored node_modules/target/generated deliberately block retirement: a tree may
hold unique accepted binaries/proofs. Review and preserve/relocate those inputs,
update references (including outside this repository), then retry. The helper
cannot discover references in external documents and grants no evidence deletion.
