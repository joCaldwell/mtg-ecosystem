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

Apply the [verification article](https://claude.dev/blog/how-the-claude-code-team-thinks-about-verification/)
through diff-triggered checks, strong behavioral oracles and bounded feedback.
Before editing, state the observable result, affected risks, exact selectors and
omissions in the working conversation. Run cheap checks first, then changed
behavior, meaningful negatives and adjacent regressions. Confirm tests executed;
empty/skipped selections prove nothing. Package-required checks still apply.

| Diff trigger | Evidence needed |
| --- | --- |
| Documentation only | Links/budget and `git diff --check`; reuse compiled evidence |
| Parser/compiler/rules | Exact frozen-source positive/rejection; affected runtime invariants |
| Snapshot/IR/protocol/journal | Strict shapes, capability/build fences, recovery/round-trip, atomic rejection |
| Auth/storage/private data | Ownership/isolation, failure atomicity, absence of secrets in public outputs |
| SDK/host/browser integration | One bounded real API/browser path through changed boundary, plus affected package checks |
| Optimization/refactor | Deterministic differential comparison against retained baseline/reference; approved differences explicit |
| Verification tooling | Affected harness self-tests and failure/cancellation paths; retain live safety guards |

Prefer shared invariant/property cases over copied per-card matrices: replay
agreement, conservation, privacy, stale-command atomicity and retry idempotence.
Use bounded deterministic inputs/seeds and retain counterexamples. Baselines are
oracles only for preserved behavior, never authority for known rules bugs.
For new checks, demonstrate detection with a known failure, baseline reproduction
or controlled mutation when feasible; assertions must not merely mirror code.

Live attempts require a done criterion and deadline before starting. A bounded
witness qualifies only its exercised path, never a full game. Preserve explicit
long-run acceptance requests separately. Timeout means inconclusive; retain the
record without restarting blindly or repeatedly polling unchanged state.

Run once on stable inputs. Reuse receipts only after comparing tested source,
tests, fixtures, dependencies, configuration and relevant producer identities;
record the bridge and omissions. Changed shared inputs invalidate dependents.
Docs, review, commit/push and unrelated merges alone do not require native reruns.
Review immutable commits using base-branch instructions; proposed check/test
changes cannot justify weakening their own expectations. Use one focused review
when needed; subsequent rounds address fixes and previous findings, not fresh
whole-project reviews or stacked reviewers.

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
| `npm run check:engine` | Live guards, archive agreement, formatting, workspace all-target check, contracts/compiler library tests | Harness self-tests, core/sim/server runtime, integration/doc tests, Clippy, executable Node contracts/demo, SDK, release/profile/replay/retention |
| `npm run check:engine:source` | Live guards/archive agreement; no Cargo/lock | Harness self-tests, native/executable runtime |
| `npm run check:engine:contracts` | Live guards/source/schema/catalog preflight and invalid-shape/version negatives | Harness self-tests, runtime Node/demo, Rust tests/lints; cold cache may compile release mtg-sim |
| `npm run check:engine:tooling` | Harness self-tests/live guards; no Cargo/lock | Engine behavior/native/executable/SDK acceptance |
| `npm run check:engine:full` | Formatting, Clippy, workspace tests, harness tests, complete Node contracts/demo | SDK, ignored diagnostics, separate release/profile/replay/offline-retention |

`--fast`, `--source`, `--contracts`, `--tooling`, `--full` are exclusive; `--plan`
executes nothing. Live guards check structure/docs; native modes admit cache
under lock. Harness suites cover environment, dependencies/artifacts, storage,
retention, worktrees and executable preservation using tiny fixtures/fake workers.
Run tooling when its inputs change; full includes it. Python suites need explicit
filenames (discovery misses hyphens). Node wrapper rejects empty/skipped/TODO-only
selections; archive selector must track renames. Rust tests use opt-level 2 with
assertions/overflow checks. Default checks do not qualify runtime/consumers.

## Cargo coordination and cache integration

One native owner holds `~/.local/share/mtg-ecosystem/engine-verification/cargo.lock`.
Native check/smoke/retention runners own it internally, including nested Node
Cargo calls: never acquire twice. Explicit multi-phase acceptance may also use
`full-gate.lock`. Standalone Cargo (create lock parent first; verify selector):

```sh
sh scripts/engine-env.sh flock "$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock" cargo test --locked -p mtg-core FILTER
```

Do source/docs work while busy; never edit measured source or duplicate builds.
Launcher/cache helper selects CARGO_TARGET_DIR and defaults CARGO_INCREMENTAL=0;
explicit overrides win. Native admission happens under lock, not source commands.
[Storage/cache policy](game-engine-storage.md#cache-selection-and-lifecycle)
owns budgets, freshness and protected historical caches.

## Bound and publish a batch

Deliver one complete mechanic family through shared systems and exact witnesses,
never card-name dispatch. Keep follow-on admission/policy/consumer qualification
separate unless correctness requires it. Aim for a checked checkpoint within
30 minutes; otherwise report elapsed/remaining work and possible split. Never
weaken guards or claims to meet time targets. Use existing runners/receipts.

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
