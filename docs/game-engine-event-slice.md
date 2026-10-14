# Atomic events and nonsemantic tracing

The follow-up [instant-response slice](game-engine-stack-slice.md) adds a third
exact profile, snapshot/bundle v5, and player projection v3. Earlier hashes below
remain historical evidence.

Scope frozen October 13, 2026. Complete the next bounded P2 foundation without
expanding the five-definition pool or admitting new card mechanics. General
replacement/prevention, triggers, simultaneous damage, and SBA batches remain
later slices. No product decision blocks this work.

## Event contract

Use a private production `commit_batch` path. A batch has a typed cause (mana
activation, casting, payment, resolving instruction, spell completion, or player
departure), an ordered/simultaneous hierarchy, and stable occurrence paths under
a checked batch ID. Capture facts before the root and before each occurrence:
consumed objects' identity, owner, controller, zone, tapped state, characteristics,
and affected players' mana/life/status. Actual outcomes occupy explicit typed
occurrence slots; consumers use actual successor identities, never guesses.

Ordered children consume earlier actual changes. A simultaneous group evaluates
independent occurrences against one common pre-event view. Reject conflicting
read/write footprints, empty groups, more than 16 occurrences, and depth beyond
4 before mutation. Independent simultaneous moves/removals are supported as a
kernel contract, not evidence for unimplemented simultaneous card mechanics.
Nested ordered groups may contain simultaneous groups; simultaneous groups
contain occurrences, not ordered programs. A failed child rolls back the entire
batch, including counters and evidence. No public batch API bypasses legality.

The existing intrinsic mana activation is one ordered tap/add-mana batch. Casting,
payment, return-to-hand instructions, and spell completion use typed singleton
batches. A departure removes its owned objects in one simultaneous batch and
then performs the remaining departure procedure. Removal is not a zone move.
The existing flat event stream remains a detailed-mode compatibility view; the
new batch evidence carries grouping, cause, pre-event facts, and actual slots.

Rules read from the pinned September 25 text: CR 106.3, 400.7, 605.3b, 608.2c,
700.1, and 800.4a. Official source:
https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt.
Ordered mana costs/effects must not be mislabeled simultaneous. Event grouping
alone does not implement replacement applicability or trigger matching.

## Trace contract

Separate diagnostic retention from semantic state. Modes are `off`, `compact`,
and `detailed`, selected on construction and preserved by fork/restore. Every
mode executes the same rules, captures needed facts, returns the same outcomes,
and advances the same logical event/batch counters. Off retains no evidence;
compact retains a deterministic rolling verification digest/count; detailed
also retains event/batch records and the legacy flat events. Compact and detailed
verification digests must match. Records link their execution boundary and
seen decision/concession prefixes; batch causes link actors, sources, stack and
instruction identities, and pinned rule IDs. No diagnostic payload or digest enters player
observations, SearchContext, game state hashes, or rule decisions.

Snapshot v4 retains trace policy/evidence separately from semantic state and
verifies both by exact-boundary replay. Semantic hashes are equal across modes;
snapshot bytes can differ. The rolling digest starts with SHA-256 of UTF-8
`mtg-event-trace-v1`; each next digest hashes the previous lowercase hexadecimal
digest, a NUL byte, and the canonical UTF-8 record (sorted object keys, exact
quantities as strings). The record count uses the shared logical sequence. Query work still binds to exact semantic state and
executor identity. Native bundle v4 retains all three modes for both scenarios,
checks independent restore/replay and budget equivalence, and rebuilds offline.
IR remains version 2 and player projection formats remain version 2.

## Exit evidence

Preserve the original P1 gameplay inputs/events and the prior departure flat
trace. Retain a new version-4 semantic hash witness rather than overwriting older
artifacts. Test ordered versus common simultaneous pre-state, outcome-slot
lineage, failed-child rollback, footprint conflicts, bound/overflow rejection,
private facts, stale incarnations, production batch compositions, all trace modes,
budget partitions, branch isolation, every suspended/pending recovery boundary,
and forged diagnostic payloads. Verify the retained source closure in a fresh
offline Cargo store. This slice alone does not sign off full P2 or full games.

Status: implemented for the two reviewed profiles; verification and retained
identities are recorded in [implementation progress](game-engine-progress.md).
No general trigger or replacement registration API is admitted by this slice.
