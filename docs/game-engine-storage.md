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

Git tracks qualified TypeScript SDK outputs for Railway builds under
`packages/game-server/sdk/dist/` and `packages/game-server/sdk/generated/`, plus
`packages/game/generated/sdk-identity.ts`. The structure guard rejects `target`,
`node_modules`, `.scryfall-cache` and `__pycache__` paths, and ELF, Wasm and SQLite
payloads, within these outputs.

## Cache selection and lifecycle

After pinned tool selection, `python3 scripts/engine-cache.py path` prints an
absolute CARGO_TARGET_DIR without writes. Explicit overrides win; otherwise each
worktree uses its own ignored `<worktree>/target/engine`, so removing a checkout
reclaims its cache and branches never thrash one shared cache. `lock` prints that
target's advisory lock,
`~/.local/share/mtg-ecosystem/engine-verification/locks/cargo-<hash>.lock` (hash of
the canonical target; `MTG_ENGINE_LOCK_DIR` overrides the directory).

`status` reports allocated bytes, free space, selection, lock and budgets; `list`
reports every registered worktree's target size and lock state (`absent`, `idle`,
`busy`) plus legacy shared caches. `check` admits native work. Above **16 GiB
allocated** (`ENGINE_CACHE_MAX_GIB`) it evicts inside the target, under its lock,
until it fits: every `incremental/` directory, then profiles other than the gate's
`debug/`, then the whole target except `CACHEDIR.TAG`. A runner already holding the
lock passes its descriptor in `MTG_ENGINE_LOCK_FD`, which `check` verifies holds
the exclusive lock (never re-locking it); otherwise `check` takes the lock without
waiting. Eviction pins the checkout's own `target/engine` without following links,
requires a regular Cargo `CACHEDIR.TAG` and refuses a lock inside the target. An
explicit `CARGO_TARGET_DIR` elsewhere, `generated/` and external evidence are never
touched. Below **16 GiB free** (`ENGINE_CACHE_MIN_FREE_GIB`) it fails without
deleting. Nothing may reference a file inside a target; copy any binary a receipt
needs into `generated/` or the evidence store.

Cargo [local-source freshness](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html)
uses mtimes, so an explicitly shared target across worktree baselines can reuse
stale local crates; prefer per-worktree targets. `list` also reports the unselected
legacy `$XDG_CACHE_HOME/mtg-ecosystem/cargo/<repo>/<rustc>` cache and global
`engine-verification/cargo.lock`; reclaiming that cache requires ownership review.

## Inventory

```sh
python3 scripts/engine-storage.py inventory
```

Inventory visits Git-registered worktrees and standard external stores, reports
symlinks without following them and reads no arbitrary home/secret files. Rows
deduplicate hard links internally, but summing rows can double count inodes.
Historical retention bundles stay in
`~/.local/share/mtg-ecosystem/engine-evidence/`; no tracked tool rebuilds or deletes
them.

## Worktree lifecycle

[Development](game-engine-development.md#enter-a-task) owns base selection,
checkpoints and handoff. After integration, retire only an inactive finished
checkout whose owner will not resume; keep branches as recovery references:

```sh
python3 scripts/engine-worktrees.py --worktree /absolute/task/worktree
# Review ownership/report before applying its exact head:
python3 scripts/engine-worktrees.py --worktree /absolute/task/worktree \
  --apply --inactive --expect-head FULL_SHA_FROM_REPORT
```

Dry-run/apply hold the worktree's Cargo lock and require an exact registered path,
no changed, untracked or ignored files, no documented branch/path or active process
references, and ancestry in the integration base (remote publication is checked
against the server's live head). Primary/invoking/protected/locked/detached trees
block. Apply rechecks and calls `git worktree remove` without force; no git clean,
branch removal or generated/dependency/cache deletion follows. Defaults are local
main and live origin/main; other bases use
`--base BRANCH --remote REMOTE --remote-branch BRANCH`. Stop the owner before
apply, since cooperative locks cannot stop editors. Uninspectable `/proc`
references need independent checks before `--allow-uninspectable`; detected
references always block.

Ignored node_modules/target/generated deliberately block retirement because a tree
may hold unique accepted binaries/proofs. Preserve or relocate them, update
references (including outside this repository), then retry. The helper cannot see
external documents and grants no evidence deletion.

`scripts/play-deck.sh` evidence directories (`generated/deck-play.*`) hold one
detached HEAD checkout (`source/`), which the helper refuses by design, beside
`compiler`, `mtg-sim`, `stage/`, build/cache receipts, `game.json`, `replay.json`
and `live-journals/`, the only record of an interrupted match's accepted inputs.
Older runs may hold `p5-live-journals/` there or under
`source/generated/p5-live-journals/`; move the latter beside the checkout first.
After recording those receipts (or retaining the directory in the evidence store)
and confirming no process or document references the checkout, its owner retires
only the checkout with `git worktree remove --force generated/deck-play.ID/source`,
which also deletes its target. Removing the rest is evidence retirement and needs
the same reference review.
