# Cards-as-data refactor plan

This plan replaces curated P6 slices as the engine's next track. Josh chose it
on 2026-10-24 after the audit in
[game-engine-cards-as-data-audit.md](game-engine-cards-as-data-audit.md).

## Goal

A newly printed card that uses mechanics the engine already supports needs no
engine code. It needs only data: its archive record, plus a deck list that
contains it.

A card that needs a new mechanic needs one generic capability. That capability
is measured by how many corpus cards it unlocks, not by one card.

## Decisions

- **Rules come from the catalog.** Behaviour is decided by the IR capabilities
  a catalog uses, not by profile identity or lineage. A rule that applies to
  one catalog applies to every catalog that uses the same IR.
- **Old profiles may be re-baselined.** Earlier catalogs, fixtures, snapshots
  and recorded batch outputs may be regenerated under the new design. Old
  evidence stays preserved under `generated/`, but byte identity with
  historical outputs is no longer a requirement.
- **Safety stays.** Every engine fault stays a typed, serializable outcome, and
  overflow keeps atomic rollback. A capacity limit may become generic, but a
  fault must never replace a rules-defined outcome silently.
- **Card facts come only from archive records.** Records are pinned by archive
  manifest and per-record SHA-256 hashes, never by compiled-in constants.

## D1 — Admission and catalogs as data

The end state of D1 is that adding an already-lowerable card is data-only.

### D1a: archives, catalogs and generation

- **Accepted archives.** Replace the compiled-in archive manifest pin with an
  accepted-archives data file. Each manifest line records its file hash, parser
  revision and corpus hash. Each fixture record names its archive. Pin the
  preserved current archive,
  `~/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl`,
  alongside the historical one.
- **Catalog manifests.** A catalog is a data manifest listing record files.
  Replace the `include_str!` arrays, the per-group "every record needs vN"
  invariants and the hard-coded counts in tests and error text.
- **Generator.** Add a generator CLI that compiles a catalog manifest to its
  expected IR and digest. Tests then compare against generated output instead
  of hand-maintained constants.
- **Record selection.** Add a tool that extracts records by oracle ID or name
  from an accepted archive into fixtures, with provenance.

### D1b: profiles, behaviour and capacity

- **Profiles as data.** A profile is a data file containing:
  - a catalog manifest;
  - decks: oracle ID and count;
  - the commander rules: the color-identity check and a banned-list reference;
  - hosting and search flags.

  `MATCH_PROFILES` becomes a loader over these files.
- **One banned list.** Keep a single dated Commander banned-list data file.
  Pool review is derived from it.
- **Remove lineage gates.** Delete `profile_at_least`, `*_enabled()` and every
  profile-name branch in core, server and sim. Where behaviour differs between
  catalogs, the catalog's capabilities decide it. Remove instructions that
  silently do nothing; an unsupported shape is rejected when the catalog is
  validated.
- **Generic capacity.** Replace the per-profile capacity chains with one
  mechanical policy. Compute capacities from catalog and deck features, or use
  one generous cap with typed faults. Document the rule once.

  Per-profile hand proofs are removed. A bounds argument may remain as a
  benchmark note.
- **Scope `BUILD_ID`.** It identifies engine code only. Matches and snapshots
  pin catalog digests separately, so adding cards does not invalidate unrelated
  snapshots.

### D1c: simulator

- **One batch command.** A single generic `match-batch <profile>` and
  `match-replay <profile>` replaces the per-profile subcommands.
- **Generic completion policies.** Base them on IR features: removal targets
  the highest-power opposing creature, burn targets the lowest life, and so on.
- **Remove card-specific code.** Delete card-name branches and the
  definition-count fingerprints. Card-specific heuristics are allowed only as
  optional data keyed by IR shape, never by name.

### D1 exit

- Re-baseline every current profile under the new system, regenerated from
  data. All tests pass.
- Add the audit's three cards (Canyon Minotaur, Open Fire, Flesh to Dust) plus
  at least ten other already-lowerable cards from the current archive. Use a
  new profile data file, with zero changes to `.rs` or `.ts` files.
- Run a batch with 0 faults, and add a per-capability witness for any
  instruction the batch did not reach.
- `npm run check:engine` and SDK verification pass.

## D2 — Generic IR and compositional compiler

The end state of D2 is that coverage grows by capability.

- **Generalise the IR.**
  - Replace `ObjectSet` with permanent filters plus relations.
  - Add a general `Duration`.
  - Merge duplicate encodings, such as the destroy variants and
    `DestroyAllCreatures` versus `MoveSet`.
  - Replace the one-template variants with generic operations:
    - for-each amounts over event results (Fumigate);
    - replacement effects with event filters (Anger);
    - cost modification (Blasphemous Act);
    - "that player" relations (Massacre Wurm);
    - damage-doubling replacement (Furnace of Rath).
  - Remove the literal subtype and shape restrictions from validation.
- **Compositional compiler.** The compiler lowers filters, amounts, players,
  zones and conditions by composition, replacing the 53 `json!` equality
  matches.
  - Widen the type lines it accepts: artifact creature, legendary and
    enchantment creature.
  - Classify multi-face cards as Unsupported, not Invalid.
  - Fix the source-line normalisation mismatch.
- **Coverage metric.** Commit a corpus coverage report generator. It reports
  the share of Commander-legal records that lower and validate, plus the top
  rejection reasons. The baseline is 5.35% at IR v20.

  Each D2 step records its coverage delta. The committed command, D2a baseline,
  deltas and verification are in
  [the coverage report](game-engine-cards-as-data-coverage.md).
- **Capability witnesses.** Correctness is shown per capability, not per card:
  - reviewed rules witnesses for each new or merged instruction;
  - a randomized check that samples lowered corpus cards into legal decks and
    runs batches with zero faults.
- **Order of work.** Take the top rejection families from the report first:
  keywords in the compiler mirror, phase, cast, attacks and deals-damage
  triggers, gain-abilities and enters-tapped. P6.11's card-flow contract
  becomes a capability step here: phase, cast and draw triggers, scry,
  look-at, and discard costs.

## Process

Each step is implemented in a worktree, independently reviewed and then
merged, as before. A review checks for:
- card-identity logic;
- profile-gated behaviour;
- template-only IR;
- weakened fault handling.

Verification stays proportional. It covers the engine check, SDK
verification, the batch and coverage numbers. No new audit frameworks are
added.
