# Four-player instant responses

Scope frozen October 13, 2026. Add one exact nine-card scenario configuration,
using the existing five-definition catalog; no source/AST/IR vocabulary changes.
P0's Island (card 0), Unsummon (1), and the borrowed Bears (2: owner P1,
default controller P2) match the baseline. Add an Island and Unsummon for each
opponent: P1 cards 3/4, P2 cards 5/6, P3 cards 7/8. Islands start untapped on
the battlefield; spells start in their respective owners' hands. Initial seating,
life, main-phase abstraction, privacy policy, and no-randomness policy are
unchanged. This is a bounded P3 slice, not full P2/P3 signoff or match admission.

## Runtime and rules

Read CR 117.3b-c/4, 400.7, 405.1-2, 601.2i, 608.1/2b/2m, 733.1, and
800.4a/h/j in the pinned September 25 rules:
https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt.

A player with priority may propose the supported instant while other completed
spells are on the stack. Only that player completes target/payment stages; no
other player responds during announcement or resolution. Each successful cast
resets the pass round, and its controller retains priority. Each all-live pass
round resolves exactly the top entry. Priority returns to the active player
(or the next surviving seat after the absent original turn owner). A new pass
round is needed for the next spell. Cap the stack at four spell entries, the
number of supported physical spells in the exact scenario. No stack-depth-only
bypass permits other definitions or configurations.

Casting frames retain the prior pass round. Reversing an unsupported/unpayable
attempt restores that round and original hand incarnation, while keeping input
history and effects of any departure. Filter saved passes by surviving players.
Losing a lower spell cannot discard an unrelated top casting/resolution frame.
Restore validates every lower entry as a completed cast, using its own program,
plus unique stack IDs/sources and exact top-frame binding.

Correct the earlier source-departure edge: under CR 608.2m an already resolving
spell continues after leaving the stack. A resolving frame owns its frozen spell
instance and reads its program from the physical definition. Departures still
remove owned objects/stack entries. The detached, choice-free invocation finishes
its remaining instructions, skips a source movement if the source already left,
and releases priority normally. Resolution-time chooser reassignment, abilities,
tokens, copies, and general player references remain unsupported.

## Public state and evidence

Observation/SearchContext v3 expose stack order **bottom to top**, each public
spell handle/controller, whether casting is complete, and announced target
handles. Save each viewer's target announcement references at the rules
transition. An expired announcement handle is historical information: expose
no current target handle and never follow the physical card into a hidden hand.
Response checking still requires live handles. Process stack rows in query work
units; queries allocate no handles or history. Snapshot/bundle v5 retain these
fields and detached resolution; query sidecars use version 2. IR stays version 2.

The main witness casts P0's Unsummon, passes P0/P1, then P2 floats U and casts
another Unsummon at the same Bears. P2/P3/P0/P1 pass: P2's spell resolves, moves
the creature to its owner's hand, and goes to P2's graveyard. P0 receives priority.
P0/P1/P2/P3 pass again: the original spell has a stale incarnation target, does
not resolve, and goes to P0's graveyard. P1/P3's spells remain private in hand.
Verify separate response cases for all three opponents and a full four-entry
chain; preserve all earlier flat gameplay traces as historical references.

Tests cover timing/ownership rejection, failed response casting over a live lower
spell, pass reset/restoration, lower versus top departure during casting/payment/
resolution, continuation after source departure, stale target/handle rejection,
public order/private hands, every paused execution/query recovery boundary,
trace-mode/budget equivalence, and rehashed malformed stack metadata. Retain the
three configurations in all three trace modes, with separate ordinary and
detached response runs (twelve retained executions), and rebuild/replay offline. No user
product decision blocks this work.

Status: implemented and verified for the closed profiles. The normal response
witness has 16 decisions, 27 events, and 41 checkpoints; the detached-source
witness has 16 decisions, one concession, 30 events, and 43 checkpoints.
See [implementation evidence](game-engine-progress.md).
