# Compiler and engine implementation progress

## First scenario verified on October 13 2026

P0 is complete for the [frozen first scenario](game-engine-first-scenario.md).
The initial P1a/P1b slice is implemented and verified: four source definitions
lower to executable IR, a separate fixed-scenario admission check runs, and a
four-seat land-mana / targeted return-to-owner-hand action executes through the
same typed event and continuation paths used by restore, fork, and replay.
One paired complete-source definition remains unsupported. This is scenario
evidence, not a released playable Commander profile or blanket P1 signoff.

The reviewed fixture has seven accepted inputs and thirteen events. The creature
is owned by P1 and controlled by P2; it returns to P1's hand with a new
incarnation. The spell finishes in P0's graveyard. The full input/event sequence
and final semantic hash are checked in `expected-scenario.json`.

Validation completed:

- `npm run check:engine`: formatting, Clippy with warnings denied, 19 Rust
  tests, and 2 Node cross-language tests passed.
- `cargo test --workspace --locked --release`: the same 19 Rust tests passed
  with optimized code and overflow checks enabled.
- Release CLI demo: uninterrupted, restored, cloned, and replayed runs agreed.
- Retained executable and bundle verified successfully from a fresh directory,
  using only retained inputs. Dependency vendoring and a fresh offline rebuild
  are still P2 retention work; this check does not claim them.
- No parser source edits were made in this worktree. Ongoing parser changes in
  the main checkout are separate from these pinned historical inputs.

The tests cover corrupt/unknown interchange, rehashed unsupported semantics,
source block association, exact arithmetic, unsupported configurations,
functioning zones, owner/controller distinction, invalid/stale choices,
payment reversal, whole-spell multi-block target legality, query purity,
pending-input recovery, receipt isolation, and unreachable rehashed snapshots.

## Retained identities

The release uses Rust 1.99.0 on x86_64 Linux, with Cargo.lock and build settings
retained. Build identity hashes engine/compiler/contracts/CLI source files,
reviewed fixtures, Cargo manifests/lockfile, toolchain declaration, compiler
identity, target, Cargo profile, and Rust flags. IR canonicalization is compact
sorted-key JSON; archive verification separately retains JavaScript serialization.

| Artifact | SHA-256 |
| --- | --- |
| Release build identity | `dee3f9efc8da625a8afa6834495f0383e712340c16f61ba31dcdc352247cace8` |
| Definition catalog | `1ecd2712be49d27c411af2ca06de471375a081136e87ef42cd123ca8c1ca466c` |
| Scenario profile/configuration | `3159762e11cd1657add028e49e0e1ca2bfade40fadce1621817ba1407a5be2ea` |
| Final semantic state | `93258caa3c9da37b71fc06e39e64986d97991dc57ae8f993a961b575260f8c32` |
| Replay bundle | `788a5a91a3aecd8abae40451ab2374fadaa53fc6013422687fbcfe1e864143e6` |
| Retained executable | `2f5008d31ff3e7a2d6bb49693f454ca51fdf659950cfdad65902522e6f983a88` |

The bundle and executable are retained outside the worktree under
`~/.local/share/mtg-ecosystem/engine-evidence/p1/`, named by their content hashes.
The bundle includes complete selected source records, IR, source/build closure,
configuration, accepted decisions, initial/announcement/final snapshots, and
expected results. Generated reports are additionally in `generated/` locally.
Small reviewed source/IR/trace fixtures remain in version control.

Initial release observations on the frozen Ryzen 9 9950X machine were 429 ns
mean clone-and-dispose, 8 ns mean priority-request query, and 467,223 ns across
the four equivalent tiny runs. These single-run calibration observations
include a three-object state and do not establish Commander throughput, memory
capacity, p95 latency, or search performance.

## Remaining delivery gates

P2 must add bounded/resumable execution and queries, permitted observations and
visibility handles, SearchContext, general recovery invariants, and dependency
closure retention/offline rebuilding. Recovery currently replays the small
scenario to prove reachability, and the core's authoritative getters are for
trusted orchestration; they are not a player-facing observation API.

P1 contracts also need refinement as later slices arrive: broader source-AST
schema export, complete printing/membership indexes, structured build/admission
reports, and additional mechanic/functioning-zone descriptors. No parser rewrite
or port of the entire TypeScript binding analyzer is required.

P3 expands mana-payment staging, stack responses, permanent casting, phase
progression, and resolution-time choices. P4 adds the Commander lifecycle and
event interactions. P5 validates full legal decks/pods, a closed reachable pool,
and seeded complete games. Unsupported runtime paths currently fault or reject;
legal-match mode is rejected. Random operations, search, server, and UI remain
outside this first slice. No product decision currently blocks P2 work.
