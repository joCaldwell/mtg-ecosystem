# Commander combat damage and library return replacement

> Historical capacity proofs: D1b-2 supersedes this slice's per-profile bounds
> with the [generic capacity policy](game-engine-capacity-policy.md). The original
> arithmetic and evidence below describe the historical release only.

Contract frozen October 15, 2026 after independent rules, source and profile
review. This extends the exact Commander casting
profiles with separately admitted damage and sorcery/library scenarios. It does
not admit arbitrary decks, general replacement ordering, or legal Commander
matches. Existing five- and six-definition catalogs, historical profiles and
gameplay fixtures retain their behavior, including the earlier explicit
unsupported Commander-damage boundary.

## Frozen sources and vocabulary

Use the same historical schema-3 archive and retained manifest SHA-256
`b20406acec7b9af4610324af8848c76a2966315f1bb4043de2b03d53aa1a6f1b`.
No parser rerun, card refresh, runtime card-name dispatch, or raw-text semantic
fallback is introduced. The independently checked source additions are:

| Source | Oracle identity | Source index | Source SHA-256 |
| --- | --- | --- | --- |
| Time Ebb | `30cc8f7b-3c28-40f5-8f8f-157e8212280b` | 7282 | `30d342ea9e4ddace911969ad008d4f39244db74c134d2786832fb35c1d32392a` |
| Yargle and Multani | `980e1721-3717-4425-95a4-938dbf2ac661` | 22862 | `23a1eb822133fa8f171c83c85987f9685a28e00b39722c85f261e562572b73e9` |

Time Ebb's raw record SHA-256 is
`a04619d10b374f8927e6d68b5bbba10c9142fc328c70f8fb5df8ba521b88c321`;
its AST SHA-256 is
`d0a2a24b923a7e56a68e89b7b69b426e53f7fee0919d63aaed8863d6f8d709eb`.
Its frozen source is a normal {2}{U} sorcery with one creature target moved to
the top of its owner's library. Its complete AST explicitly preserves target,
owner and top placement. Yargle and Multani's raw record SHA-256 is
`dd2878582513bae556e31852560ea21cd694abad63d3756ea6b2296d0f124e13`;
its empty AST has SHA-256
`cf1cbb66a638b4860a516671fb74850e6ccf787fe6c4c8d29e9c04efe880bd05`.
Its frozen source is a normal legendary 18/6 creature with empty rules text,
mana cost {3}{B}{B}{G}, and black/green color identity.

The new IR vocabulary is `CardKind::Sorcery`, `CastSorceryV1`,
`OwnerLibraryTopMoveV1`, and
`Instruction::PutOnTopOwnerLibrary { target_slot, source_block }`.
Sorceries use ordinary main-phase, active-player, empty-stack timing. The
target specification and whole-spell legality check remain the existing
creature-target machinery. IR schema v4 is required for the new vocabulary;
the vanilla damage catalog requires no new card semantics. Lower the complete
card or reject it. Bottom/random/shuffled placement, wrong ownership, additional
effects and unrecognized source clauses must not be silently generalized.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
The pinned bytes were independently rehashed to
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules include 120–121, 117.3–5, 307.1, 400.2/7, 401.2–4, 601.2,
608.2, 614.6, 704.3–4, 733.1, 800.4, and 903.3/8/9b/10a.

## Exact admission and scenario witnesses

The existing five-definition and six-definition catalogs remain separate.
Damage profiles require the exact seven-definition schema-3 catalog containing
the previous six definitions plus Yargle and Multani. The library profile
requires its separate seven-definition schema-4 catalog containing the previous
six plus Time Ebb. Neither catalog substitutes for the other. Unknown, edited,
extra or missing definitions and arbitrary configurations reject.

All new profiles use four seats, 40 life each, turn 1 with P0 active and holding
precombat-main priority; empty stack, mana, damage, cast history and damage
history; unused land play; incarnation 0; untapped objects; no control grants.
Every object is owned and default-controlled by its stated owner except the
explicit borrowed commander below. Initial battlefield creatures satisfy
continuous-control eligibility. Libraries use the exact top-first order shown.
All three new profiles enable Commander combat damage; earlier Commander
profiles retain their explicit unsupported boundary. No player owns more than
sixteen objects. Each player's reachable mana remains
monochromatic; Yargle and Multani cannot be successfully recast using this pool.
That restriction introduces no mixed-color payment choice.

### `commander_damage()` and `commander_damage_borrowed()`

| Physical IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Yargle and Multani | battlefield; designated commander |
| 1 | P0 | Forest | battlefield |
| 2 | P1 | Island | battlefield |
| 3 | P1 | Unsummon | hand |
| 4 | P2 | Island | battlefield |
| 5 | P3 | Island | battlefield |
| 6,7 | P0 | Forest | library, ordered **6,7** |
| 8,9 | P1 | Island | library, ordered **8,9** |
| 10,11 | P2 | Island | library, ordered **10,11** |
| 12,13 | P3 | Island | library, ordered **12,13** |

The borrowed profile changes only commander 0's default controller to P1;
owner remains P0 and there is no control-changing effect. Owner and borrowed
witnesses attack P2 twice, beginning respectively on P0's turn 1 or P1's turn 2.
After the first hit, P2 has 22 life and 18 Commander damage and remains live.
After the second hit, P2 has 4 life and 36 Commander damage and loses during
the next SBA wave. In borrowed witnesses, P1 then concedes: P1-owned objects
leave and commander 0 is exiled because its default controller left. P0's
fresh-exile owner return acceptance and decline are separate witnesses. Both
preserve the physical commander's damage record for departed recipient P2.

### `commander_library()`

| Physical IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Time Ebb | hand |
| 1–3 | P0 | Island | battlefield |
| 4 | P1 | Isamaru | battlefield; designated commander |
| 5 | P1 | Plains | battlefield |
| 6 | P2 | Island | battlefield |
| 7 | P3 | Island | battlefield |
| 8,9 | P0 | Island | library, ordered **8,9** |
| 10,11 | P1 | Plains | library, ordered **10,11** |
| 12,13 | P2 | Island | library, ordered **12,13** |
| 14,15 | P3 | Island | library, ordered **14,15** |
| 16 | P2 | Grizzly Bears | battlefield |

P0 casts Time Ebb using its three Islands. Three commander branches target
Isamaru 4: accept tuck replacement and later cast from command; decline tuck
but accept the subsequent draw-to-hand replacement and cast from command; or
decline both choices and cast from hand. P1 uses Plains 5 to pay {W}. Only the
two successful command casts add one to its command-cast history. A fourth
branch targets Bears 16, inserts it above P2's Islands 12/13, and later draws
it into P2's private hand without any Commander choice. It does not attempt
to cast the Bears using blue mana. These are seven new source-to-runtime
witnesses across the three exact profiles, not complete legal matches.

## Commander damage

Only actual combat damage dealt by a designated physical commander to a player
adds to that commander's per-recipient history. Use consumed source incarnation,
controller and recipient facts from the common pre-damage state; do not infer
damage from life changes or aggregate together different commanders that hit
one player. Creature recipients and noncombat damage do not contribute. No
noncombat-damage source or prevention/lifelink/infect vocabulary is added here.

The ledger belongs to physical commander identity, rather than an incarnation,
controller, owner turn, or spell instance. It persists through zone changes,
recasting, controller changes and player departure. The same commander's damage
under different controllers combines for the same recipient; damage from two
different commanders never combines toward one 21-damage threshold. Public
Commander history exposes the existing exact per-recipient quantities without
raw physical IDs or hidden-zone handles.

Integrate ledger accumulation into the existing atomic simultaneous damage
event path. Validate the whole occurrence group before any damage/history write.
At the next normal SBA checkpoint, any player with at least 21 damage from one
commander loses. Freeze the union of life, failed-draw and Commander-loss
candidates, then perform simultaneous departure cleanup and outcome adjudication.
If one player satisfies several reasons, a deterministic single reported reason
may be selected without changing simultaneous loss membership. Never check
Commander loss between simultaneous damage occurrences or announce an
intermediate winner. No counters are reset by cleanup or by damage removal.

Source-driven threshold evidence must begin with empty history. Yargle and
Multani deals 18 on the first hit without causing Commander loss, then 36 total
on the second hit, causing Commander loss while the recipient remains at 4 life.
Exact 20/21 and multiple-source/recipient arithmetic may use paired private
typed-event domain fixtures; those do not broaden profile admission.

## Library movement and draws

Time Ebb moves a legal target directly from its present zone to its owner's
library top, preserving the actual destination and incarnation change in the
typed movement outcome. Insert at the front of the explicit ordered library;
never append, sort by physical ID, or replace its previous top. Every departure
and subsequent movement removes obsolete membership exactly once. Noncommander
targets use the same actual ordered-library path without a Commander choice.

For a commander, CR 903.9b offers its living owner a replacement choice before
the library move. Acceptance moves directly to command and leaves library order
and count unchanged. Decline moves to the actual library top. This is not a
post-entry SBA return, and it cannot be presented as a priority window. Preserve
the detached resolving spell, source facts, target binding, instruction cursor
and actual outcome through the owner's suspended choice; resume exactly once.
Only the owner answers, even when caster and controller differ. Source departure
preserves a started resolution; owner departure removes the target without
resurrection.

CR 903.9b applies from anywhere. Consequently, after a declined tuck, the next
draw of that commander has a second, distinct owner replacement choice before
the library-to-hand move. Declining that choice puts it in hand and permits an
ordinary hand cast without Commander tax. Accepting moves it to command and
consumes that one library-top draw attempt without drawing a second card.
Preserve the draw continuation across budgets, recovery and concessions; the
turn must not enter draw-step priority or repeat the draw before the choice is
finished. Actual outcome evidence must distinguish the replacement destination
from an ordinary card drawn into hand. An empty library alone still causes no
loss; a failed draw attempt causes loss at the next SBA check.

Libraries remain hidden to every player, including their owner. Public counts
and preexisting Commander designation/history do not grant library inspection,
reordering or an actionable current object handle. A pending owner draw-return
request identifies the designated commander with its already-public
`designation`, and carries `object: None`; visible-zone return choices carry
`object: Some(handle)`. Neither form exposes an ordinary library object row or
mints a hidden-zone object handle. The old battlefield target handle becomes stale on tuck;
the hand incarnation gets an owner-only handle only after an actual hand move.
Other players retain public history and counts, not a new private-hand handle.
General hidden-zone searches, revealed library cards and arbitrary draw effects
remain outside this profile.

## Required verification

Freeze exact scenario tables and catalog membership before implementation
acceptance. All admission is whole-configuration equality. Source fixtures,
definitions and initial ordered libraries are retained separately from generated
replay bundles. Every player's reachable mana remains monochromatic; do not
create an implicit mixed-color generic payment policy.

Verify natural threshold losses, same-physical history persistence and departure,
zero/negative/noncombat exclusion, independent sources and recipients, simultaneous
loss timing, library accept/decline, correct top insertion and draw replacement
accept/decline, sorcery timing, hand and command recasting, stale handles and
receipts, owner/source departure before and after accepted choices, and hidden
library/hand observations. Pair unsupported/invalid compiler and admission cases.

Recovery formats advance to snapshot v10, bundle v11, player projection/context
v8 and query sidecar v7. The optional return-request object handle and designation
are a versioned projection change; this does not change historical gameplay
semantics. Damage catalogs remain IR v3; library vocabulary requires IR v4.

At each suspended resolution/draw choice and threshold-damage/SBA boundary,
compare uninterrupted, budget-partitioned, cloned, restored and replayed state,
queries, observations, actual outcomes and semantic hashes. Reject rehashed
forgeries of damage history, owner/caster/source bindings, ordered libraries,
continuation cursors and pending draw identity. Off/compact/detailed trace modes
must agree semantically. Complete debug/release workspace tests, Clippy/formatting,
cross-language archive contracts, runnable source-to-runtime demo and fresh
offline rebuild/replay before reporting completion.
