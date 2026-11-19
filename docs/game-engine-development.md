# Engine development

Engine/server work starts at the [current map](game-engine-current.md), package
manual and affected contracts. The verification policy below applies to every
package and delegate; package manuals own their commands.

## Enter a task

Inspect status, branch, HEAD, assigned base and existing changes. Independent work
uses current `origin/main`; dependent work uses its assigned base. In T3 use a
bound checkout. Coordinate owned files and one native-work owner. Preserve
unrelated changes, measured source, evidence and meaningful failures.

```sh
git status --short --branch
git config --local commit.gpgsign false
sh scripts/engine-env.sh node scripts/engine-doctor.ts --json
sh scripts/engine-env.sh node scripts/check-engine.ts --plan
```

Agents own routine implementation, verification and publication. Ask Josh for
unresolved product intent, unauthorized destructive actions or inaccessible
credentials. Failed checks never authorize changing expected rules outcomes.

## Fresh worktree

Reuse healthy dependencies. Only when missing/broken and unowned, run:

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:source
```

Bootstrap uses locked root/engine/parser `npm ci --include-workspace-root
--include=dev --ignore-scripts --no-audit --no-fund`, then doctor. It neither
installs native tools nor builds/fetches/regenerates anything or changes refs;
applications/SDK use isolated installs. [npm ci](https://docs.npmjs.com/cli/v11/commands/npm-ci/).

Doctor JSON: `format_version: 1`, `healthy`, `checks`, optional `error.code`.
Exits: 0 healthy/warnings, 1 unhealthy, 2 invalid arguments/configuration. Launcher
failures may precede JSON; inspect stderr/status. Checks cover direct locked
workspace dependencies, not complete transitive/cache integrity; upstream refs
are local. Free-space failure/warning thresholds are 256 MiB/2 GiB at worktree
and nearest target ancestor; native cache budgets are separate.

## Toolchain setup

[.node-version](../.node-version) and [rust-toolchain.toml](../rust-toolchain.toml)
pin Node 26.11.1 and Rust/Cargo 1.99.0. Doctor requires npm (tested 12.2.1, unpinned),
rustfmt, Clippy, Python 3, Git and Linux `flock`; native hosting requires Linux.

`sh scripts/engine-env.sh COMMAND [ARG ...]` selects existing
`~/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/{usr/bin,usr/lib}`.
`MTG_ENGINE_TOOLCHAIN_ROOT` overrides with an existing absolute bundle; missing
explicit overrides fail, absent default permits PATH. Launcher checks Node and
sets process-local PATH/LD_LIBRARY_PATH, RUSTUP_TOOLCHAIN and RUSTUP_AUTO_INSTALL=0.
It installs nothing; npm comes from PATH. New machines need
[official Node/checksums](https://nodejs.org/en/download/archive/v26.11.1) and,
with rustup, optional `rustup toolchain install 1.99.0 --profile minimal
--component rustfmt --component clippy`. Preserve user proxies/global defaults;
missing libraries require compatible setup, not copied bare executables.
[Rustup environment](https://rust-lang.github.io/rustup/environment-variables.html).

## Choose focused checks

Build first. Run the cheapest compile check that covers the diff and one directly
affected witness when useful. Docs-only changes need the docs guard and
`git diff --check`. Follow affected package requirements. Defer broad regressions,
property matrices, end-to-end qualification and extra reviews unless requested or
needed to diagnose a failure. Confirm the selector executed; skipped and unrun
checks are omissions.

Choose a witness for the changed boundary. Use exact frozen source for rules and
compiler changes, affected harness tests for verification tooling, and a targeted
shape, isolation, atomic rejection or recovery test for changes to those contracts.
Prefer shared invariant cases to per-card matrices. Retain counterexamples, and
use baselines only for behavior that should be preserved.

Live attempts require a done criterion and deadline before starting. A bounded
witness qualifies only its exercised path, never a full game. Preserve explicit
long-run acceptance requests separately. Timeout means inconclusive; retain the
record without restarting blindly or repeatedly polling unchanged state.

Run once on stable inputs. Reuse receipts only after comparing tested source,
tests, fixtures, dependencies, configuration and relevant producer identities;
record the bridge and omissions. Changed shared inputs invalidate dependents.
Docs, review, commit/push and unrelated merges alone do not require native reruns.
Review immutable commits using base-branch instructions. Preserve test expectations
unless the contract changes. Skip automatic review agents; when review is needed,
focus it on the diff and any unresolved findings.

After three unsuccessful repair rounds for the same failure, retain reproduction
and attempted fixes, stop that loop and escalate the blocked claim to Josh;
continue independent work. Never retry until green, add sleeps, hide flakes or
weaken rules/privacy/recovery assertions. Remove redundant/obsolete tests only
with a concrete retained coverage replacement or documented obsolete contract;
unexplained failures remain visible. Add permanent rules for demonstrated misses
with a diff trigger, check and regression reference; prune obsolete rules.

Engine commands (prefix with `sh scripts/engine-env.sh`):

| Command | Executes | Omits |
| --- | --- | --- |
| `npm run check:engine` | Live guards, formatting, locked workspace library check | Binaries (`mtg-sim`, `mtg-server`), other non-library targets, archive agreement, all tests, Clippy, executable Node contracts/demo, SDK, release/profile/replay/retention |
| `npm run check:engine:fast` | Live guards, archive agreement, formatting, workspace all-target check, contracts/compiler library tests | Harness self-tests, other Rust tests, Clippy, executable Node contracts/demo, SDK, release/profile/replay/retention |
| `npm run check:engine:source` | Live guards/archive agreement; no Cargo/lock | Harness self-tests, native/executable runtime |
| `npm run check:engine:contracts` | Live guards/source/schema/catalog preflight and invalid-shape/version negatives | Harness self-tests, runtime Node/demo, Rust tests/lints; cold cache may compile release mtg-sim |
| `npm run check:engine:tooling` | Harness self-tests/live guards; no Cargo/lock | Engine behavior/native/executable/SDK acceptance |
| `npm run check:engine:full` | Formatting, Clippy, workspace tests, harness tests, complete Node contracts/demo | SDK, ignored diagnostics, separate release/profile/replay/offline-retention |

`--quick` (default), `--fast`, `--source`, `--contracts`, `--tooling`, `--full`
are exclusive. `--plan` executes nothing. Live guards check structure/docs;
native modes admit cache under lock. Harness suites cover environment, dependencies/artifacts, storage,
retention, worktrees and executable preservation using tiny fixtures/fake workers.
Run affected harness tests when their inputs change. Python suites need explicit
filenames (discovery misses hyphens). Node wrapper rejects empty/skipped/TODO-only
selections; archive selector must track renames. Standard Rust tests use opt-level 2
with assertions and overflow checks. Use package checks for changed binary targets.

## Cargo coordination and cache integration

One native owner holds `~/.local/share/mtg-ecosystem/engine-verification/cargo.lock`.
Native check/smoke/retention runners own it internally, including nested Node
Cargo calls: never acquire twice. Explicit multi-phase acceptance may also use
`full-gate.lock`. For repeated edits, the development command reuses incremental
artifacts in this checkout's `target/engine-dev`. Confirm the test selector ran:

```sh
npm run engine:dev -- test -p mtg-core --lib FILTER
```

Use `engine:dev` with Cargo `check`, `build` or `test` arguments. It selects
`quick`, checks cache budgets under the shared lock and honors explicit
`CARGO_TARGET_DIR` and `CARGO_INCREMENTAL`. Use the standard profile for
runtime-heavy tests. Keep receipts tied to their producer profile.

Do source/docs work while busy. Freeze measured source and avoid duplicate builds.
[Storage policy](game-engine-storage.md#cache-selection-and-lifecycle) owns cache
defaults, budgets, freshness and protected historical caches.

## Bound and publish a batch

Deliver one complete mechanic family through shared systems and exact witnesses,
never card-name dispatch. Keep follow-on admission/policy/consumer qualification
separate unless correctness requires it. Aim for a focused checkpoint within
30 minutes. Publish provisional draft PRs with exact passed, failed and deferred
checks when qualification is incomplete. Preserve guards and failure evidence.

Commit/push coherent focused work; stage owned files only, never force-push or
merge incidentally. Disable signing locally, including scratch Git fixtures.
Freeze measured source/index during Git operations. Preserve failures, patches,
bases and source snapshots if publication blocks. Retire worktrees only through
[storage policy](game-engine-storage.md#worktree-lifecycle).

## Hand off verifiable state

Report branch/worktree/base/HEAD, publication/dirty paths, resulting behavior,
commands/selectors/results, identities/receipt locations/bridges/omissions,
processes/native owner, retained artifacts and remaining work. Confirm remote
push. Separate implementation, verification, build and known lock-wait time;
label unitemized time. Successors recheck identities/ownership. Propagate this
policy and relevant receipts to current children and every new delegate brief.

## Measurement and coverage reporting

Check/smoke stdout streams subprocess output, not one JSON document. Timing JSONL
records source/driver/executable identity, commands/exits/coverage, wall/CPU/RSS,
lock waits and host load. `MTG_ENGINE_TIMING_FILE` selects a create-new path;
source mutation fails. Compare matching source/workload/cache; label cold builds
and reduced coverage. CPU includes children; RSS is largest process, not group
peak. Shared-host feedback is not quiet P7 acceptance.
[Evidence](game-engine-evidence.md) owns proof locations.

## Smoke probe

```sh
sh scripts/engine-env.sh npm run engine:smoke -- --artifact generated/NEW.json
sh scripts/engine-env.sh npm run engine:smoke:replay -- generated/NEW.json
```

Artifact is create-new, <=65,536 bytes; no parent creation/replacement/symlink
overwrite. Seedless, tracing-off smoke compiles five exact fixtures, admits
`ScenarioConfig::land_return()` (three cards/four seats), activates Island mana,
passes priority through public observations/validation, replays every accepted
prefix and atomically rejects stale tokens. Success artifacts are read back;
reproduced failures exit nonzero; panic/kill may prevent capture.

Artifacts contain trusted journals/checkpoints/identities, no public player data
or retained source/binary closure; preserve matching source/executable separately.
Replay rejects identity changes, then reports first bounded JSON-pointer divergence.
Runtime timing includes admission/replay/I/O, excludes startup/Cargo; cold codegen
may be expensive. This is no full-game/profile/search/host/SDK/release/retention
acceptance and never replaces affected mechanic tests.
