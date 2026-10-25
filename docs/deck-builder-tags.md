# Deck-builder tag system audit

Audited 2026-10-11 against the working deck-builder and spec §5.4.

## Interaction

The card panel has a searchable checkbox dropdown. Only assigned tags appear
below it, as removable chips; an untagged card displays “No tags assigned.”
The dropdown stays open while selecting or deselecting several tags. Async
writes temporarily disable choices and preserve keyboard focus. The returned
server composite is authoritative; failed writes leave the existing selection.

The deck toolbar has a separate tag filter, initially empty:

- **Any tag:** union of the selected tags.
- **All tags:** intersection of the selected tags.
- Name search and tag filtering compose with AND.
- Commanders, companions, regular cards and the maybe list all participate.
- Proposed additions match tags already assigned to the card in the deck or
  maybe list; a new, untagged proposed card does not match a tag filter.
- Group counts, targets, prices and review decisions retain full-deck scope.
- Removing a tag from the vocabulary removes its active filter and card links.
- Selected cards can have a tag added or removed through the bulk Tag menu.

## Data coverage

| Concern | Representation and behavior | Verification |
| --- | --- | --- |
| Vocabulary scope | `tags(id, deck_id, name)`; each deck owns its vocabulary | Service scoping and database foreign keys |
| Identity | Stable integer ID; renaming keeps all assignments | Rename regression test |
| Names | Case-insensitive uniqueness, reserved-name validation, near-duplicate validation on both create and rename | Deck service tests |
| Multiple tags | `deck_card_tags(deck_id, oracle_id, tag_id)`; unique composite primary key | Replacement and deduplication tests |
| Same-deck integrity | Composite FK `(deck_id, tag_id)` → `tags(deck_id, id)` | Direct invalid SQL insert tests |
| Card membership | Composite FK to `deck_cards`; tags require a deck-card row | Direct invalid SQL insert and cut tests |
| Quantities | Tags describe the oracle-card row, so every copy in that row shares the tag set | Filtering preserves the row and its quantity |
| Slots and roles | Independent fields; commanders and companions can have tags; slots remain optional | Filter tests across all sections |
| Maybe list | Uses the same deck-card row and join table | Parking, promotion and fork lifecycle tests |
| Single-card writes | Complete tag-set replacement, validated and transactional with slot filing | Invalid IDs, empty sets and rollback tests |
| Agent writes | `file_cards` adds/removes tags by name in one transaction and records Activity; organization needs no membership approval | Existing MCP and bulk-filing tests |
| Queries | UI has any/all tag filters; existing deck-context search supports `tag:` and boolean combinations | Domain filter and existing search tests |
| Deletion | Tag/card/deck deletion cascades to assignments; deleting one deck does not affect a fork's vocabulary | Cascade and lifecycle tests |
| Fork | Copies vocabulary and remaps assignment IDs for regular and maybe cards | Fork and lifecycle tests |
| Undo | Cut snapshots store tag IDs; undo restores surviving tags and skips deleted ones | Membership and lifecycle tests |
| Migration | Schema 2/3 → 4 preserves valid assignments and adds the composite FK; invalid legacy links abort the transaction | File-backed migration and rollback tests |
| Lookup performance | Assignment PK handles card lookups; `(deck_id, tag_id, oracle_id)` index handles tag lookups | Schema definition |
| Display state | Filter IDs and any/all mode live in `DeckUi`, separate from server assignments; deleted IDs are pruned | Browser deletion check |

## Gaps corrected

1. The card panel previously showed available and assigned tags together with
   subtle checkmark/border differences. It now shows assigned tags separately.
2. The deck list had no tag filter. It now supports unions and intersections.
3. Bulk tagging could add a tag but could not remove it. Both are available.
4. Renaming bypassed the near-duplicate rule enforced on creation. Both paths
   now use that rule.
5. Direct slot-and-tag filing could change a slot before discovering an invalid
   tag. The whole filing operation now runs in a transaction.
6. The old tag-ID FK allowed another deck's tag to be attached through direct
   SQL. Schema 4 enforces both the tag and its owning deck.
7. Runtime tag-set inputs are now validated as arrays of positive safe integers
   in the capability layer, before changes are written.

## Boundaries

A “collection of tags” here means a temporary union or intersection. It does
not require another stored entity. Saved named filter collections, global tags,
tag hierarchies, per-copy tags and tag descriptions/colors are not modeled;
none is needed for the requested assignment and list-filter workflows.

Archidekt text import/export currently maps categories to slots and roles, not
native tag sets. Tags therefore do not round-trip through that text format.
Adding this requires a separate format decision and verification against the
external format's documentation.

At the time of this change the app was desktop-oriented; phones get a
read-and-rule view in v3 phase 4a (plan D17), which has no tag editing. Tag
controls were inspected at
1280×800, 1440×960 and 1024×768; long selected labels wrap without horizontal
clipping. A 390px viewport still exceeds the existing application shell's
width, so full phone layout support remains outside this change.

## Initial tag redesign validation

`npm run check` passes server/UI typechecks and all 344 tests, including the new
filter, integrity, migration and lifecycle regressions. Browser checks covered
multiple checkbox selections without closing, any/all behavior, name search,
searching dropdown choices, ArrowDown/Space selection, Escape, chip removal,
bulk removal, long labels, empty results and clearing filters, and deletion of
an actively filtered tag. Card/tag mutation checks used an isolated database
copy. The normal local app was rebuilt and restarted after a database backup;
the schema-4 migration preserved every deck, deck-card, tag and assignment row
when compared with that backup, and `PRAGMA foreign_key_check` returned no
violations. The two cards in the supplied screenshots were checked read-only
in the rebuilt app: the tagged card displays its assigned chip, and the
untagged card displays no chips.

## Physical assembly follow-up (2026-10-11)

Physical assembly now uses ordinary tags when the owner wants to track it.
The dedicated per-card flag, controls, Paper panel, missing-card export,
pull/buy computation and `paper_status` agent tool have been removed. The
schema-6 migration discards existing marks, as requested by the owner, and
removes the obsolete field from cut snapshots without changing their tag or
slot restoration data. It creates no replacement tags. Whole-deck categories
remain independent of tags and membership.

Validation: `npm run check` passes both typechecks and all 366 tests. New
regressions cover migration, historical undo, absence of the old endpoints
and tool, and agent tag filing on an active deck. A backup comparison of the
live schema-6 database preserved all 1,165 deck-card rows, 83 tag assignments,
14 decks and remaining application data, with no foreign-key violations;
379 old marks were discarded. Browser checks on an isolated seeded copy
confirmed tag assignment and chips still work, the Paper panel and checkbox
are gone, and export offers only the remaining category option. The local
app was rebuilt and restarted after the backup.
