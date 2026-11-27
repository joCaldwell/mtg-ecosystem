# Engine development

Engine/server work starts at the [current map](game-engine-current.md), package
manual and affected contracts. The verification policy below applies to every
package and delegate; package manuals own their commands.

## Enter a task

Inspect status, branch, HEAD, assigned base and existing changes. Independent work
uses current `origin/main`; dependent work uses its assigned base. In T3 use a
bound checkout. Coordinate owned files and one native-work owner.

```sh
git status --short --branch
git config --local commit.gpgsign false
sh scripts/engine-env.sh node scripts/engine-doctor.ts --json
sh scripts/engine-env.sh node scripts/check-engine.ts --plan
```

Ask Josh only for unresolved product intent, unauthorized destructive actions or
inaccessible credentials. Failed checks never authorize changing expected rules
outcomes.

## Fresh worktree

Reuse healthy dependencies. Only when missing/broken and unowned, run:

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:tooling
```

Bootstrap runs locked root/engine/parser `npm ci` without scripts, then doctor;
it installs no native tools, builds or fetches nothing and changes no refs.
Applications/SDK use isolated installs. Doctor (`scripts/engine-doctor.ts`)
exits 0 healthy/warnings, 1 unhealthy, 2 invalid arguments; launcher failures may
precede its JSON, so inspect stderr/status. It checks direct locked workspace
dependencies, not transitive or cache integrity, and fails/warns below 256 MiB/2 GiB
free at the worktree and nearest target ancestor.

## Toolchain setup

[.node-version](../.node-version) and [rust-toolchain.toml](../rust-toolchain.toml)
pin Node 26.11.1 and Rust/Cargo 1.99.0. Doctor requires npm (tested 12.2.1,
unpinned), rustfmt, Clippy, Python 3, Git and Linux `flock`; native hosting
requires Linux.

`sh scripts/engine-env.sh COMMAND [ARG ...]` selects existing
`~/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/{usr/bin,usr/lib}`.
`MTG_ENGINE_TOOLCHAIN_ROOT` overrides it with an existing absolute bundle; a
missing explicit override fails, an absent default falls back to PATH. The
launcher sets process-local PATH/LD_LIBRARY_PATH, RUSTUP_TOOLCHAIN and
RUSTUP_AUTO_INSTALL=0 and installs nothing. New machines need
[official Node/checksums](https://nodejs.org/en/download/archive/v26.11.1) and,
with rustup, `rustup toolchain install 1.99.0 --profile minimal --component
rustfmt --component clippy`. Preserve user proxies/global defaults; missing
libraries need compatible setup, not copied bare executables.

## Choose focused checks

Build first. Run the cheapest compile check that covers the diff and one directly
affected witness when useful. Docs-only changes need the docs guard and
`git diff --check`. Defer broad regressions, property matrices, end-to-end
qualification and extra reviews unless requested or needed to diagnose a
failure. Confirm the selector executed; skipped and unrun checks are omissions.

Choose a witness for the changed boundary: exact frozen source for rules and
compiler changes, affected harness tests for verification tooling, and a targeted
shape, isolation, atomic rejection or recovery test for those contracts. Prefer
shared invariant cases to per-card matrices; retain counterexamples and use
baselines only for behavior that should be preserved.

Live attempts need a done criterion and deadline before starting. A bounded
witness qualifies only its exercised path, never a full game. Timeout means
inconclusive; retain the record without blind restarts or polling unchanged state.

Run once on stable inputs. Reuse receipts only after comparing tested source,
tests, fixtures, dependencies, configuration and producer identities; record the
bridge and omissions. Changed shared inputs invalidate dependents. Docs, review,
commit/push and unrelated merges alone need no native reruns. Review immutable
commits using base-branch instructions, focused on the diff and unresolved
findings; skip automatic review agents.

After three unsuccessful repair rounds for the same failure, retain reproduction
and attempted fixes, stop that loop and escalate the blocked claim to Josh while
continuing independent work. Never retry until green, add sleeps, hide flakes or
weaken rules/privacy/recovery assertions. Remove tests only with a concrete
retained replacement or a documented obsolete contract. Add a permanent rule only
for a demonstrated miss, with its trigger and check; prune obsolete rules.

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

Prefix with `sh scripts/engine-env.sh`; `--plan` prints commands and omissions
without executing. Every debug command shares one feature shape
(`--workspace --features mtg-core/testkit`) and the root `Cargo.toml` `dev`/`test`
profile, keeping assertions and overflow checks. The production check builds host and simulator without
`testkit`, so dead code stays visible. No plan builds release or runs `mtg-sim`.
Compiler-coverage PRs also run one whole-archive `engine:decks coverage --all`
before publishing and report its family scoreboard. Python suites need explicit
filenames (discovery misses hyphens).

### Engine test topology

Cargo builds twelve test executables: eight library harnesses (view, encoder,
batched environment, local contracts, compiler lowering and catalog
regeneration, private core anchors, simulator policies/search/smoke, server
units) and four integration suites for cross-module behavior.

| Integration target | Scope |
| --- | --- |
| `mtg-core --test rules` | Turns, stack, combat, payment, commander, triggers, kernel and public adversarial inputs |
| `mtg-core --test recovery` | Snapshots, history, query work, search privacy, trace and budget invariance |
| `mtg-witness --test cards` | Runtime witnesses compiled from exact frozen R5 source |
| `mtg-server --test host` | Host protocol, store, authorization and recovery, including the opt-in systemd resume case |

Core's non-default `testkit` feature supplies `FixtureCatalog`, `Board`
(name-based private scenarios) and `Driver` (trusted request/response helpers);
public adversarial modules use only the public API and `core/tests/support`.
Recovery endpoints share `assert_recoverable` (replay, restore, fork and trace at
quanta 1 and `u64::MAX` in every trace mode); mechanic-specific forgery, privacy
and saved-work assertions stay with their witnesses.

The gate runs `cargo test --tests` with one libtest thread budget (minimum of
available CPUs and 16); test helpers must not spawn their own thread pools
(nested pools once reached 13 GiB). Each of the twelve harnesses must report a
passing test. The separate `cargo test --doc` phase requires both core
compile-fail examples (action continuations and player sessions) to pass.

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
never card-name dispatch; keep follow-on admission/policy/consumer qualification
separate unless correctness requires it. Aim for a focused checkpoint within
30 minutes and publish provisional draft PRs with exact passed, failed and
deferred checks. Stage owned files only, disable signing locally (including
scratch Git fixtures) and freeze measured source/index during Git operations. If
publication blocks, preserve failures, patches, bases and source snapshots.
Retire worktrees only through [storage policy](game-engine-storage.md#worktree-lifecycle).

Hand off branch/worktree/base/HEAD, publication and dirty paths, behavior,
commands/selectors/results, identities, receipt locations, bridges, omissions,
live processes/native owner, retained artifacts and remaining work; confirm the
push and separate implementation, verification, build and lock-wait time.
Successors recheck identities and ownership. Pass this policy and relevant
receipts to every delegate brief.

## Batched NumPy environment

Build the native extension with `PYO3_PYTHON=/usr/bin/python3 sh scripts/engine-env.sh cargo build --locked -p mtg-env`.
On Linux, `python3 packages/game-engine/crates/env/python/smoke.py target/engine/debug/libmtg_env.so`
loads that exact library; a module search path containing it named `mtg_env.so`
supports `import mtg_env`. Native PyO3 supports Python 3.14; the runtime requires NumPy.

`VecEnv(catalog_dir_or_profile, pods=None, num_envs=1, threads=32, seed=0, config=None)`
accepts a profile name and its default decks, or pod files selected by
environment index modulo pod count. An embedded-profile pod file contains the
four serialized decklists. A bulk catalog directory requires a `manifest_sha256`
configuration value and dynamic pod files in the simulator's `{"decks": [...]}`
multi-seat format. Episode seeds bind the supplied seed, environment index and
episode ordinal; explicit reset restarts episode zero. Each rules and query call
uses the pod driver's 1024-unit quantum.

```python
env = mtg_env.VecEnv("commander-foundations-v1", num_envs=32, seed=57)
env.reset()
with env.buffers() as batch:
    # Opening declarations use the pointer head.
    actions = batch["ptr_mask"].argmax(axis=1).astype("int32")
env.step(actions)
```

Arrays from `buffers()` borrow contiguous Rust storage. Batch exit or `release()`
drops the batch's references; slices and retained arrays keep a lease, and the
next step or reset raises while any lease is live, so copy tensors to keep them.
Reset and step release the GIL. Each environment owns a persistent thread pool;
worker `w` writes rows whose batch index modulo the worker count is `w`. An
internal fault requires a reset; an invalid input leaves all games at the
previous decision.

`payment` is `auto` (default) or `manual`; spend always uses the exact solver.
`opponents` is `self_play` (default) or `baseline` (seat zero acts, other seats
use Develop). `mask_unfundable` defaults to true: the encoder proves fixed costs
from the unrestricted pool and intrinsic single-color lands. It keeps an
otherwise unfundable candidate with funding code 2 when the pool contains
restricted mana or when sources, payments or visible spell-cost reducers make
funding uncertain. It preserves the original pointers if filtering would empty
the head. `head` selects pointer (0) or number (1). Pointers 0–127 address
ordered candidates, 128 ends a slot or ordering, and 129 and 130 finish
production with undo or keep.
Numbers 0–62 are literal and 63 selects the numeric upper bound, which AnnounceX
can tighten when resources are fully modeled; otherwise the published maximum
applies, and an unbounded decision with unknown supply uses the signed 32-bit
ceiling. The bound never falls below the decision's published minimum.

Entity rows preserve object order, then stack order, then remembered library
positions, then one hand-zone row per publicly known revealed hand card in
published order; remembered positions and known hand cards have no targetable
handle. Missing entity and seat references are -1; card zero is absent identity.
Player features and entity seat references rotate around the viewer; `seat` and
terminal rewards use absolute seat order. A terminal step returns the next
opening observation with the previous episode's reward, done and counters. Info
columns 0–3 count illegal actions, cast reversals, censor class (0 outcome,
1 workload limit, 2 engine capacity, 3 tensor capacity) and completed rules
decisions. Tensor overflow ends the episode with zero reward and a fresh opening
observation. Numeric features saturate at the signed 32-bit limits. The
observation wire has no lore-counter or selected-mode field.

`catalog()` exposes padded 192-token rows, lengths, sixteen numeric features and
the SHA-256 of the sorted vocabulary encoded as compact JSON. Token IDs start at
one; an overlength definition ends with a truncation token. Bind model inputs to
both catalog and vocabulary identity.

## Measurement and deck work

Check output streams subprocess output. Timing JSONL records source/driver
identity, commands, exits, wall/CPU/peak RSS (largest single process), lock waits
and host load; `MTG_ENGINE_TIMING_FILE` selects a create-new path and source
mutation fails. Compare only matching source, workload and cache state; label
cold builds.

`npm run engine:decks -- COMMAND` is the one deck harness, on a release
`mtg-sim` built from this checkout:

```sh
npm run engine:decks -- coverage SIM --out generated/NEW [--baseline OLD/report.json] [--all]
npm run engine:decks -- stage SIM OUT --deck "Jo's Cascade"   # or --decklist FILE
npm run engine:decks -- play "Jo's Cascade" 0
npm run engine:decks -- bench SIM --bulk-artifact DIR --manifest-sha SHA --pod-decks A.json --swap SLOT:REMOVE:ADD --blocks 8 --out generated/NEW
```

Use [rework-sweep.py](../scripts/rework-sweep.py) to compare fixed rework cohorts and source guardrails.
While Stage 1 reads unchanged R5, [rework-reference-diff.py](../scripts/rework-reference-diff.py)
compares `mtg-sim catalog reference-trace` with the frozen resolution, and each
baseline definition's IR with the candidate's. It exits 1 on an unexplained difference.

The script docstring (`scripts/engine-decks.py`) owns options and report
contents. `coverage` joins `catalog coverage` receipts to the deck snapshot
(`--all`: the whole archive) and ranks blockers by family, counting the deck
slots and cards each family touches and alone blocks. `stage` accepts only the
registered Archive R5 file, requires every card to compile and templates the
reviewed `commander-foundations-v1` profile. `play` stages and plays one seeded
match from a detached HEAD under `generated/deck-play.*` (live journals in its
`live-journals/`). `bench` wraps the [paired runner](game-engine-search.md#paired-deck-experiments)
and writes its report and Markdown summary.
