# Cards-as-data corpus coverage

D2a measures complete-card lowering and IR validation, not Commander deck
admission or complete rules support. Every count uses the record's own
`source.legalities.commander == "legal"` value.

Run from the repository root with the pinned engine toolchain:

```sh
T=/home/josh/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/usr
export PATH=$T/bin:$PATH LD_LIBRARY_PATH=$T/lib
cargo run --locked --release -p mtg-sim -- catalog coverage \
  /home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl \
  --out generated/cards-as-data-coverage/d2a/final
```

The accepted-archives data must pin both the exact manifest and full archive
SHA-256. Unknown archives and changed bytes produce typed errors. No manifest
substitution occurs. The input is read-only. Output defaults to
`generated/cards-as-data-coverage/` if `--out` is omitted.

`summary.json` reports all-record and Commander-legal totals, the legal lowered
count/percentage, legal status counts and descending rejection counts (ties by
reason then status). The top 30 reasons include up to five distinct card names,
sorted alphabetically. `records.tsv` classifies **every** archive record, including
nonlegal cards; its columns are oracle_id, name, status, reason. Backslash, tab,
CR and LF are escaped. Summary counts include only legal records.

Statuses are `lowered_validated`, `unsupported`, `invalid`, and `unparsed`.
Reasons identify an AST discriminator (`trigger:phase`, `effect:gain-abilities`,
`keyword:ward`) or a compiler field path and specific boundary. Field-path
reasons normalize diagnostic punctuation to underscores; serde locations and
expected-variant lists are excluded. These codes describe the first failed
boundary, so fixing one can expose another without admitting a card. Unparsed
means the archive does not contain a complete successful card parse. Unknown
semantics are always rejected for the entire card.

## D2a baseline

Measured on base `317e759` with the committed command, before intake fixes:
38,721 records; 32,116 Commander-legal; **1,719 lowered and validated (5.35247%)**
at IR v20. Legal statuses: 24,694 Unsupported, 3,222 Invalid, 2,481 unparsed.
This reproduces the audit's 1,719 result without substituting a header: D1a
already accepted the current archive through data. Accepted-archive fix delta: 0.

Archive SHA-256:
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
parser revision `327b3360ea8e5cffb75b136223e75a348e903cbe`.
Raw baseline: `generated/cards-as-data-coverage/d2a/00-baseline/` (ignored).

## D2a intake measurements

Each output directory contains the full TSV and JSON at that stage. Deltas
are sequential and count newly lowered **and validated** legal records.

| Stage | Lowered | Change | Percent |
| --- | ---: | ---: | ---: |
| Baseline (`00-baseline`) | 1,719 | — | 5.35247% |
| Source normalization (`01-normalization`) | 1,719 | +0 | 5.35247% |

The source adapter now uses case-sensitive Unicode word boundaries, punctuation
canonicalization, full and source-established legendary short names in all
clauses, guarded source pronouns, balanced reminder removal, and complete
modal/dice blocks. Explicit literal-name quotation is canonicalized on both
sides without dropping any name or surrounding text. The resulting comparison
is exact. AST hashes, source hashes, line/aggregate equality and unknown-field
checks remain mandatory for every successful compilation.

Unknown ASTs are rejected before normalization: unsupported station blocks and
self-replacement paragraphs do not become false source-integrity failures.
This changes the first reported boundary, not successful admission. The 2,229
original source-line mismatches and 227 orphan modal-header failures now reach
Unsupported reasons. Legal Invalid falls from 3,222 to 767. There is no coverage
increase because those records still require unsupported semantics.

Multi-face classification (`02-multi-face`): **1,719 (5.35247%), +0**.
All 766 legal records formerly failing single-face required fields now report
Unsupported: transform 341, adventure 147, split 113, modal_dfc 83, prepare 66,
flip 16. Source and binding hashes are checked before layout inspection; absent
single-face fields are never filled with defaults. One legal Invalid remains.

Type-line composition (`03-type-lines`): **1,820 (5.66696%), +101**.
`Basic` and `Legendary` are parsed independently of card types and subtypes.
A canonical execution kind (creature, then land, then artifact, then enchantment)
plus sorted `additional_types` retains all supported simultaneous permanent
types. Empty additional types are omitted, preserving existing IR/digest bytes.
The IR remains v20. Combined types add the corresponding existing capabilities.
Removal's positive/negative type predicates, spell-counter type predicates,
sacrifice filters and enchantment wipes all use complete type membership.
Creature casting, combat and summoning sickness use the canonical creature kind.
Legendary noncreature permanents and inert permanent subtypes no longer hit
closed presentation whitelists; their entire AST still has to lower.
Planeswalkers, battles, unsupported card types/supertypes (including Snow and
World), mixed spell/permanent types and land-creatures retain explicit failures.
No new keyword, trigger, snow-cost, world-rule or planeswalker behavior is added.
All 68 compiler tests passed, including exact freshness of every committed
catalog: no expected IR or digest needed regeneration.
