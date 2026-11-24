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
sh scripts/engine-env.sh npm run check:engine:tooling
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

### Inner loop and one gate

While coding, run only the selector the edit touches, then the gate once on the
commit you publish:

```sh
npm run engine:dev -- test -p mtg-compiler --lib FILTER
npm run engine:dev -- test -p mtg-core --test rules FILTER
sh scripts/engine-env.sh npm run check:engine:gate
```

| Command | Executes |
| --- | --- |
| `npm run check:engine` | Live guards, formatting, `cargo check --workspace --all-targets --features mtg-core/testkit`, production `mtg-server`/`mtg-sim` check |
| `npm run check:engine:gate` | The above with Clippy `-D warnings`, parser archive agreement, every workspace unit, doc and integration test, and all harness self-tests |
| `npm run check:engine:tooling` | Harness self-tests and live guards; no Cargo or lock |

Prefix with `sh scripts/engine-env.sh`. `--plan` prints commands and omissions
without executing them. Every debug command shares one feature shape
(`--workspace --features mtg-core/testkit`) and one `dev`/`test` profile:
dependencies at opt-level 2 without debug info, `mtg-core`/`mtg-contracts` at
opt-level 1 (the fastest measured rebuild-plus-test setting), other members
unoptimized, all with line tables, assertions and overflow checks. The production check builds
the host and simulator without `testkit`, so dead code stays visible. No plan
builds release or runs `mtg-sim`. Compiler-coverage PRs add one whole-archive
scan through `engine:decks coverage --all` before publishing. Python suites need
explicit filenames (discovery misses hyphens).

### Engine test topology

Cargo builds nine test executables. Five library harnesses hold local contracts,
compiler lowering and catalog regeneration, private core anchors, simulator
policies/search/smoke, and server unit tests. Four integration suites own the
cross-module behavior.

| Integration target | Scope |
| --- | --- |
| `mtg-core --test rules` | Turns, stack, combat, payment, commander, triggers, kernel and public adversarial inputs |
| `mtg-core --test recovery` | Snapshots, history, query work, search privacy, trace and budget invariance |
| `mtg-witness --test cards` | Runtime witnesses compiled from exact frozen R4 source |
| `mtg-server --test host` | Host protocol, store, authorization and recovery, including the opt-in systemd resume case |

Core's non-default `testkit` feature supplies `FixtureCatalog` (one parse per
catalog per process), `Board` (name-based private scenarios) and `Driver`
(trusted request/response helpers). Public adversarial modules use the public
API and the permitted-input helpers in `core/tests/support`. Compiler-backed
witnesses stay in their own crate. Nine core endpoints (including surviving priority after a
concession) and a compiled R4 combat-trigger endpoint exercise the same
`assert_recoverable` matrix. Each checks replay, restored state, fork and trace
at quanta 1 and `u64::MAX` in Off, Compact, Stream and Detailed modes. Mechanic-specific
forgery, privacy and saved-work assertions stay with their witnesses.

The gate runs `cargo test --tests` directly with one libtest thread budget
(minimum of available CPUs and 16). Test helpers must not spawn their own thread
pools (13 GiB nested-pool lesson). Each of the nine named harnesses must report
a passing test. The host's opt-in systemd case shares its executing harness.
The separate `cargo test --doc` phase requires both passing core compile-fail
examples for action continuations and player sessions.

With eight Cargo jobs, commit `173bdcf2` measured 177.354 s cold (6.71 GiB
target, 3.01 GiB largest-process peak RSS), 32.585 s warm (median of three)
and 43.865 s after a one-line core edit. The slowest individual test took
4.722 s. [PR #199](https://github.com/joCaldwell/mtg-ecosystem/pull/199)

## Cargo coordination and cache integration

Each worktree builds in its own `target/engine` and holds that target's advisory
lock (`python3 scripts/engine-cache.py lock`); worktrees never wait on or
invalidate each other. Commands sharing a target, including an explicit shared
`CARGO_TARGET_DIR`, serialize. `check:engine*`, `engine:dev` and `play-deck.sh`
own the lock internally and admit the cache under it: never acquire it twice.
`engine:dev` accepts Cargo `check`, `build`, `test` or `clippy` arguments and
adds `--features mtg-core/testkit` when the selection builds core. Unset
`CARGO_BUILD_JOBS` defaults to a quarter of the CPUs (at least 4); on 32
threads/30 GiB a cold test build then peaks near 6 GiB per worktree.
[Storage policy](game-engine-storage.md#cache-selection-and-lifecycle) owns the
budget and eviction.

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

## Measurement and deck work

Check output streams subprocess output. Timing JSONL records source/driver
identity, commands, exits, wall/CPU/peak RSS, lock waits and host load;
`MTG_ENGINE_TIMING_FILE` selects a create-new path and source mutation fails.
RSS is the largest single process. Compare only matching source, workload and
cache state; label cold builds.

`npm run engine:decks -- COMMAND` is the one deck harness, on a release
`mtg-sim` built from this checkout:

```sh
npm run engine:decks -- coverage SIM --out generated/NEW [--baseline OLD/report.json] [--all]
npm run engine:decks -- stage SIM OUT --deck "Jo's Cascade"   # or --decklist FILE
npm run engine:decks -- play "Jo's Cascade" 0
npm run engine:decks -- bench SIM --bulk-artifact DIR --manifest-sha SHA --pod-decks A.json --swap SLOT:REMOVE:ADD --blocks 8 --out generated/NEW
```

`coverage` lowers only the deck snapshot's Oracle IDs (`--all`: the whole
archive) and reports compiled slots, blocking cards, top blockers, the Track 3A
family scoreboard and baseline deltas. The scoreboard classifies every blocker
of each unsupported card into a family and counts, deck-first, the deck slots
and cards each family touches and alone blocks ("sole"), plus Commander-legal
archive cards with `--all`; every 3A PR reports against it. `stage` accepts only the registered Archive R4 file, requires every card
to compile and templates the reviewed `commander-foundations-v1` profile.
`play` stages and plays one seeded match from a detached HEAD under
`generated/deck-play.*`, with live journals in its `live-journals/`. `bench` wraps the [paired runner](game-engine-search.md#paired-deck-experiments)
and writes its report and Markdown summary.
