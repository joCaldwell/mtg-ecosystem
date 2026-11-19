# Engine storage and cache lifecycle

Ignored paths are not disposable. In particular, `generated/final-engine-verification/`
contains source designs, phase logs and acceptance evidence referenced by docs.

| Class | Examples | Policy |
| --- | --- | --- |
| Source/reviewed fixtures | Tracked code, card records, manifests and small results | Reviewed changes only |
| Pinned inputs | `.scryfall-cache`, rules, source archives, paper snapshots | Preserve while referenced; fresh downloads cannot replace historical bytes |
| Build caches | Compiler intermediates, `target/**/incremental` | Reclaim only after ownership checks; incremental first |
| Temporary generation | Run-owned retention scratch | Supervised cleanup on success, error and ordinary cancellation |
| Release evidence | External `engine-evidence`, selected reports/binaries | No age-based deletion; replacement/retirement requires reference review |
| Toolchains | External `engine-toolchains` | Preserve while verification uses them |

## Cache selection and lifecycle

After pinned tool selection, `python3 scripts/engine-cache.py path` prints an
absolute CARGO_TARGET_DIR without writes. Explicit overrides win; otherwise use
`$XDG_CACHE_HOME/mtg-ecosystem/cargo/<repo>/<rustc>` (`~/.cache` default), where
repo hashes canonical Git common-dir and rustc hashes `rustc -vV`. Worktrees share
a repo/toolchain cache; separate clones do not. Cargo owns its profile/flag inputs.

`status` reports allocated bytes, free space, selection and budgets. `check` fails
before native compilation above **16 GiB allocated** or below **4 GiB free**;
`ENGINE_CACHE_MAX_GIB` / `ENGINE_CACHE_MIN_FREE_GIB` override deliberately. This is
admission, not a runtime disk quota. Inspect usage after heavy builds. Launcher
and runner default CARGO_INCREMENTAL=0; explicit opt-in retains its disk tradeoff.

[Native coordination](game-engine-development.md#cargo-coordination-and-cache-integration)
holds the shared Cargo lock across admission/build. Cache path/status operations
neither lock nor delete. No automatic eviction/migration occurs. The preserved
`perf-engine-verification-loop/target` exceeds the default budget; coordinated reuse
requires the explicit 32 GiB override. Choosing another target leaves it intact.
Absolute worktree paths can still trigger recompilation in a shared cache.

Cargo [local-source freshness](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html)
uses mtimes; switching worktree baselines has reused stale local crates. Preserve
contradictory builds, coordinate ownership, refresh only relevant owned source/build
script timestamps with byte hashes unchanged, then rebuild under the lock. Never
delete the shared target as a repair. Check actual source identity/schema, not cache warmth.

## Inventory and bounded cache cleanup

From repository root:

```sh
python3 scripts/engine-storage.py inventory
python3 scripts/engine-storage.py prune-cargo --worktree /absolute/worktree
# Establish ownership and review dry-run JSON before applying:
python3 scripts/engine-storage.py prune-cargo --worktree /absolute/worktree --apply
```

Inventory visits Git-registered worktrees and standard external stores, reports
symlinks without following them and reads no arbitrary home/secret files. Allocated
rows deduplicate hard links internally, but summing rows can double count inodes.

`prune-cargo` proposes worktree debug/release and `engine:dev` incremental
directories above **2 GiB** in aggregate (`--max-bytes 0` proposes all).
It never invokes Cargo or removes deps/build/final executables, source or evidence.
Apply checks registration, tracked-file exclusion, containment, ancestor/internal
symlinks, shared Cargo lock and same-user Linux `/proc` cwd/exe/open descriptors.
Known active references always refuse. Uninspectable processes also refuse unless
`--allow-uninspectable` acknowledges an independent ownership check. Cooperative
locking cannot prevent an unrelated writer starting after the scan.

## Retention defaults

`npm run retain:engine` / `npm run retain:server` use `scripts/run-retention.py`;
direct TypeScript worker invocation is unsupported. Existing replay/hash/offline
and server/client guards still execute and their failures stay nonzero. Supervisor
safety tests use tiny fixtures/fake workers, not fresh full acceptance.

Defaults are **32 GiB scratch**, **128 GiB evidence store**, **16 GiB free** on both
filesystems. Admission and two-second allocated-byte samples stop the owned process
group on overflow. These are monitored bounds, not quotas: fast/concurrent writers
can overshoot. Reviewed larger captures may use explicit overrides:

```sh
npm run retain:engine -- /absolute/evidence-store --scratch-gib 48 --store-gib 160
npm run retain:server -- --scratch-gib 48
```

Server's evidence root comes from its verified predecessor pointer. A full store
refuses work; no existing evidence is deleted to make room. Review pinned sources,
rules, bundles, binaries, logs and downstream references before moving/retiring a
specific run or raising budgets. Age alone never authorizes retirement.

The supervisor holds the Cargo lock and owns fixed ignored
`generated/.retention-work` for captures, JSON stdout, Cargo homes and build targets.
Initial native builds override inherited target paths so shared caches cannot
escape the scratch bound. Host-only absolute MTG_ENGINE_ARTIFACT_ROOT routes demo
captures/journals there; relative values reject. Ordinary simulator output retains
its generated default. Incremental compilation remains off unless opted in.

Exact rebuilt binaries are copied exclusively to retained `bin/mtg-sim` or
`bin/mtg-server`, hash-checked and executed there before manifest publication.
Source closure/vendor/policy/bundles/proofs and failed retained outputs stay in the
store; temporary intermediates/duplicate captures are removed. Scratch failure
captures are not debug archives. Success/errors/SIGINT/SIGTERM stop descendants
and remove only owned scratch, reporting `generated/retention-storage-latest.json`.
SIGKILL/host/filesystem failures may leave it; future runs refuse existing scratch.
Inspect owner.json, prove descendants stopped under the lock, then review before
removing that exact directory. No generated/evidence-tree sweep is authorized.

Cheap checks: `python3 scripts/test-engine-storage.py`,
`python3 scripts/test-retention-workspace.py`, and pinned Node `--test` on
`scripts/{retention-workspace,retain-server-evidence,capture-json-output,retain-policy-sources}.test.ts`.
Parser publication atomically replaces named outputs and removes its own `.tmp`
files on ordinary failures; content-addressed inputs remain preserved.

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

Dry-run/apply hold the Cargo lock. Requirements: exact registered path, no changed,
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

Ignored node_modules/target/generated deliberately block retirement: a tree may
hold unique accepted binaries/proofs. Review and preserve/relocate those inputs,
update references (including outside this repository), then retry. The helper
cannot discover references in external documents and grants no evidence deletion.
