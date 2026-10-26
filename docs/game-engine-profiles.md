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
  to a dated Commander ban data file. Unknown policy values reject.
- `hosted` and `search`: feature requests. Build-time typed validation rejects requests whose
  catalog requires capabilities outside the explicit reviewed mechanic sets in
  `core/src/profile_data.rs`. Search's set records the mechanics of its proven
  catalog; hosting's set records the union of the reviewed hosted mechanics.
  Names and digests never grant a feature. Host admission still validates the
  complete pod and pins both executable identities. Search also verifies catalog
  integrity, fresh root shape and the information/reconstruction contract.
- `completion`: deterministic simulator suite layout, with `rotate` and two
  `homogeneous_seats` indices for cases 16 and 17. This preserves the old
  four-colors suite without a profile-name branch; it does not select rules.

Every deck still undergoes 100-card construction, singleton-name, commander
count, eligibility, color-identity and basic-land mana checks (CR 903.3–903.5).
The profile's complete serialized admission data, canonical catalog digest,
selected banned-list digest and normalized private match configuration bind the
pod identity. Fixture changes stay outside engine code `BUILD_ID`.

`commander-banned-2026-10-15.json` preserves the official source URLs, retrieval
date, HTML hashes and complete hashed archive records for its named bans and
offensive-card exclusions. Lutri's companion-only category stays distinct.
All `commander-banned-*.json` files in the fixtures data directory are embedded
at build time; each profile selects its list by relative reference. Dates and
source metadata live in those files. A new dated list needs no Rust edits.
There are no per-profile ban or pool-review copies. Pool review is derived from
each source record's Commander legality, type line and Oracle text, and membership
in the profile-selected named/offensive list. Compiler source/provenance checks establish
record authenticity; build-time checks and cross-language tests establish these
admission exclusions for every selected catalog.

Use `mtg-sim match-batch PROFILE OUTPUT` and `mtg-sim match-replay PROFILE FILE`.
Batch generation and replay both obtain the profile and catalog from data;
replay rejects a report for a different profile. The Develop policy reads public IR target domains, filters, instruction amounts
and costs, plus permitted board observations. No profile or card identity selects
strategy. One generic interaction summary lists instruction-site coverage and
unused catalog cards; see [D1c verification](game-engine-cards-as-data-d1c.md). Historical reports retain
their original source, catalog, policy and build qualifications.

`match-game PROFILE INDEX` runs one completion game;
`match-replay-live PROFILE FILE` verifies a live journal and rejects a profile
mismatch. `catalog build MANIFEST [--write]` replaces the old per-catalog
`compile-*` commands. Malformed profile JSON, duplicate names, invalid completion
indices and unsupported feature requests fail the build with the input path.

`Definition::requirements` exhaustively maps enum variants to capabilities, but
Rust exhaustiveness does not cover new IR struct fields. Whenever an IR field
is added, the reviewer must re-check this capability mapping and the hosted and
search capability sets before admitting catalogs that use the field.
