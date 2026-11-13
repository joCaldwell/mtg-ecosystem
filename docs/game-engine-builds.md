# Engine build inputs and cache policy

`mtg_contracts::CURRENT_IR_VERSION` is the sole IR authority, fixed at **1**
while there are no external consumers. Validation and generated schemas accept
exactly that value. There is no minimum-version range, feature introduction gate,
or historical decoder compatibility. Add supported semantics in place with strict
capability and shape validation. Revisit versioning when a consumer needs an
explicit compatibility contract.

All active catalog manifests and expected outputs use schema 1. The distinct
catalogs remain source-qualified admission pools, not schema editions. Regenerate
their outputs and canonical digests through the compiler when the IR changes;
never preserve a second expected output just for an older schema. Source records,
parser archives, snapshot/bundle/public protocol versions, Git history and external
historical evidence have separate purposes and retain their own identities.
Exact build and catalog identity fence saved-state recovery. This policy does not
expand acceptance status or reviewed hosting/search capabilities.

The schema-1 reset regenerated all 29 active pools through the compiler and
verified their 1,357 definitions and source memberships were unchanged. Focused
checks passed: all-target compilation, 124 compiler/24 contracts/36 selected core
tests, one source-coverage and one hosted-registry test, 40 Node contracts,
54 SDK tests with strict TypeScript, and explicit smoke replay. Full runtime,
full-game, release-test and retention matrices were omitted. Raw receipts and
pool-equivalence evidence are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/single-ir-*`.

Run `python3 scripts/engine-structure-check.py` without Cargo. It checks the
contracts/compiler/core dependency boundaries, including target-specific tables,
workspace inheritance and aliases. Core's compiler dependency is permitted only
for development tests. New dependencies require explicit review of the small
allowlist. This checks declared direct dependencies, not arbitrary Rust code or
all transitive feature combinations.

The same command checks the Git index for generated/cache paths and recognizable
ELF/Wasm/SQLite output anywhere. It catches force-added ignored files and reads
the staged bytes, even when the working copy differs. There is no blanket size
limit for source or reviewed fixtures. Renamed JSON output outside known output
directories cannot always be distinguished from legitimate source; review still
matters. The command never unstages, rewrites, or deletes files.

Focused helper tests: `python3 scripts/test-engine-structure.py` and
`python3 scripts/test-engine-cache.py`. Execute these files directly: unittest
discovery skips hyphenated module names and can misleadingly report zero tests.

## Cache selection and lifecycle

After selecting the pinned toolchain, `python3 scripts/engine-cache.py path`
prints one absolute path, with no mkdir or mutation. Explicit `CARGO_TARGET_DIR`
wins. Otherwise the default is
`$XDG_CACHE_HOME/mtg-ecosystem/cargo/<repo>/<rustc>` (default cache home
`~/.cache`). Repo means SHA256 prefix of the canonical Git common-directory path;
rustc means SHA256 prefix of `rustc -vV`, including compiler/host identity.
Worktrees of the same repository/toolchain share the location. Separate clones
do not. Cargo still owns its normal target/profile/flags fingerprints.

`python3 scripts/engine-cache.py status` reports allocated bytes, filesystem free
space, explicit/default selection and budgets. `check` reports the same JSON and
fails before compilation if the cache exceeds 16 GiB or free space is below
4 GiB. Override deliberately with `ENGINE_CACHE_MAX_GIB` and
`ENGINE_CACHE_MIN_FREE_GIB`. This is a preflight guard, **not a hard disk quota**:
a single build can grow past the budget. Inspect again after heavy work.
The environment launcher defaults `CARGO_INCREMENTAL=0`; explicit settings win.

Native checks must hold the existing shared Cargo lock across budget preflight
and compilation. Never wrap a gate that already owns that lock in another
acquisition. Cache path/status operations do not acquire the lock or mutate data.
No automatic eviction, old-cache relocation, executable removal or evidence
cleanup occurs. Existing external/per-worktree targets remain explicit overrides.
The preserved `perf-engine-verification-loop/target` measured about 22 GiB during
this wave: its coordinated reuse sets `ENGINE_CACHE_MAX_GIB=32` explicitly.
Selecting the new stable default instead leaves those bytes in place and can
require a cold compile; cross-worktree reuse is not guaranteed to avoid rebuilding
workspace crates because their absolute source paths differ.
Cargo's [local-source freshness checks](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html)
use modification times. Shared-target reuse across worktrees has produced stale
local crate artifacts when switching source baselines. If a build contradicts its
local IR authority, preserve the failure and refresh only that owned worktree's
relevant source/build-script timestamps, verifying byte hashes are unchanged,
then rebuild under the shared lock. Do not delete the shared target to repair it.
Check compiled source identity and exported schemas; a warm cache alone is not
proof of the selected source.

For reviewed incremental-only cleanup use the existing engine storage tools and
their coordination rules; do not treat retained proof executables as disposable.

## Invalidation and identity

Compiler build inputs are catalog manifests, provenance/accepted-archive lists,
record groups, and their referenced exact records/archives. The compiler watches
each input path, including absent inputs that may be repaired, plus the catalog
directory for new manifests. Core watches catalog/profile directories and every
data file it reads. Banned-list discovery follows selected profiles; an unused
new policy file cannot silently make cold/warm generated output disagree.
Profile edits discover new policy paths. No card facts or expected rules results
change.

Cargo directory watches recurse, so watching the entire fixture root previously
invalidated both crates for a README edit. See the
[Cargo build-script contract](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed).
The broad engine **source** watch and `engine_build_input` identity remain intact.
The core executable still hashes all engine Rust source, manifests, lock/toolchain
files and effective compilation settings. A simulator edit therefore still
invalidates core. We did not weaken release evidence identity or introduce a
separate development identity merely to avoid that cost.

Measurements on 2026-11-09 used Rust 1.99.0, `CARGO_INCREMENTAL=0`, serialized
`cargo check --locked -p mtg-core -p mtg-compiler`, and the existing
`perf-engine-verification-loop/target` cache. They are single observations, not a
statistical benchmark or runtime performance claim. Baseline was `c62777b`;
changed-source measurements include this branch's version/watch changes.

| Probe | Before | After |
| --- | ---: | ---: |
| Warm no-op | 0.029 s | 0.031 s |
| Fixture README comment | 5.994 s | 0.031 s |
| Restore fixture README | 5.955 s | 0.032 s |

Afterward an archive-header whitespace edit still rechecked compiler (0.893 s);
a simulator-source comment still rechecked core (6.083 s). All temporary edits
were restored. Initial cache priming was 8.088 s before and 7.862 s after;
those timings are affected by source/tool fingerprint changes and are not a
claimed cold-build speedup. Raw logs/measurements are ignored under
`generated/platform-build-measurements/` in the implementation worktree.
No release build, gameplay matrix or retention rebuild is implied by these checks.

Final focused validation also passed 97 compiler and 12 contracts library tests
(26.480 s including 26.19 s compilation; test bodies 0.26/0.02 s), seven Python
helper tests, the structural guard, formatting and whitespace checks. A final
core/compiler check after the selected-policy discovery refinement passed in
7.969 s; warm no-op was 0.0333 s, README edit/restoration 0.0317/0.0313 s, and
addition/removal of an unused policy file 0.0310/0.0305 s, with no crate rebuilds.
These checks omit core/server/simulator runtime tests, SDK, release, full corpus
coverage and retention. No previous failed evidence was relabeled as passed.
