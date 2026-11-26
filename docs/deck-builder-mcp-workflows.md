# Commander MCP: sourcing and proposal cleanup

The app computes overlaps, quantities, counts, estimates and checks; the agent
supplies deck references and explicit evidence. Shapes are the tool schemas in
`packages/deck-builder/src/mcp/`; contracts are spec §§5.5, 8 and
[10.3](deck-builder-spec.md#103-tools). Use the IDs and revisions the app returns.

## Compare a target with multiple source decks

After `start_session`, call `compare_deck_sources` once with the target, every
source, filters and sort. Maybe lists are excluded; read-only calls need no revision
guards and may read Archived decks. `shared_cards` and `remaining_cards` ignore
filters; `remaining_cards` is not a shopping list. Missing prices are excluded from
estimates, never free.

`apply_source_tags: true` atomically adds `From <exact deck name>` tags to **all**
shared target cards and needs `expected_revision` plus `expected_source_revisions`
for every source. Source tags are additive history: reruns never remove old labels,
unrelated tags or slots, and deck renames do not rename them.

## Record ownership without inventing quantities

`Have` tags, `Keep`, `Ordered`, engines, memories and source membership follow
spec §5.5; never add source quantities together. When the owner supplies better
evidence, call `record_ownership` on an editable deck with a `basis`, passing the
newest returned evidence ID (or null) as `expected_evidence_id`. A batch is
all-or-nothing, one entry per card. Each statement replaces the whole current claim,
so omitted quantities become unknown; history is kept. `unowned` requires
`quantity: 0`; available quantity cannot exceed a known owned quantity. Correct
evidence by recording a new status.

## Replace an obsolete plan as one proposal group

Read `list_proposals` and the brief first. `propose_document_changes` groups
thesis, engine and power-target changes, using exact engine names from `get_deck`;
the owner rules on the group as a whole. To change it, send the **complete**
replacement via `revise_proposal` (`action: "amend"`, any member ID) or
`propose_document_changes` with `supersedes`; omitted unaccepted changes are
abandoned. `revise_proposal` also withdraws. Agents revise only their own pending
work; ruled items stay immutable. Retired edits show `withdrawn`/`superseded`, not
an owner rejection.

## Limits

This is an evidence ledger, not a printing-level collection or reservation system;
moving a card never adjusts availability. Staleness counts every membership revision
and compares stored engine structure and text, not meaning. Document groups never
include card changes. Archived decks reject cleanup, tagging and evidence writes.
`check-agent-workflows.mjs` in the
[browser fixture](../packages/deck-builder/browser-checks/README.md) exercises these flows.
