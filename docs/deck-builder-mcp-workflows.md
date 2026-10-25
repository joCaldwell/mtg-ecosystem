# Commander MCP: sourcing and proposal cleanup

The app computes overlaps, quantities, counts, estimates and checks. The agent
supplies deck references and explicit evidence, then interprets the result. No
card list, inventory, shopping list or accepted brief is maintained in chat.

## Compare a target with multiple source decks

Call `start_session`, read the requested decks/revisions, and use one operation:

```json
{
  "name": "compare_deck_sources",
  "arguments": {
    "deck": "Jo’s Wildfire",
    "sources": ["Jo’s ramp", "Jo’s Chaos"],
    "sourcing": "not_in_sources",
    "not_marked_owned": true,
    "sort": "price_desc",
    "apply_source_tags": true,
    "expected_revision": 18,
    "expected_source_revisions": { "7": 11, "9": 24 }
  }
}
```

IDs/revisions above are illustrative: use the IDs and revisions actually
returned by the app, including every source. Names resolve exactly. The target
cannot be one of its own sources; duplicate source references are deduplicated.
Maybe lists are excluded. Read-only comparisons omit `apply_source_tags` and
need no revision guards; they can read Legacy decks. The tool is conservatively
annotated as a write because it can apply tags.

The result has concise `content` text and MCP `structuredContent`:

- `target`, `sources`: deck identities and membership revisions.
- `cards`: target cards matching filters and sorting.
- `shared_cards`: every shared target card, each with every matching source.
- `remaining_cards`: every target card absent from all sources and not
  explicitly owned, sorted by estimated unit price descending, unpriced last.
  This includes unresolved ownership; it is not a shopping list.
- `summary`: target, shared, remaining, unknown-ownership and filtered-result
  distinct-card/copy counts and cost estimates; also the number of cards tagged.
  Costs use target list quantities, whole USD cents, and `unpriced` distinct-card
  counts. Missing prices are excluded from the estimate, never treated as free.
- `prices_as_of`: the timestamp of the installed price dataset, or null.

Each card contains `oracle_id`, exact `name`, target `quantity`, `source_decks`
(with ID, name, membership revision, list quantity and source-tag label),
`sourcing`, `ownership` (status, physical quantity and newest-first evidence),
`reuse` (availability status and available quantity), and `price`
(`usd_cents`, `as_of`, `estimated`). Estimates describe the cheapest playable
printing, not the value of the owner's actual copy. Without an installed price
refresh, the timestamp can be null even when test/imported price rows exist.

`source_decks` describes lists, never a physical inventory. Winter Moon in two
sources receives both `From <exact deck name>` tags. Its physical quantity and
availability stay unknown unless explicitly recorded. Source tags are additive
and historical labels: rerunning adds current matches but never deletes old
source labels, unrelated tags, or primary/secondary gameplay slots. Tagging
covers **all shared target cards**, regardless of filters. It is atomic and
requires matching revisions for target and every source; tag organization does
not increment the membership revision. Source labels follow existing tag
vocabulary validation, and deck renames do not rename previously applied tags.

Filters: `sourcing` (`shared`, `not_in_sources`), `ownership` (`owned`, `unowned`,
`unknown`), `not_marked_owned`, and `availability` (`available`, `insufficient`,
`unknown`). Sorts: `name`, `price_desc`, `price_asc`, `ownership`, `sourcing`.
Unknown prices sort last in both price directions; ties sort by exact name.
Filters affect `cards` and its result summary, while the full shared/remaining
sections and their summaries keep their meanings.

## Record ownership without inventing quantities

Existing, newly applied, or explicitly renamed `Have` tags record ownership, with null
physical quantity and null available quantity. Heroic Intervention tagged Have
is therefore excluded from `not_marked_owned` even when absent from both sources.
Reapplying Have preserves a more precise current owned quantity/availability
statement instead of replacing it with unknowns.
`Keep`, `Ordered`, engines, memories and source membership establish no physical
ownership. Descent into Avernus may remain a Keep while ownership stays unknown.

Record a new explicit statement when the owner supplies better evidence:

```json
{
  "name": "record_ownership",
  "arguments": {
    "deck": "Jo’s Wildfire",
    "expected_revision": 18,
    "cards": [{
      "card": "Heroic Intervention",
      "status": "owned",
      "quantity": 1,
      "available_quantity": 1,
      "basis": "The owner confirmed one physical copy is available for this deck.",
      "expected_evidence_id": 63
    }]
  }
}
```

Use an editable context deck. Evidence ID 63 is illustrative: use the newest
returned evidence ID, or null if no evidence exists. This optional guard rejects
concurrent evidence changes. A batch lands entirely or not at all. Each card
may be recorded once per batch. All earlier statements remain in the evidence
history; the newest statement is current. A new statement replaces the entire
current physical/available-quantity claim, so omitted quantities become unknown.

`owned` permits an unknown physical quantity; a known quantity must be positive.
`unowned` requires explicit `quantity: 0`; `unknown` permits only null quantities.
Available quantity, when known, is nonnegative and cannot exceed a known owned
quantity. Availability is sufficient/insufficient relative to the target list
quantity only when an explicit available quantity exists. Even known ownership
does not prove that a copy is free to move. Never add source quantities together.

Evidence persists independently of membership and tags, including removal,
parking, moves, forks and deletion of the originating deck. The original deck
name and ID remain provenance, not a live membership assertion. Removing Have
does not retract evidence; record a corrected status with its basis instead.

## Replace an obsolete plan as one proposal group

Read `list_proposals` and the deck's brief/checks first. After the owner removes
a package, repair the accepted descriptions through owner approval:

```json
{
  "name": "propose_document_changes",
  "arguments": {
    "deck": "Jo’s Wildfire",
    "expected_revision": 19,
    "changes": [
      {
        "kind": "engine_remove",
        "engine_name": "Norin’s exchange racket",
        "reason": "The owner removed this package from the list."
      },
      {
        "kind": "engine_set",
        "engine_name": "Budget finish",
        "description": "The replacement finishing plan.",
        "pieces": [{ "card": "Descent into Avernus" }],
        "reason": "Reflect the owner's revised budget and explicit keep."
      },
      {
        "kind": "thesis",
        "text": "Use the budget finishing plan and retain [[Descent into Avernus]].",
        "reason": "Keep the thesis aligned with the replacement engine."
      }
    ]
  }
}
```

Use the exact engine name returned by `get_deck`. This example describes proposal
mechanics, not advice to include a particular card. The response gives a
`group_id`, `edit_ids`, and structured views. Grouped changes always remain
pending until ruled on. Brief and power-target edits from agents never apply
directly, on either Paper or Theorycrafted decks.

If the budget changes again, replace the **complete** group:

```json
{
  "name": "revise_proposal",
  "arguments": {
    "type": "document",
    "id": 52,
    "action": "amend",
    "reason": "The owner changed the finishing budget again.",
    "expected_revision": 20,
    "changes": [
      {
        "kind": "engine_remove",
        "engine_name": "Norin’s exchange racket",
        "reason": "This earlier package is still obsolete."
      },
      {
        "kind": "thesis",
        "text": "Use the latest budget plan while retaining [[Descent into Avernus]].",
        "reason": "Replace the earlier finishing instructions."
      }
    ]
  }
}
```

Here 52 is any pending edit in the earlier group. The omitted, unaccepted
replacement engine is abandoned; the still-needed removal is carried forward.
Alternatively call `propose_document_changes` with `supersedes: [52]`. Multiple
old groups can be replaced together. An amendment receives new IDs; earlier
payloads, authors, timestamps, reasons, and links remain intact. Naming one
member expands to its entire group for supersession, withdrawal or ruling.

Other cleanup calls:

```json
{ "name": "revise_proposal", "arguments": { "type": "document", "id": 52, "action": "withdraw", "reason": "This plan was abandoned.", "expected_revision": 20 } }
{ "name": "revise_proposal", "arguments": { "type": "cards", "id": 12, "action": "amend", "reason": "Revised the remaining ramp plan.", "expected_revision": 20, "items": [{ "action": "add", "card": "Heroic Intervention", "reason": "The owner asked to reconsider this addition." }] } }
{ "name": "list_proposals", "arguments": { "deck": "Jo’s Wildfire", "include_history": true } }
```

Agents revise only their own pending work. A partially ruled card proposal can
be amended/withdrawn; its accepted/rejected items remain immutable in the old
record and the replacement describes only remaining work. Accepted documents
need new proposals. Duplicate current drafts from the same author for the same
field/named engine require explicit supersession, rather than accumulating.
Different authors' proposals are never silently withdrawn by one another.

Views expose `is_current`, `disposition`, `superseded_by`, `supersedes`,
`base_revision`, `current_revision`, `stale`; document views also include
`group_id`, `group_edit_ids`, and shallow `group_edits`. `status` preserves the
existing storage vocabulary: retired document edits have terminal `rejected`
status with a distinguishing `withdrawn`/`superseded` disposition; this does not
create an owner rejection ruling. Retired card proposals are `resolved` with
the corresponding disposition; their unruled items remain as historical drafts.
Only current, pending/open work contributes to application projections.

## Owner approval and history

The inbox presents a related document group once and shows all changes with one
accept/reject control. The brief also presents the complete group together.
“Show proposal history” is read-only and includes previous drafts, withdrawn
work and resolved proposals. `GET /api/proposal-history` serves this same recent
history; `list_proposals` gives agents structured history. Each list currently
retains the existing 200-row response limit; all underlying records stay stored.

Approval checks every group member before changing anything, then applies all
changes and logs each ruling atomically. Engine completeness and other checks
are refreshed in the response and live UI. A material membership revision or
changed document/engine base marks a proposal stale. The owner must review and
acknowledge it explicitly. Owner API acceptance supports `expected_revision`
and `acknowledge_stale: true`; stale deck edits require both. Revision mismatch
returns 409 even when acknowledged. The browser sends the displayed revision
and asks for acknowledgment. It also sends `expected_documents` (edit ID →
reviewed current text/snapshot) to reject another document change at the same
membership revision; live updates reset its acknowledgment. Agents have no ruling tool. Superseded/withdrawn
drafts cannot be revived by stale acknowledgment.

Legacy remains read-only for both actors, including proposal cleanup, tagging
and evidence recording through that deck. Only the owner changes lifecycle or
restores it. Paper membership changes still need owner-approved card proposals;
Theorycrafted direct card edits retain their existing behavior. Creating card
proposals and individual brief/power-target proposals also accept optional
`expected_revision`.

## Migration and limits

Schema 10 is migrated transactionally on the next database open. Existing deck
and proposal IDs, lists, tags, documents and owner decisions are retained. New
persistent evidence is seeded only from currently assigned explicit Have tags,
with unknown physical and available quantities. Earlier tags already lost by
past removals cannot be reconstructed. Existing pending proposals receive an
unknown base revision and are conservatively stale until reviewed or amended;
no historical deck revision is fabricated. Reopening schema 10 does not re-seed
ownership or infer extra copies. Older supported schema versions still migrate.

This is an evidence ledger, not a printing-level collection or reservation
system. Explicit available quantities are statements at a time, not automatic
reservations; moving a card does not adjust them. Source membership may refer
to Paper, Theorycrafted or Legacy lists, with lifecycle included in returned
source deck metadata. Staleness conservatively includes every membership
revision, including changes that an owner may consider immaterial. It checks
stored engine structure and document text, not the meaning of arbitrary prose.
Grouped document changes do not include card membership changes.

Search pagination, compact mutation responses, richer type statistics, manual
price refresh controls and advisory interaction warnings remain backlog.
