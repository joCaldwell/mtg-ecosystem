# Engine development commands

Start at [the current engine map](game-engine-current.md). This workflow favors
focused verification with explicit omissions; historical release gates are not
mandatory after each edit. Run commands below from the repository root.

## Fresh worktree

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:source
```

Bootstrap checks existing tools, installs locked root/engine/parser dependencies,
then runs doctor. It does not compile Rust, fetch card data, regenerate fixtures,
change Git refs, or install a toolchain. This is a minimal engine install, not a
deck-builder or server SDK install. `npm run engine:bootstrap` is equivalent when
npm is already usable. `npm run engine:doctor` rechecks the environment; failures
return 1, advisory warnings return 0. Add `-- --json` for structured diagnostics.

The installer uses `npm ci` with both workspaces and `--include-workspace-root`,
`--include=dev`, `--ignore-scripts`, `--no-audit`, and `--no-fund`. It may download
locked npm dependencies; it never runs their install scripts. It replaces the
local dependency installation, so do not run it concurrently with another npm
install or in a worktree whose full application install you need to retain.
It leaves package manifests and the lockfile unchanged.
See [npm ci semantics](https://docs.npmjs.com/cli/v11/commands/npm-ci/).

## Toolchain setup

The version authorities are [rust-toolchain.toml](../rust-toolchain.toml) and
[.node-version](../.node-version): Rust/Cargo 1.99.0 and Node 26.11.1 at this
checkpoint. Doctor also requires npm, rustfmt, cargo-clippy, Python 3, Git and
Linux `flock`; npm's detected version is reported rather than pinned. The tested
host npm is 12.2.1. Native host tooling requires Linux.

The launcher first selects the existing user-scoped bundle:

```text
~/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/
  usr/bin/     node, rustc, cargo, rustfmt, cargo-clippy
  usr/lib/     matching shared libraries
```

`MTG_ENGINE_TOOLCHAIN_ROOT=/absolute/existing/bundle` selects another bundle with
that layout. An unavailable explicit override fails; an absent default leaves
the existing PATH available for already-installed tools. The launcher verifies
the exact Node version before executing the requested command. Bootstrap/doctor
verify exact Rust and Cargo versions as well. The preserved host bundle has no
npm executable, so the existing npm on PATH runs under the selected Node.

For a machine without that bundle, explicitly install the pinned Node from the
[official archive and signed checksums](https://nodejs.org/en/download/archive/v26.11.1)
in a user-owned directory, and place its `bin` on PATH. With an existing rustup
installation, an **explicit optional download** is:

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
```

Keep user-owned rustup proxies on PATH, then rerun bootstrap. Do not downgrade
the system toolchain or change the global rustup default. No helper performs
this installation. Missing platform libraries require a compatible existing
bundle or separately arranged host setup; copying only the executables is not
enough.

`engine-env.sh COMMAND [ARG ...]` applies PATH and LD_LIBRARY_PATH to that process
and its children, preserving argument boundaries and existing loader paths.
It reads the Rust pin into RUSTUP_TOOLCHAIN and sets RUSTUP_AUTO_INSTALL=0, so
[rustup will not auto-install missing tools](https://rust-lang.github.io/rustup/environment-variables.html).
It does not edit shell startup files or persist environment variables.

## Choose focused checks

```sh
sh scripts/engine-env.sh npm run check:engine -- --plan
sh scripts/engine-env.sh npm run check:engine:source
sh scripts/engine-env.sh npm run check:engine
```

The default checks tooling/source, formatting, all-target compilation and
compiler/contracts library tests. It omits core/simulator/server runtime tests,
executable contracts, SDK, full demo, profile matrices, release and retention.
Source mode avoids Cargo. Contracts/full modes are opt-ins; see the exact
[verification workflow](game-engine-verification-workflow.md) before selecting
them. Add the smallest affected runtime test for behavior changes. Never report
unrun work as passed, relax a runtime guard, or bless a changed expected outcome
without source/rules review.

For a tiny runtime sanity probe, use the coordinator-wired commands:

```sh
sh scripts/engine-env.sh npm run engine:smoke
sh scripts/engine-env.sh npm run engine:smoke -- --artifact generated/my-new-smoke.json
sh scripts/engine-env.sh npm run engine:smoke:replay -- generated/my-new-smoke.json
```

These use the check driver's native lock, cache and environment controls. See
[the smoke contract](game-engine-smoke.md) for the fixed seedless scenario,
create-new bounded artifact, replay diagnostics and omissions. The small probe
does not replace an affected mechanic test or establish combined acceptance.

New helpers can be checked without compiling native code:

```sh
sh scripts/engine-env.sh node --test scripts/engine-environment.test.ts
```

## Cargo coordination and cache integration

One heavy owner coordinates native compilation. The check runner owns
`~/.local/share/mtg-ecosystem/engine-verification/cargo.lock`; never wrap it in
another acquisition of that lock. Standalone Cargo commands need that lock:

```sh
sh scripts/engine-env.sh flock "$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock" cargo test --locked -p mtg-core EXACT_FILTER
```

Replace EXACT_FILTER with a real relevant test and check that tests actually ran.
The lock parent must already exist (the check runner normally creates it).
Continue source/docs work while the native owner is busy.

The environment launcher uses `scripts/engine-cache.py`: `path` supplies
CARGO_TARGET_DIR without writes and preserves an
explicit override; CARGO_INCREMENTAL defaults to 0. Bootstrap invokes `check`
before installing dependencies. Cache budget checks belong before native builds,
not every source-only command. Older checkouts without that helper retain Cargo's
normal target default. No environment helper migrates, deletes or prunes
caches. The existing `perf-engine-verification-loop/target` stays preserved;
reuse requires an explicit coordinated override. See [cache selection and budgets](game-engine-builds.md#cache-selection-and-lifecycle)
and [storage boundaries](game-engine-storage.md).

Doctor checks free bytes at the worktree and nearest existing target ancestor,
reporting a failure below 256 MiB and a warning below 2 GiB. These are small-install
diagnostics, not native build budgets; the cache helper owns those. Doctor checks
direct locked dependency versions and workspace destinations, not full npm cache
integrity or every transitive module. Git upstream comparisons use local refs
only, without fetching, and never claim the remote is fresh. No raw environment,
remote URL, npm configuration or child diagnostic stream is printed by doctor.

Commit and push coherent checkpoints after relevant focused checks. Preserve
historical source/evidence/worktrees and all failed/interrupted results. The
coordinator integrates changes; publishing a branch is not release acceptance.
Use [the worktree lifecycle guide](game-engine-worktrees.md) for inventory,
checkpoint and cleanup commands; do not improvise historical worktree deletion.
