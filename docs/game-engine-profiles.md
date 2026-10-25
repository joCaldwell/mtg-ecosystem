# Match profile data

Legal match profiles are strict JSON files discovered under
`packages/game-engine/tests/fixtures/profiles/` at build time. Core embeds the
reviewed inputs and performs no filesystem I/O. Adding a file needs no registry
or Rust selector edit. Fields are:

- `name`: unique admission identity.
- `catalog_manifest`: relative reference to a catalog manifest in `catalogs/`.
  The manifest determines the IR version, complete catalog and canonical digest;
  its expected digest and source-regeneration checks remain enforced.
- `decks`: four default decks, each with `commander` and `cards` entries containing
  `oracle_id` and positive `count`. The commander is included in the 100 cards.
- `commanders`: the supported commander Oracle identities, expressed as data.
- `commander_policy`: `color_identity: "commander_subset"`,
  `eligibility: "legendary_creature"`, and `banned_list`, a relative reference
  to the shared dated Commander ban data. Unknown policy values reject.
- `hosted` and `search`: independent admission flags. Host admission still validates
  the complete pod and pins both executable identities. Search still checks its
  exact catalog digest, fresh root shape and information/reconstruction contract.
- `completion`: deterministic simulator suite layout, with `rotate` and two
  `homogeneous_seats` indices for cases 16 and 17. This preserves the old
  four-colors suite without a profile-name branch; it does not select rules.

Every deck still undergoes 100-card construction, singleton-name, commander
count, eligibility, color-identity and basic-land mana checks (CR 903.3–903.5).
The profile's complete serialized admission data, canonical catalog digest,
shared banned-list digest and normalized private match configuration bind the
pod identity. Fixture changes stay outside engine code `BUILD_ID`.

`commander-banned-2026-10-15.json` preserves the official source URLs, retrieval
date, HTML hashes and complete hashed archive records for 42 named bans and
seven offensive-card exclusions. Lutri's companion-only category stays distinct.
There are no per-profile ban or pool-review copies. Pool review is derived from
each source record's Commander legality, type line and Oracle text, and membership
in the single named/offensive list. Compiler source/provenance checks establish
record authenticity; build-time checks and cross-language tests establish these
admission exclusions for every selected catalog.

Use `mtg-sim match-batch PROFILE OUTPUT` and `mtg-sim match-replay PROFILE FILE`.
Batch generation and replay both obtain the profile and catalog from data;
replay rejects a report for a different profile. The existing Develop policy
and its card-specific heuristics remain D1c work. Historical reports retain
their original source, catalog, policy and build qualifications.
