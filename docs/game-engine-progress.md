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

## Budget and observation slice verified on October 13 2026

The [bounded P2 slice](game-engine-p2-slice.md) is implemented. Execution uses
explicit work budgets and suspends without inventing a game outcome. Observation,
decision, response-validation, and SearchContext queries resume from actual row
or stage cursors. Live work and receipts bind to executor and exact state epoch;
automatic execution invalidates them even when the accepted-input revision stays
unchanged. Recovered query work recomputes its saved prefix before rebinding.

Player-facing types expose own hands, public objects, and hidden-zone counts,
using viewer-specific handles that expire on movement or lost visibility.
Observation history and handle allocation happen during deterministic transitions,
never queries. SearchContext contains permitted observations/history, open deck
compositions, public definition/profile identity, and a caller-supplied sampling
seed. Raw internal IDs, game-state hashes, snapshots, frames, and game seeds are
absent from these projections. The sampler remains future work.

Snapshot version 2 restores pending inputs and suspended payment/resolution
frames at the recorded rule-step boundary. The original seven inputs and thirteen
events remain unchanged. The new state hash includes persisted visibility and
the ten deterministic rule steps; it is recorded separately from the P1 hash.
Library, command, and exile zone types are represented without admitting their
new gameplay. Legal-match mode remains rejected.

Validation completed:

- `npm run check:engine`: formatting, Clippy with warnings denied, 25 Rust tests,
  and 3 cross-language/CLI projection tests passed.
- `cargo test --workspace --locked --release`: all 25 Rust tests passed.
- Execution budgets of 1, 2, 3, 7, and unlimited units produced equal final
  states/events. Every pending/suspended boundary restored exactly.
- Query checks covered purity, zero-budget incompleteness versus complete empty
  domains, stale/foreign work, scoped/stale handles and decision tokens, receipt
  invalidation, tampered recovery scratch, and equal permitted contexts despite
  different hidden internal IDs.
- `npm run retain:engine`: retained 1,581 dependency files and native source
  closure, rebuilt with `cargo --frozen` in an initially empty Cargo home and
  target directory, and verified replay plus the observation-query sidecar with
  the rebuilt executable. Preparing the vendor closure fetched a locked
  target-specific dependency; the independent rebuild was offline.

Current release identities:

| Artifact | SHA-256 |
| --- | --- |
| Release build identity | `317ef0d4de787ede292276b47c2b1f6843715f13516ad271af14ff5c03d12772` |
| Final version-2 state | `ab6e126940e3af4e4847f2e9afdba7a788c9efc388b969faf96b4b678e7aa60f` |
| Replay bundle | `5af01a788e07ac3e8310d641308d2385b99048e8fe8d3f5fe27d81c96185a3a4` |
| Rebuilt executable | `ac20768f94483477d024258dafa43c9d491224c24d21b1019e20e656a11d08c4` |
| Dependency file manifest | `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f` |

The catalog and scenario-profile digests are unchanged. Current evidence,
sources, vendor files, and rebuilt executable are retained in
`~/.local/share/mtg-ecosystem/engine-evidence/p2-5af01a788e07ac3e-B1D47x/`;
`evidence.json` records every dependency file hash and verification result.
The P1 bundle and executable remain separately retained for their original build.

Tiny-scenario release observations: 6,061 ns mean clone-and-dispose; 256 retained
branches increased process RSS by 12,296,192 bytes; version-2 snapshots encoded
32,772 bytes. These include the added four-viewer history/knowledge state and
do not establish representative Commander throughput, memory capacity, or search
latency. Copying retained histories and journals still grows with run length.

## Commander and departure follow-up — October 13, 2026

The [frozen departure brief](game-engine-departure-slice.md) is implemented for
both exact scenarios. IR v2 adds an explicit legendary characteristic and the
source-verified Isamaru fixture; complete source/AST hashes still use archive
schema 3. The catalog contains five reviewed definitions and one paired
unsupported source record. Snapshot/bundle v3 retain original seats, absent
active-player state, last-known departed-player facts, default/control-effect
provenance, a physical Commander ledger, and a mixed decision/external journal.
Observations and SearchContext v2 expose only permitted commander designation
handles and public history, without authoritative physical IDs.

Concessions invalidate work/receipts immediately and run before ordinary steps.
Tests cover owned objects leaving, default-controller exile, ending grants,
static-source versus persistent grants, live priority and prior passes, casting
reversal before payment, source departure, target removal before/during
resolution, final-survivor victory, budget partitions, and forged recovery.
Command-zone tax and per-commander combat history have exact domain checks;
combat accumulation is test-only until the damage interpreter is implemented.
Neither admitted scenario casts its commander or deals damage.

Validation: `npm run check:engine` passed formatting, Clippy, **38 Rust tests**
and **3 Node cross-language checks**. `cargo test --workspace --locked --release`
passed the same 38 Rust tests. `npm run retain:engine` rebuilt with
`cargo --frozen` using 1,581 vendored files, an initially empty Cargo home, and a fresh
target directory. The rebuilt executable verified both mixed journals, all
thirteen departure checkpoints, and the query recovery sidecar. The original seven decision inputs and
thirteen events remain byte-for-byte equivalent as JSON values. The current
baseline state hash is
`862f13d9e8f38ce272b7664f672875395cd522d2a6481a1143c3e581c49f8778`.
The departure demo uses five decision inputs and three concessions, verifies
thirteen exact recovery checkpoints, and ends with P1 winning. Its state hash is
`5aa8dca40556c64ca229a4c091fac00cce9faf60413da89fcd9ac0c985a17998`.
Reviewed trace/state witnesses are in `expected-departure-scenario.json`.

Current native release retention:

- Build: `91db7bf35112686e578fa931607a2c851e9affcab369167feeb7df4f1b868cfd`.
- Catalog: `7ca94c66fd9c8d1884fa13ccbd5066f0d62c2ed08c0c67d741f4b260d6b92a66`.
- Baseline profile: `758cade50b263c744cd7dc5db08f20685496f73f82adacb8a57da8dfff942272`.
- Commander profile: `87fca35a63ff9ccc5e22a48ae9c71d92ee6c3c8b0ae4766206bc80d7ebef4588`.
- Bundle: `68b7847d11c977f3593f92b36973d214bb06250ace8738a12db67f2971e86e16`.
- Dependency manifest: `387384711d404f44993e8ab583e269c5105d1cc8b583307e3035f14e03d7474f`.
- Executable: `2cba0516b0a69f85a1a36475feeea8209219096c2abad656a17ded4e8f6dcdb7`.

Retained closure: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-68b7847d11c977f3-VMkaSX/`.
The evidence manifest is `evidence.json` there and
`generated/p2-retention.json` in this worktree. Generated artifacts stay ignored.
For this tiny scenario, 256 retained branches increased RSS by 13,217,792 bytes
and snapshots encoded 38,037 bytes. These observations are not representative
Commander performance or memory guarantees.

## Remaining delivery gates

Remaining P2 foundations include additional event compositions and trace modes,
and wider recovery invariants. Commander designation/history and departure
foundations are implemented in the bounded follow-up described above; production
command-zone casts and actual combat damage remain later mechanics.
Recovery still replays the small scenario to prove reachability. Rules work units
are not wall-clock or process-memory guarantees; larger profiles require measured
limits and finer resumable work. Core authoritative getters remain trusted host
APIs; player-facing consumers use the new projection/query types. This slice is
not full P2 signoff.

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
