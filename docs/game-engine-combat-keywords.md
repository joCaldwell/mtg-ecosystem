# Combat damage keywords — `commander-combat-keywords-v1`

P6.2 slice, following `commander-keywords-v1` (docs/game-engine-keywords.md).
Adds trample, first strike, double strike and menace, and releases a third
finite legal Commander profile. Earlier profiles, catalogs, policies and their
bytes stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Keyword | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Trample | 702.19a–d, 702.2c, 510.1c, 510.1e | A blocked attacking creature with trample assigns lethal damage to every creature blocking it before assigning any to the player it attacks; the remainder is divided as its controller chooses among those blockers and that player. Lethal accounts for damage already marked and for damage other creatures assign in the same step, but not for prevention or replacement. Any nonzero amount from a deathtouch source is lethal. With no blockers remaining, all its damage is assigned to the player (702.19d). No effect while blocking. |
| First strike | 702.7b, 510.4 | If any attacking or blocking creature has first strike or double strike as the combat damage step begins, that step is a first-strike damage step in which only those creatures assign damage, followed by a second combat damage step. |
| Double strike | 702.4b, 510.4 | Assigns damage in both combat damage steps. |
| Menace | 702.111b, 509.1b | Can't be blocked except by two or more creatures. Checked over the defending player's whole block declaration. |

In the second step, a creature assigns damage if it remains in combat and either
had neither first strike nor double strike as the first step began, or has double
strike. Each combat damage step gives the active player priority afterwards
(510.3), so state-based actions, triggers and instants occur between the steps.
Lifelink, deathtouch and Commander combat damage apply separately in each step,
so a double striker with lifelink gains life twice. A creature removed from
combat after the first step assigns nothing in the second.

No effect in the pool grants or removes keywords. "Had first strike or double
strike as the first step began" therefore equals the creature's current
intrinsic keywords; the engine still records the participating set when the
first-strike step begins so the second step never re-derives it. Adding
keyword-changing effects later requires no change to that record.

Out of scope: trample over planeswalkers (no planeswalkers), protection,
keyword grants/removal, banding, and damage-assignment order (removed from the
pinned rules; 510.1c divides damage freely).

## Player interface

Damage assignment requests may name the attacked player as a recipient when the
attacker has trample, together with each blocker's lethal-damage requirement.
Validation rejects any assignment to the player unless every blocker receives
at least its lethal amount, and checks the controller's total assignment
(510.1e). A trampling attacker blocked by one creature still receives a choice,
since excess may go to the player. The blockers request marks attackers with
menace. Foundations and keywords-v1 projections keep their existing bytes; all
new fields and turn-step variants are additive and never appear in those
profiles. Snapshot, bundle, projection and sidecar version numbers do not change.

## Profile

IR v13 extends `KeywordAbility` with `trample`, `first strike`,
`double strike` and `menace`; v12 stays limited to the P6.1 keywords. The
profile catalog is the 50 keywords-v1 definitions plus:

| Color | Card | Keywords |
| --- | --- | --- |
| W | Youthful Knight | First strike |
| W | Knight of Meadowgrain | First strike, Lifelink |
| W | Fencing Ace | Double strike |
| W | Skyhunter Skirmisher | Flying, Double strike |
| B | Gurmag Swiftwing | Flying, First strike, Haste |
| B | Cursed Minotaur | Menace |
| B | Alley Strangler | Menace |
| G | Colossal Dreadmaw | Trample |
| G | Elfhame Wurm | Vigilance, Trample |

Decks add these to the keywords-v1 decks, replacing basic lands of the
matching colors: the white deck has 85 Plains (11 + 4 nonbasic = 15 cards plus
85 = 100); the black/green deck has 37 Forest and 35 Swamp (23 + 5 = 28 cards
plus 72 = 100). Commanders, admission,
setup, capacities and loss rules are those of the earlier profiles.

## Evidence

- Compiler positive/negative pairs, including v12 rejecting the new keywords.
- Reviewed runtime witnesses: trample excess to the player; trample with an
  under-lethal assignment rejected; trample with deathtouch lethal of 1;
  trample after its blocker leaves combat; first strike killing a blocker
  before it deals damage; double strike dealing damage twice, with lifelink
  gaining twice and Commander damage counted twice; a creature without first
  strike dealing damage only in the second step; priority, SBAs and triggers
  between steps; Fog preventing both steps; menace rejecting a single blocker
  and accepting two.
- Snapshot/restore/replay equality across trace modes for the witnesses.
- A frozen 18-game completion batch with seed domain
  `commander-combat-keywords-v1/completion-batch/` completes with zero engine
  faults or truncations.
- Earlier profiles' catalog, policy and expected IR digests are unchanged, and
  `npm run check:engine` passes.
