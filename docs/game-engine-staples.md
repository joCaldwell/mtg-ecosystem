# Commander staples — `commander-staples-v1`

P6.4 slice, following `commander-tokens-v1`
(docs/game-engine-tokens-counters.md). Adds the effects that most Commander
decks are built around: targeted removal, a board wipe, graveyard recursion,
land-search ramp, card draw with life loss, and a static anthem. It releases a
fifth finite legal profile. Earlier profiles, catalogs, policies and their bytes
stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Effect | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Destroy target creature | 701.8a–b, 608.2b | Moves the target from the battlefield to its owner's graveyard; counts as "destroyed" and "dies". An illegal target on resolution does nothing. |
| Destroy all creatures | 701.8a, 603.10a | All creatures are destroyed simultaneously as one event; leave-the-battlefield triggers (Blood Artist) look back at all of them together. Tokens then cease to exist (704.5d). |
| Return target creature card from your graveyard to your hand | 115.1, 608.2b, 400.7 | Targets a creature card in its controller's own graveyard. It becomes a new object in hand. A Commander returned this way is an ordinary hand card. |
| Optional enters trigger ("you may") | 603.5 | The trigger goes on the stack with its target; the controller chooses on resolution whether to perform the action. |
| Search library for basic land(s) | 701.23a–b, 701.23e, 701.24a–b | The searching player privately sees the matching cards in their own library and may find fewer than the stated number (up to that number). Found cards are not revealed. They are put onto the battlefield tapped, and the rest of the library is then shuffled using the match's trusted seeded randomness. |
| Draw and lose life | 121.1, 119.3 | The existing draw and life-loss instructions, for "you" and for a target player. |
| Static anthem | 611.3a–b, 613.4c, 613.7a | While the source is on the battlefield, creatures its controller controls get +1/+1 in layer 7c. The affected set is evaluated continuously, including for creatures that enter later (611.3c); it stops when the source leaves. |

Out of scope: regeneration and "can't be regenerated", destroy-with-restriction
removal ("nonblack", "with power 4 or greater"), reveal-then-split searches
(Cultivate, Kodama's Reach; see docs/parser-requests.md), searches for nonland
or non-basic cards, anthems with type restrictions or keyword grants, and
returning cards to the battlefield.

## IR and compiler

IR v15 adds instructions for destroy-target-creature, destroy-all-creatures,
return-target-creature-card-from-own-graveyard-to-hand, search-own-library-for-
basic-lands with a fixed maximum and tapped entry followed by a shuffle, and a
static controller-scoped +N/+N creature anthem. It also adds the optional flag
on the recursion instruction. Draw and lose-life reuse existing instructions,
extended to target players if they aren't already. "They can't be regenerated"
rejects the whole card (Wrath of God) rather than being dropped. IR v14 and
earlier reject the new instructions.

## Profile

`commander-staples-v1` uses the 72 tokens-v1 definitions plus:

| Color | Card | Behaviour |
| --- | --- | --- |
| W | Day of Judgment | Sorcery: destroy all creatures |
| W | Glorious Anthem | Enchantment: creatures you control get +1/+1 |
| B | Murder | Instant: destroy target creature |
| B | Raise Dead | Sorcery: return target creature card from your graveyard to your hand |
| B | Gravedigger | Enters: you may return target creature card from your graveyard to your hand |
| B | Night's Whisper | Sorcery: you draw two cards and lose 2 life |
| B | Sign in Blood | Sorcery: target player draws two cards and loses 2 life |
| G | Rampant Growth | Sorcery: search for a basic land, put it onto the battlefield tapped, shuffle |
| G | Explosive Vegetation | Sorcery: search for up to two basic lands, put them onto the battlefield tapped, shuffle |

That makes 81 definitions; verify each record's oracle text and AST before
admission. The white deck replaces two Plains (76 Plains: 24 + 76 = 100). The
black/green deck replaces five Swamp and two Forest (29 Forest, 30 Swamp:
41 + 59 = 100). Enchantments are a new permanent type for the match profiles.
Casting, stack placement, resolution to the battlefield and leaving the
battlefield must work for a noncreature permanent spell.

## Resource bounds

P6.3's lifetime token capacity (64) relied on no admitted card recurring a
token producer. Raise Dead and Gravedigger break that: Hornet Queen can be
recast after it dies. Token creation is therefore no longer bounded by the
decks. This profile replaces the lifetime cap with a capacity on live tokens of
256. Token identities stay monotonic and never reused, with checked overflow
far beyond the host's decision cap. Ceased tokens free live capacity.

Damage-occurrence, stack/pending and event-group capacities are re-derived with
the P6.3 method (docs/game-engine-tokens-counters.md, "Reviewed resource
bounds"), using C = this profile's creature cards plus 256. Each derivation must
account for recursion: a creature card can now enter and die more than once
during one nonempty-stack episode only if a recursion spell resolves in that
episode. Bound that, or state the bound per resolved spell.

Reaching a capacity is an explicit typed resource fault, classified as a host
truncation and never as a rules outcome. The completion batch must show zero.
Earlier profiles keep their exact limits.

## Player interface and hidden information

A search request is shown only to the searching player. It lists the matching
cards in their library by handle, and allows choosing up to the stated number,
including none. Other players observe only the found cards entering the
battlefield and the shuffle. Library order stays hidden, and handles minted for
the search never authorize anything after it. The shuffle consumes the match's
versioned seeded randomness exactly as setup shuffles do. Earlier profiles'
projections keep their bytes. Snapshot, bundle, projection and sidecar version
numbers do not change.

The completion policies use only observation and public catalog. They cast
removal at the most threatening opposing creature (highest power, then
Commander), cast the wipe only when opponents control more creatures than the
caster does, always find the maximum number of lands, and choose the
highest-power creature card for recursion.

## Evidence

- Compiler positive/negative pairs, including Wrath of God rejected because
  of "can't be regenerated", and v14 rejecting the new instructions.
- Reviewed runtime witnesses:
  - Murder on a creature;
  - Murder fizzling when its target already left;
  - Day of Judgment with Blood Artist seeing every death simultaneously,
    including tokens ceasing to exist afterwards;
  - Raise Dead and Gravedigger with yes and no choices, including returning a
    Commander card;
  - Rampant Growth and Explosive Vegetation finding zero, one or two lands;
  - the searcher's request never leaking to other players;
  - deterministic shuffle replay;
  - Sign in Blood targeting an opponent;
  - the anthem affecting a creature entering later, combining with counters
    and Giant Growth, and ending when the anthem leaves.
- Snapshot/restore/replay equality across trace modes for the witnesses,
  including restoring at a pending search choice.
- A frozen 18-game completion batch with seed domain
  `commander-staples-v1/completion-batch/` completes with zero engine faults or
  truncations. Its report counts destroyed creatures, recursions, lands
  searched and anthem-affected combat damage.
- Earlier profiles' catalog, policy and expected IR digests and historical
  batch outputs are unchanged, and `npm run check:engine` passes.
