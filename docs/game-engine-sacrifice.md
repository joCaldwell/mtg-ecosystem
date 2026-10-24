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

IR v18 and earlier reject all of these. The definition-kind relaxations for
noncreature enchantment triggers, enchantment activations and creature statics
require the v19 `SacrificeV1` capability; historical Aura triggers keep their
existing exception. Noncreature self-enters matching is enabled only in the
sacrifice lineage.

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
- **Blocker hints** exclude Carrion Feeder. Inherited blocker legality hints
  and first/double-strike steps use the ordered `MATCH_PROFILES` lineage, so
  later reviewed profiles inherit them without new profile-name lists.
- **Chooser departure.** Prior forced-sacrifice choices are projected only
  while both their chooser and selected object remain in the game (CR 800.4a,
  104.3a), matching the public view's choice filter.
- **SDK.** New request and response variants are additive. The SDK's
  exhaustive gates list them. Snapshot, bundle, projection and sidecar
  versions do not change.

The completion policies use only observation and the public catalog. They are
completion machinery, not strategy:
- **Optional effects.** Accept Goblin Arsonist's optional damage when its
  trigger is on top of the stack. Other optional effects use the removal
  policy, so Reclamation Sage declines to destroy its controller's Bastion
  or Sol Ring merely because it is the only target.
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

## Reviewed resource bounds

Only sacrifice-v1 changes capacities. The reviewed color-legal creature maxima
(including each commander) are white **17**, BG **42**, blue **11**, red **12**.
For the P6.3 calculation we deliberately overcount all **400** physical cards
as creatures. Capacity is on live tokens; ceased identities are never reused.

The new death producers require a lifetime proof as well as an episode proof.
There are at most 400 ordinary cards available to draw, eight tuck instructions
(one Time Ebb per blue seat, with eight conservatively allowing both blue
return instructions), four failed draws and four departures before a draw step.
Thus **416 turns** is an upper bound on normal untap opportunities. Searches,
extra draws and mulligans only reduce the number of remaining normal draws.
No instruction replenishes a library except those bounded tucks. A physical
mana source produces at most two mana per untap; the eight useful creature
recursions and eight bounce/tuck returns add at most sixteen extra activations
through haste. Lifetime spendable mana is therefore at most
**400×2×416 + 16×2 = 332,832**. This overcounts Signet's net production and
untaps of another player's permanents. Failed-payment reversals subtract their
receipt-recorded production before untapping, so repeated undo cannot fund
additional completed casts (CR 733.1–.2). There are no other untap instructions.

Every command-origin cast costs at least 1+2i for its ith previous command cast.
For four physical commanders, sum(n_i²) ≤332,832, so Cauchy–Schwarz bounds their
combined command casts below **1,154**. Hand recasts are already among the
sixteen returns. Neither an admitted commander nor a returned Aura produces
mana or tokens itself. Commander recasting can feed Requiem Angel, so it must
be included; the removal slice's 116-token lifetime proof cannot be reused.

Excluding Angel's separately counted Spirit generation, each physical producer
makes at most four tokens per entry or death. Overcounting every physical card
as a four-token producer gives **(400+16)×4 = 1,664** seed tokens, conservatively
treating even the existing Spirit producers' tokens as non-Spirits. This includes Siege-Gang,
Bastion and Doomed Dissenter. Singleton construction, color identity, no control
changes/copies, and the absence of white recursion give at most one Requiem
Angel per controller at any instant. A non-Spirit death therefore produces at
most one Spirit. A Spirit never produces another Spirit. All non-Spirit deaths
are bounded by **400+16+1,154+1,664 = 3,234**. Consequently at most
**1,664+3,234 = 4,898 lifetime tokens** are created. The deliberately conservative
live-token capacity is **8,192**, with checked monotonic u32 identity allocation.
Total lifetime creature deaths are at most **400+16+1,154+4,898 = 6,468**.

For a nonempty-stack episode, no creature has flash, no instruction returns a
creature directly to the battlefield, and recursion returns cards to hand.
Thus each physical creature can enter and die at most once in that episode.
The same is true of Butcher and Angel: they cannot be recast over a nonempty
stack. Allow 400 physical creatures, 8,192 initially live tokens, 1,600 tokens
from all 400 physical producers, and four extra Dissenter tokens. Every
non-Spirit can die once, and every resulting Spirit can die once:
**D = 2×(400+8,192+1,600+4) = 20,392 deaths**. This decreasing stock of
non-Spirits proves Butcher cascades finite. A Butcher trigger never creates a
creature; a forced sacrifice only consumes one from that stock. Requiem adds a
single terminal Spirit generation. Free outlets likewise consume a creature
per activation, including when they sacrifice themselves.

The capacities are:

- Live tokens: **8,192**.
- Damage occurrences: **2×(400+8,192) = 17,184**.
- Event members: **400+8,192+4 = 8,596**.
- Stack plus pending triggers: **294,540** = 400 physical spell entries +
  208 Azure Mage activations + 24 inherited activations + 12D + 400 self-enters
  triggers + four Rancor triggers + 4×(400+1,600+4+D/2) Warden triggers.
  The 12D term allows one cost activation per death, four Blood Artists from
  any seats, six controller-scoped payoffs (Zulaport, Bastion, Vindictive,
  Reaper, Butcher, Angel), and one additional self-dies payoff per death
  (Dissenter, Arsonist or Bouncer). Warden's separate term includes the Spirit
  births; it does not count initially live tokens as entering again. Mage's
  inherited per-step spendable mana bound is 832: 400 sources producing two plus
  sixteen returned sources producing two, hence at most 208 activations at
  four mana. Equip still requires an empty stack.
- Temporary effects: **8,192**. Bloodthrone creates at most one effect per
  sacrificed creature, bounded even over the entire match by 6,468 deaths.
  Adding the inherited 32-effect allowance stays below this capacity.

The catalog/pool tests guard these premises and the exact constants. All five
capacity checks retain typed faults and atomic rollback. Earlier profile
capacities and version numbers 18/19/16/15 remain unchanged.

## Reviewed witness substitutions

The admitted pool has no control-changing effect and no white/BG commander.
Requiem Angel cannot control Doomed Dissenter or its black Zombie. The reachable
substitute uses **Seller of Songbirds, its Bird token, and a Spirit** controlled
by Angel's white seat; Butcher forces their sacrifices. It tests a non-Spirit
card death, a non-Spirit token death, and a Spirit death, preserving every filter
rule. Dissenter's separate BG death produces its Zombie in that same accepted
pod. No private injected fixture is used for this substitution.


## Independent-review correction batch

The corrected batch is `generated/p69-review-fixes/after/sacrifice-match/report.json`
(SHA256 `246a99a08200b18f38ccb571221dd8849f01ac721bedb250ecc6c77eb63fe041`).
It completes **18/18 games**, including the batch's independent replay/checkpoint
checks, with **zero engine faults and zero truncations**. It uses the unchanged
seed domain and pod manifest. The pre-review report remains preserved with
SHA256 `82fc11f3b65be83e6c2972bb00e697504d71b9a0011968c97915d9f9c22f1f31`.

| Recorded quantity | Pre-review | Corrected |
| --- | ---: | ---: |
| Decisions | 32,385 | 35,615 |
| Turns | 717 | 775 |
| Rule steps | 96,004 | 104,112 |
| Blocks | 0 | 140 |
| First-strike steps | 0 | 48 |
| Cost sacrifices | 52 | 57 |
| Forced sacrifices | 1 | 1 |
| Each-opponent life-loss events | 8 | 15 |
| Each-opponent damage events | 11 | 12 |
| Total Fling damage | 7 | 12 |
| Tokens created on death | 5 | 6 |

Collected dies triggers change from Blood Artist **3→4**, Bastion **3→6**,
Zulaport **0→1**, Dissenter **0→1**, and Arsonist **0→1**. Vindictive **4**,
Reaper **3**, Angel **5** and Butcher **0** are unchanged. These remain
completion-policy counts, not claims that every admitted interaction appears
in the batch; the accepted-pod witnesses cover the remaining cases.

The corrected release's **397-file** native source closure is verified by
`generated/p69-review-fixes/final-source-manifest.json` (SHA256
`ad449df01ce263f84b8d3293def3521e9a6b5b45a72ca3f74c79f96d25bf3118`),
with build identity
`fdbd25acb9deed7a057300dbec1c2254cd8b2398310830ac2faf36e1584ffc51`.
The code commit is `dc021ae943cb6c8fa52e0f1d417063455a9243d8`.
The progress ledger records the complete check and historical-comparison gates.


Historical regression reruns pass **162/162 games** across all nine earlier
profiles. Their canonical gameplay evidence, checkpoint state/trace bytes and
final digests are identical; the comparison excludes only measured elapsed
time and the explicitly recorded build-identity envelope fields. All **44**
earlier expected-output/policy fixtures remain raw-byte identical to `2681c45`.
The progress ledger records the exact comparison artifact, hashes and totals.


Final checks pass with pinned Rust 1.99.0 / Node 26.11.1: `npm run check:engine`
verifies formatting, warnings-denied Clippy, **840 Rust tests** (zero failures,
12 ignored diagnostics) and **54 JavaScript checks** (zero failures or
cancellations). Check log SHA256:
`7b2d7542e2429b0c1f9032801c17eb28070d60dacbb499aef1e86a3aa13e3ebd`. The required release server build and SDK verification
pass **47 tests, strict TypeScript and all 31 response variants**; generated SDK
files are removed. The progress ledger qualifies the separate stale-build
attempts and records all final log/artifact paths.
