# Engine development

Use this guide for engine/server work and related parser tooling; other packages
own their checks. Read the [current map](game-engine-current.md), target package
manual and affected contracts. Historical acceptance is qualified to its source,
not a mandatory gate for each edit.

## Enter a task

Inspect status, branch, HEAD, supplied base and existing changes. Independent work
starts from current `origin/main`; dependent work uses its assigned base. In T3,
use an orchestrator-bound checkout rather than an unrelated shell worktree.
Coordinate owned files and the single native-work owner when peers are active.
Preserve unrelated changes, measured source, evidence and meaningful failures.

```sh
git status --short --branch
git config --local commit.gpgsign false
sh scripts/engine-env.sh node scripts/engine-doctor.ts --json
sh scripts/engine-env.sh node scripts/check-engine.ts --plan
```

Agents own routine implementation, verification and checkpoints. Ask Josh only
when product intent cannot be inferred, an unauthorized destructive action is
necessary, or access only he can supply is required. A failed check is evidence
to investigate, never permission to change an expected rules outcome.

## Fresh worktree

Reuse a healthy installation. Only when dependencies are missing or broken, and
no other task owns that installation, run:

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:source
```

Bootstrap checks tools, installs locked root/engine/parser workspaces with
`npm ci --include-workspace-root --include=dev --ignore-scripts --no-audit --no-fund`,
then runs doctor. It replaces that dependency installation; it does not install
native tools, build Cargo, fetch cards, regenerate fixtures or change Git refs.
It is not an application or SDK install. See [npm ci semantics](https://docs.npmjs.com/cli/v11/commands/npm-ci/).

Doctor JSON uses `format_version: 1`, `healthy`, `checks`, and optional
`error.code`. Exit 0 is healthy with possible warnings, 1 unhealthy, 2 invalid
arguments/configuration. A launcher failure can precede JSON: inspect stderr and
exit status. Doctor checks direct locked dependencies/workspace destinations,
not all transitive/cache integrity; upstream comparisons use local refs without
fetching. Free-space diagnostics fail below 256 MiB and warn below 2 GiB at the
worktree/nearest existing target ancestor; native cache budgets are separate.

## Toolchain setup

[.node-version](../.node-version) and [rust-toolchain.toml](../rust-toolchain.toml)
own the exact pins: Node 26.11.1 and Rust/Cargo 1.99.0 at this checkpoint. Doctor
also requires npm, rustfmt, Clippy, Python 3, Git and Linux `flock`; npm is reported,
not pinned (tested host version 12.2.1). Native host tooling requires Linux.

`sh scripts/engine-env.sh COMMAND [ARG ...]` selects the existing bundle
`~/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/{usr/bin,usr/lib}`.
`MTG_ENGINE_TOOLCHAIN_ROOT` selects another absolute existing bundle. Missing
explicit overrides fail; an absent default permits existing PATH tools. The
launcher verifies Node, sets process-local PATH/LD_LIBRARY_PATH and pinned
RUSTUP_TOOLCHAIN with RUSTUP_AUTO_INSTALL=0. It changes no shell/global settings
and installs nothing. The host bundle has no npm; PATH's npm uses selected Node.

On a new machine, explicitly arrange the pinned Node from its
[official archive/checksums](https://nodejs.org/en/download/archive/v26.11.1).
With existing rustup, optional explicit setup is:

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
```

Keep user-owned proxies on PATH. Do not downgrade system tools or change global
rustup defaults; missing libraries need compatible host setup, not copied bare
executables. [rustup environment behavior](https://rust-lang.github.io/rustup/environment-variables.html).

## Choose focused checks

Josh's 2026-11-09 direction favors faster focused verification with explicit
omissions, superseding mandatory full matrices. `--plan` prints commands without
locks or execution. Run checks once on stable source and confirm selectors
actually execute tests; add affected runtime/compiler/parser/SDK tests as needed.

| Root command | Executes | Omits |
| --- | --- | --- |
| `npm run check:engine` | Tooling/failure guards, exact archive/parser agreement case, formatting, workspace all-target `cargo check --locked`, contracts/compiler library tests | Core/sim/server runtime tests, integration/doc tests, Clippy, executable Node contracts/demo, SDK, release/profile/replay/retention matrices |
| `npm run check:engine:source` | Tooling and archive/parser agreement; no Cargo/lock | All native and executable runtime checks |
| `npm run check:engine:contracts` | Tooling and source/schema/catalog Node preflight with invalid-shape/version negatives | Runtime-backed Node contracts/demo, Rust tests/lints; a cold cache may compile release mtg-sim |
| `npm run check:engine:full` | Formatting, Clippy, workspace Rust tests, tooling, complete Node contracts/demo | SDK, ignored diagnostics, separate release/profile/replay/offline-retention checks |

Use the launcher for these commands. `--fast`, `--source`, `--contracts`, `--full`
are mutually exclusive. Every mode reports omissions and includes environment,
cache, worktree, dependency/artifact, documentation budgets/links and checker tests,
storage/retention safety and executable preservation guards; these use tiny fixtures/fake workers, not offline rebuilds.
Python suites must run by filename: discovery skips hyphenated names. Node's
selected-test wrapper rejects empty/skipped/TODO-only selections. Source/fast's
exact archive test selector must stay synchronized with test renames. Rust tests
use opt-level 2 with assertions/overflow checks.

Select new behavior and meaningful negatives, then the smallest affected adjacent
regressions. Check baseline source, complete fixtures and lifecycle before writing
a test; never bless guessed diagnostics or unsupported baseline behavior. Use
executable contracts for IR/schema changes and isolated SDK checks for public API
changes. Choose full/release, a profile/seed, independent replay or reconstruction
only when the change or requested claim needs it. Documentation/read-only reviews
do not invalidate compiled-source evidence. After fixes, repeat affected checks
with a reason; retain earlier failures/receipts and explicit unchanged-input bridges.

## Cargo coordination and cache integration

One native owner holds `~/.local/share/mtg-ecosystem/engine-verification/cargo.lock`.
Fast/contracts/full, smoke and retention runners own it internally, including
nested Node Cargo calls: **never acquire it twice**. An explicitly chosen
multi-phase acceptance may also coordinate `full-gate.lock` in the same directory.
Wrap standalone Cargo commands (replace FILTER and verify tests ran):

```sh
sh scripts/engine-env.sh flock "$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock" cargo test --locked -p mtg-core FILTER
```

The lock parent must exist. Do independent source/docs work while busy; never
edit measured source or launch duplicate native builds. Launcher/checks select
CARGO_TARGET_DIR through the cache helper and default CARGO_INCREMENTAL=0; explicit
overrides win. Native cache admission runs under the lock, not on every source-only
command. [Storage/cache policy](game-engine-storage.md#cache-selection-and-lifecycle)
owns budgets, freshness failures and protected historical caches.

## Bound and publish a batch

Deliver one coherent mechanic family using shared IR/runtime systems and exact
source-backed witnesses, never card-name dispatch. State behavior, owned scope,
checks and exclusions before edits. Keep follow-on admission, policy and consumer
qualification separate unless required for correctness. Aim for a checked
checkpoint within 30 minutes; if longer, report elapsed time, remaining work and
whether to split. The target never excuses weakened guards or incomplete claims.
Reuse onboarding/environment and existing runners/receipts; no transcript or
per-task audit file belongs in permanent manuals.

Commit coherent focused checkpoints and push before handoff/end of session; no
full matrix is required. Stage owned files only, never force-push shared branches
or merge incidentally. Repository-local unsigned commits avoid interactive signing;
scratch Git fixtures also need local signing disabled. Preserve original failures,
patches, bases and source snapshots if publication access blocks. Keep measured
source/index frozen during Git operations. Ordinary scope decisions need no new
approval. Retire worktrees only through [storage policy](game-engine-storage.md#worktree-lifecycle).

## Hand off verifiable state

Report branch/worktree/base/HEAD, publication and dirty/owned paths; resulting
behavior; exact commands/selectors, results, source identities, receipt locations
and omissions; active processes/native-slot owner; retained artifacts and remaining
work/decisions. Confirm push success against the intended remote ref. Include
increment/cumulative elapsed time, distinguishing command time and known lock waits
from unitemized work. A successor rechecks identities/ownership before resuming.

## Measurement and coverage reporting

Check/smoke stdout streams subprocess output, not one JSON document. Use timing
JSONL for source/driver/executable identities, commands, exits, coverage, wall/CPU,
RSS, lock waits and host load. `MTG_ENGINE_TIMING_FILE` selects a create-new path;
a source change during execution fails. Compare matching source/workload/cache
conditions; label cold builds and reduced coverage. CPU includes waited children;
RSS is largest individual process, not group peak. Shared-host developer feedback
is not a quiet P7 performance claim. [Evidence](game-engine-evidence.md) owns proofs.

## Smoke probe

```sh
sh scripts/engine-env.sh npm run engine:smoke -- --artifact generated/NEW.json
sh scripts/engine-env.sh npm run engine:smoke:replay -- generated/NEW.json
```

The optional artifact is create-new, at most 65,536 bytes, without parent creation,
replacement or symlink overwrite. Smoke compiles five exact fixtures and admits
`ScenarioConfig::land_return()` (three cards/four seats), activates Island mana and
passes priority through public observations/validation, replays every accepted
prefix and rejects stale tokens atomically. It is seedless with tracing off.

Successful artifacts are read back; reproduced failures still exit nonzero.
Panic/kill can prevent capture. Artifacts contain trusted raw journals/checkpoints
and identities, never public player data or a retained source/binary closure.
Preserve the matching executable/source separately. Replay rejects identity changes
before reporting the first bounded JSON-pointer divergence. Runtime timing includes
source admission/replay/artifact I/O, excludes startup/Cargo; cold codegen can be
expensive. This probe is not full-game/profile/search/host/SDK/release/retention
acceptance and does not replace affected mechanic tests.
