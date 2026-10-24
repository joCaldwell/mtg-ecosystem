# Sacrifice and death payoffs — `commander-sacrifice-v1`

This is the P6.9 slice. It follows `commander-removal-v1`
(docs/game-engine-removal.md).

Sacrificing creatures for value ("aristocrats") is a core Commander archetype.
The engine already has two narrow pieces of it:
- self-sacrifice activations (Hapless Researcher, Viridian Zealot);
- one "whenever a creature dies" payoff (Blood Artist).

This slice adds:
- sacrificing another permanent as an activation cost or an additional spell cost;
- death triggers that are filtered by controller, token status and subtype;
- "each opponent" life loss, damage and forced sacrifice;
- last-known information about a sacrificed creature;
- a static "can't block" restriction.

It releases a tenth finite legal profile. Earlier profiles, catalogs, policies
and their bytes stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Sacrifice | 701.21a | The controller moves its own permanent from the battlefield to its owner's graveyard. Sacrifice is not destruction. |
| Sacrifice as a cost | 118.8, 118.8d, 601.2b, 601.2h, 602.2b | The permanent to sacrifice is chosen and sacrificed while costs are paid, after targets are chosen. If no permanent matches the filter, the cost is unpayable and the spell or ability can't be cast or activated. "Sacrifice a creature" may sacrifice the source creature itself; then any effect on the source does nothing. Additional costs don't change mana value. |
| Dies triggers | 700.4, 603.6c, 603.10a | "Dies" means put into a graveyard from the battlefield. These triggers look back in time. "You control", "another", "nontoken" and "non-Spirit" are evaluated on the dying creature as it last existed on the battlefield. The trigger source's own death counts where the text includes it ("this creature or another", Midnight Reaper's "a nontoken creature you control"). Creatures that die together each trigger every payoff on the battlefield or leaving with them (603.2c). |
| Last-known information | 608.2h, 113.7a | Fling uses the sacrificed creature's last-known power, including counters, anthem, Equipment and until-end-of-turn pumps. Mogg Fanatic and Goblin Arsonist deal damage from themselves as they last existed. |
| Each opponent | 102.2–102.3, 800.4a | All other players still in the game when the effect happens. A player who has left is not an opponent. |
| Forced sacrifice | 101.4, 701.21a | When each opponent sacrifices a creature of their choice, choices are made in APNAP order: the active player first, then turn order. Each player sees the earlier choices, and then all are sacrificed simultaneously. A player with no creature sacrifices nothing. These deaths trigger the payoffs of the sacrificing players. |
| Damage to each opponent | 120.3, 903.10a | Vindictive Vampire, Goblin Bombardment, Siege-Gang Commander and Mogg Fanatic deal noncombat damage. Furnace of Rath doubles it, and it is never commander damage. |
| Can't block | 509.1b | Carrion Feeder is never a legal blocker. The blocker hints exclude it. |
| Until end of turn | 514.2, 611.2a | Bloodthrone Vampire's +2/+2 uses the existing temporary-effect machinery and ends in cleanup. |
| Search on a sacrificed source | 701.23, 602.2 | Sakura-Tribe Elder's ability resolves after its source is gone. It reuses the existing private basic-land search and seeded shuffle, and the land enters tapped. |

Out of scope:
- mana abilities with sacrifice-another costs (Ashnod's Altar, Phyrexian Altar);
- "triggers only once each turn" (Morbid Opportunist);
- undying, persist and other self-return;
- Treasure and other noncreature tokens;
- trigger doubling (Teysa);
- "sacrifice one or more" (Plumb the Forbidden);
- Grave Pact and Liliana's Triumph, which are unparsed in the frozen archive.

## IR and compiler

IR v19 adds:

- sacrifice costs with a permanent filter, on activated abilities and as a
  spell's `additional-cost` ability. The filter covers card types, `anyOf`
  types and subtypes (Goblin), and does not have to be the source.
- dies triggers whose subject is either a filtered set or `any-of` this and a
  filtered set. The filter covers `control: you`, `other`, `nonToken` and
  `nonSubtypes`.
- `each-opponent` as a player set for lose-life, damage and sacrifice. A
  `chosenBy: each-opponent` sacrifice uses the APNAP choice above.
- an amount equal to the power of the creature sacrificed as this spell's cost
  (Fling's `event-result` relation).
- optional targeted damage on a dies trigger (Goblin Arsonist).
- static `cant block` on the source creature.
- self-sacrifice abilities with a targeted damage effect or a land search.

IR v18 and earlier reject all of these.

## Profile

`commander-sacrifice-v1` uses the 135 removal-v1 definitions plus the cards
below.

| Color | Card | Text (abbreviated) |
| --- | --- | --- |
| B | Carrion Feeder | {B} 1/1: can't block; sacrifice a creature: put a +1/+1 counter on this |
| B | Bloodthrone Vampire | {1}{B} 1/1: sacrifice a creature: +2/+2 until end of turn |
| B | Village Rites | {B} instant, additional cost: sacrifice a creature; draw two cards |
| B | Costly Plunder | {1}{B} instant, additional cost: sacrifice an artifact or creature; draw two cards |
| B | Bone Splinters | {B} sorcery, additional cost: sacrifice a creature; destroy target creature |
| B | Zulaport Cutthroat | {1}{B} 1/1: when this or another creature you control dies, each opponent loses 1 life and you gain 1 life |
| B | Bastion of Remembrance | {2}{B} enchantment: enters with a 1/1 Human Soldier token; when a creature you control dies, each opponent loses 1 life and you gain 1 life |
| B | Vindictive Vampire | {3}{B} 2/3: when another creature you control dies, it deals 1 damage to each opponent and you gain 1 life |
| B | Midnight Reaper | {2}{B} 3/2: when a nontoken creature you control dies, it deals 1 damage to you and you draw a card |
| B | Doomed Dissenter | {1}{B} 1/1: when it dies, create a 2/2 black Zombie |
| B | Butcher of Malakir | {5}{B}{B} 5/4 flying: when this or another creature you control dies, each opponent sacrifices a creature |
| G | Sakura-Tribe Elder | {1}{G} 1/1: sacrifice this: search for a basic land, put it onto the battlefield tapped, then shuffle |
| R | Goblin Bombardment | {1}{R} enchantment: sacrifice a creature: 1 damage to any target |
| R | Fling | {1}{R} instant, additional cost: sacrifice a creature; damage equal to its power to any target |
| R | Mogg Fanatic | {R} 1/1: sacrifice this: 1 damage to any target |
| R | Goblin Arsonist | {R} 1/1: when it dies, you may have it deal 1 damage to any target |
| R | Siege-Gang Commander | {3}{R}{R} 2/2: enters with three 1/1 red Goblin tokens; {1}{R}, sacrifice a Goblin: 2 damage to any target |
| W | Requiem Angel | {5}{W} 5/5 flying: when another non-Spirit creature you control dies, create a 1/1 white flying Spirit |

That makes 153 definitions. Before admission, verify each record's:
- oracle text, AST, power and toughness;
- Commander legality and color identity.

In table order, the archive line numbers are: 24338, 25615, 8089, 21180, 1914,
17831, 30059, 2691, 35051, 2711, 25302, 34271, 35838, 5439, 19100, 29025, 33396
and 13487. Costs and stats come from the source records, not from this table.

Each deck adds its colored cards. Basics fill each deck to 100 cards:

| Deck | Adds | Basics |
| --- | --- | --- |
| White | Requiem Angel | 57 Plains |
| Black/green | the eleven black cards and Sakura-Tribe Elder | 9 Forest, 10 Swamp |
| Blue | nothing | 71 Islands |
| Red | the five red cards | 70 Mountains |

The default pod and the batch pods are those of removal-v1. The seed domain is
`commander-sacrifice-v1/completion-batch/`.

### Resource bounds

Re-derive the resource bounds with the P6.3 method. Account for:
- the larger BG and red creature pools;
- free sacrifice outlets. Each activation consumes a creature, so the bound
  comes from creatures and tokens available in an episode, not from mana;
- multiplicative death triggers. Each death can trigger every payoff its
  controller has: Zulaport, Bastion, Vindictive Vampire, Midnight Reaper,
  Butcher, Requiem Angel, and Blood Artist from any seat;
- Butcher cascades. Opponents' forced sacrifices trigger their own Butchers;
- token makers on death: Doomed Dissenter and Requiem Angel. A Spirit token's
  death does not trigger Requiem Angel;
- Siege-Gang tokens and Bastion's token in the lifetime token allowance.

Each bound must be finite and proved from the admitted pool. Capacity
overflows keep typed faults and atomic rollback. Earlier profile capacities
stay unchanged.

## Player interface

- **Sacrifice cost requests.** A sacrifice-cost request lists the controller's
  matching permanents. It appears during cost payment, after target selection,
  and is answered with one choice.
- **Forced sacrifice.** Each player is asked in APNAP order, and only that
  player sees the request.
- **Blocker hints** exclude Carrion Feeder.
- **SDK.** New request and response variants are additive. The SDK's
  exhaustive gates list them. Snapshot, bundle, projection and sidecar
  versions do not change.

The completion policies use only observation and the public catalog. They are
completion machinery, not strategy:
- **Sacrifice choices.** Sacrifice tokens first, then the lowest-power
  creature. Never sacrifice the commander unless it is the only creature.
- **Cost spells.** Cast Village Rites, Costly Plunder and Bone Splinters when a
  token or another creature with power at most 1 is available.
- **Damage outlets.**
  - Use Fling when the sacrificed creature's power is lethal to an opposing
    creature or player.
  - Use Bombardment, Mogg Fanatic and Siege-Gang to finish an opponent at low
    life or to kill an opposing creature with toughness at most the damage.
- **Counter outlets.** Carrion Feeder and Bloodthrone sacrifice only tokens.
- **Sakura-Tribe Elder** sacrifices itself when it would otherwise die, or at
  its controller's end step.

## Evidence

- **Compiler positive/negative pairs.**
  - Ashnod's Altar, Morbid Opportunist, Young Wolf and Pitiless Plunderer
    reject as whole cards.
  - v18 rejects every new capability.
- **Reviewed runtime witnesses.** Use only lines reachable from accepted inputs.
  - Village Rites is uncastable with no creature. Sacrificing a token draws
    two cards, and Zulaport plus Bastion each trigger.
  - Costly Plunder sacrifices Sol Ring.
  - Bone Splinters sacrifices its controller's creature while targeting an
    opponent's. If the target becomes illegal, the sacrificed creature stays
    sacrificed.
  - Fling sacrifices a creature wearing Bonesplitter and deals its last-known
    power, including the +2; Furnace doubles it. Fling and Bloodthrone are in
    different decks, so Bloodthrone's pump is witnessed on its own attack.
  - Carrion Feeder sacrifices itself to its own ability, so the counter does
    nothing. Carrion Feeder can't block.
  - Zulaport, Bastion and Vindictive Vampire skip a conceded opponent. Their
    damage and life changes are correct with Furnace of Rath.
  - Midnight Reaper triggers on its own death, but not on a token's death.
  - Requiem Angel makes a Spirit when Doomed Dissenter dies, and another when
    the Zombie token dies. A Spirit dying makes nothing.
  - Butcher of Malakir's forced sacrifice takes choices in APNAP order. An
    opponent with no creature sacrifices nothing. The cascade ends when one
    opponent's Butcher triggers against the first player's Butcher.
  - Day of Judgment with Zulaport, Blood Artist and Butcher on the battlefield
    gives every simultaneous death its triggers, with trigger ordering.
  - Sakura-Tribe Elder is sacrificed in response to Murder and still finds its
    land. If Stifle counters the ability, the Elder is still gone.
  - Mogg Fanatic and Goblin Arsonist (optional) deal damage after they die.
    Arsonist with no "may" chosen deals nothing.
  - Siege-Gang sacrifices a Goblin token, and Raging Goblin also qualifies. A
    non-Goblin creature is not a legal choice.
  - Each of these is a Blood Artist death that triggers once per creature:
    Swords to Plowshares exiling a creature (no death), a token sacrificed by
    Bombardment, and a commander sacrificed then moved to the command zone.
- **Unreachable witnesses.** If a listed witness proves unreachable from
  accepted inputs in the admitted pool, substitute the nearest reachable
  witness, cite why, and record it as a deviation. Do not use a private
  injected fixture unless the substitute would lose the rule being tested.
- **Replay.** Snapshot/restore/replay equality holds across trace modes for the
  witnesses. This includes snapshots inside a sacrifice-cost payment and inside
  an APNAP forced-sacrifice choice.
- **Batch.** A frozen 18-game completion batch completes with zero engine
  faults or truncations. Its report counts:
  - sacrifices, split into cost and forced;
  - dies triggers by payoff card;
  - each-opponent life loss and damage events;
  - Fling damage;
  - tokens created on death.
- **Earlier profiles and checks.** Earlier profiles' catalog, policy and
  expected IR digests and historical batch outputs are unchanged. `npm run
  check:engine` and SDK verification pass.
