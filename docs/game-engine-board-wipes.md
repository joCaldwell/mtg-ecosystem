# Board-wide effects and X spells — `commander-wipes-v1`

This is the P6.10 slice. It follows `commander-sacrifice-v1`
(docs/game-engine-sacrifice.md).

The pool has one board wipe today, Day of Judgment, and no spells with {X}
costs. Board-wide effects are a Commander staple. This slice adds:
- mass destroy, exile, damage, -N/-N and bounce, each with filters;
- damage to each player;
- one-sided effects;
- counted amounts;
- a turn-long "exile instead" replacement;
- a self cost reduction;
- "can't be countered";
- {X} in mana costs.

It releases an eleventh finite legal profile. Earlier profiles, catalogs,
policies and their bytes stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| "All"/"each" effects | 608.2c, 608.2h | The affected set is the set of objects that match the filter as the instruction is performed. Destruction, exile and bounce of that set are one simultaneous event. Creatures that die together trigger as one batch of occurrences (603.2c). |
| Damage to each object and player | 120.3, 120.4, 608.2h | One damage event deals damage to every matching creature and every player at once. Furnace of Rath and Samite Healer apply to each occurrence. It is noncombat damage, so it is never commander damage. If every remaining player loses at once, the game is a draw (104.4a). |
| Filtered sets | 608.2h | Each filter is evaluated on current characteristics, granted keywords included: attacking creatures, creatures with or without flying, nonland permanents, enchantments, and creatures your opponents control. |
| Until-end-of-turn -N/-N to a set | 611.2c, 613.4c, 514.2 | The affected set is fixed when the effect begins. Creatures that enter later are not affected. The effect applies in layer 7c with counters, the anthem and attachments. It ends in cleanup. |
| Counted amounts | 107.3c, 608.2h | Chain Reaction's X is the number of creatures on the battlefield as it resolves. Fumigate's life is the number of creatures destroyed by its own destroy instruction. That count includes tokens, and commanders that are then moved to the command zone. |
| "Exile instead" for the turn | 614.1a, 614.6, 611.2c | Anger of the Gods creates a replacement effect for this turn. It applies to the creatures it dealt damage to, and to nothing else. If one of those creatures would die later this turn, it is exiled instead, by any cause: SBA from damage, destruction or sacrifice. It does not die, so dies triggers don't fire. A commander exiled this way gets the existing 903.9a choice. The effect ends at cleanup. |
| {X} in a mana cost | 107.3a, 601.2b, 601.2f, 202.3e | The caster announces X ≥ 0 while casting and pays it as generic mana. On the stack the spell's mana value includes X; anywhere else, X is 0. X never changes after the announcement. |
| Self cost reduction | 601.2f, 118.7a | Blasphemous Act costs {1} less for each creature on the battlefield. The count is taken when the total cost is determined. It reduces only the generic part, so the total is never below {R}. |
| Can't be countered | 101.2, 113.6g, 608.2c | Counterspell and Swan Song can target Volcanic Fallout, but countering it does nothing. Swan Song's following instruction still happens, so the Bird is created. |
| Each player sacrifices | 101.4, 701.21a | Innocent Blood includes the caster. Choices are made in APNAP order using the P6.9 machinery. |
| Return all to hand | 903.9b, 704.5d | Each commander's owner gets the existing hand-or-command choice. Tokens cease to exist. Auras go to the graveyard, and Equipment becomes unattached (704.5m–n). |

Out of scope:
- modal "choose two or more" (Austere Command, Fiery Confluence);
- cycling;
- flashback;
- miracle;
- {X} in activation costs;
- cost increases;
- "lands don't untap" (Bontu's Last Reckoning);
- regeneration;
- linked "exiled this way" returns.

## IR and compiler

IR v20 adds:

- `all`/`each` object sets for destroy, exile, return-to-hand and pump. Sets use
  the filters above, including `status: attacking`, `withAbility` and
  `withoutAbility`.
- damage to `each` with a list of object sets and `each-player`.
- the `x` amount, from either:
  - a mana-cost {X}, which is announced when the spell is cast; or
  - a `count` binding of creatures on the battlefield, which is computed on
    resolution.
- `forEach` an `event-result` of this spell's own destroy, for amounts.
- `create-replacement` with duration `this-turn`. It is limited to the shape
  "creature dealt damage this way would die: exile it instead".
- a static `cost-modification` that reduces this spell's generic cost by a
  creature count.
- a static `cant be-countered` on this spell.
- `each-player` sacrifice, in spells.

IR v19 and earlier reject all of these.

## Profile

`commander-wipes-v1` uses the 153 sacrifice-v1 definitions plus:

| Color | Card | Text (abbreviated) |
| --- | --- | --- |
| W | Fumigate | {3}{W}{W} sorcery: destroy all creatures; gain 1 life for each destroyed this way |
| W | Planar Cleansing | {3}{W}{W}{W} sorcery: destroy all nonland permanents |
| W | Final Judgment | {4}{W}{W} sorcery: exile all creatures |
| B | Infest | {1}{B}{B} sorcery: all creatures get -2/-2 until end of turn |
| B | Innocent Blood | {B} sorcery: each player sacrifices a creature |
| B | Massacre Wurm | {3}{B}{B}{B} 6/5: when it enters, opponents' creatures get -2/-2 until end of turn; whenever a creature an opponent controls dies, that player loses 2 life |
| G | Tranquility | {2}{G} sorcery: destroy all enchantments |
| U | Evacuation | {3}{U}{U} instant: return all creatures to their owners' hands |
| U | Aetherize | {3}{U} instant: return all attacking creatures to their owner's hand |
| R | Pyroclasm | {1}{R} sorcery: 2 damage to each creature |
| R | Anger of the Gods | {1}{R}{R} sorcery: 3 damage to each creature; exile instead of dying this turn |
| R | Volcanic Fallout | {1}{R}{R} instant, can't be countered: 2 damage to each creature and each player |
| R | Chain Reaction | {2}{R}{R} sorcery: X damage to each creature, X = creatures on the battlefield |
| R | Earthquake | {X}{R} sorcery: X damage to each creature without flying and each player |
| R | Blasphemous Act | {8}{R} sorcery, costs {1} less per creature: 13 damage to each creature |

That makes 168 definitions. Before admission, verify each record's oracle
text, AST, power/toughness, Commander legality and color identity. The archive
line numbers, in table order, are: 26672, 25481, 15934, 32314, 15537, 22242,
37174, 38299, 18717, 34443, 8755, 28447, 1282, 23195 and 18372. Costs and
stats come from the source records.

Each deck adds its colored cards. Basics fill each deck to 100:

| Deck | Adds | Basics |
| --- | --- | --- |
| White | Fumigate, Planar Cleansing, Final Judgment | 54 Plains |
| Black/green | Infest, Innocent Blood, Massacre Wurm, Tranquility | 8 Forest, 7 Swamp |
| Blue | Evacuation, Aetherize | 69 Islands |
| Red | the six red cards | 64 Mountains |

The default pod and the batch pods are those of sacrifice-v1. The seed domain
is `commander-wipes-v1/completion-batch/`.

### Resource bounds

Re-derive with the P6.3 method. The new terms to account for are:
- damage occurrences per noncombat event: every creature plus every player;
- simultaneous-death trigger batches from mass destruction, including the death
  payoffs of P6.9 and Massacre Wurm's per-death trigger;
- temporary-effect slots for -N/-N sets and Anger's replacement;
- bounds on X. X is limited by the mana available in one payment. Quantities
  have no fixed-width ceiling;
- lifetime tokens: no new token makers.

Earlier profile capacities stay unchanged.

## Player interface

- **Casting with {X}.** Casting a spell with {X} asks the caster for X. Give the
  request an allowed maximum equal to the mana the player could produce, or let
  any value be submitted and validate it at payment. The choice is the
  implementer's; document it. Once announced, X is public on the stack.
- **Blasphemous Act.** Its cost in mana windows and spend requests reflects the
  reduction.
- **Versions.** New request/response variants are additive, and the SDK's
  exhaustive gates list them. Snapshot, bundle, projection and sidecar versions
  do not change.

The completion policies use only observation and the public catalog. They are
completion machinery, not strategy.

- **Symmetric wipes** (Fumigate, Planar Cleansing, Final Judgment, Infest,
  Pyroclasm, Anger, Chain Reaction, Blasphemous Act, Evacuation, Volcanic
  Fallout): cast only when opponents together control at least two more
  creatures that the wipe would remove than the caster does. Volcanic Fallout is
  never cast if it would reduce the caster to 0 or less.
- **Massacre Wurm:** cast whenever castable.
- **Innocent Blood:** cast when the caster has a token or no creature, and some
  opponent has a creature.
- **Tranquility:** cast when opponents control more enchantment mana value than
  the caster does.
- **Aetherize:** cast in the caster's declare-blockers step when at least two
  creatures attack it, or when the attack would be lethal.
- **Earthquake:** choose the smallest X that kills the most opposing creatures
  without flying, capped so the caster stays above 0 life after Furnace
  doubling. Cast only if that X kills at least one creature, or wins the game.

## Evidence

- **Compiler.** Positive/negative pairs, with Austere Command, Fiery Confluence,
  Bontu's Last Reckoning and Rolling Earthquake (horsemanship) rejected as whole
  cards. v19 rejects every new capability.
- **Reviewed runtime witnesses.** These must be reachable from accepted inputs.
  - **Fumigate.**
    - Counts tokens and a commander that then goes to the command zone.
    - All the simultaneous deaths trigger Blood Artist and Zulaport once each.
  - **Planar Cleansing.**
    - Destroys Equipment, Auras, Sol Ring, Glorious Anthem and Bastion, but not
      Command Tower or basics.
    - Rancor returns.
  - **Final Judgment.**
    - Exiles creatures without dies triggers; tokens cease to exist.
    - Commanders get the 903.9a choice.
  - **Infest.**
    - A creature with Glorious Anthem or a +1/+1 counter survives at toughness
      1 or more.
    - A creature cast after Infest resolves keeps its full stats (611.2c).
  - **Massacre Wurm.**
    - Shrinks only opponents' creatures.
    - Each opposing creature that dies, tokens included, costs its controller 2
      life. That includes a later combat death the same turn.
  - **Innocent Blood.** APNAP choices, with the caster included.
  - **Pyroclasm.** With Furnace of Rath it deals 4 to each creature. A Samite
    Healer shield on one creature prevents only that creature's damage.
  - **Anger of the Gods.**
    - A commander that dies from its damage is exiled, then gets the
      command-zone choice.
    - A survivor with toughness 4 or more dealt damage by Anger that dies to Lightning
      Bolt the same turn is exiled.
    - A survivor sacrificed the same turn is exiled. No Blood Artist or
      Zulaport trigger happens for any exiled creature.
    - The replacement ends at cleanup.
    - A creature that was not dealt damage by Anger dies normally.
  - **Volcanic Fallout.**
    - Counterspell resolves without countering it.
    - Swan Song does not counter it but still creates the Bird.
    - Simultaneous player deaths produce the 104.4a draw when they are the last
      players.
  - **Chain Reaction.** X counts tokens, and counts creatures with flying, at
    resolution.
  - **Earthquake.**
    - X=0 is legal.
    - X=3 skips Storm Crow and Rorix but hits every player.
    - Furnace doubles each occurrence.
    - The mana value on the stack is X+1, as seen by Counterspell policy
      ordering.
    - X is retained through snapshot/restore.
  - **Blasphemous Act.**
    - Its castability and cost track the creature count: with eight creatures it
      costs {R}, and with twelve still {R}.
    - It is uncastable with too few creatures for the available mana.
  - **Evacuation.**
    - Commanders get the hand/command choice, and tokens cease.
    - Auras go to the graveyard; Rancor returns.
    - Equipment stays on the battlefield, unattached.
    - Cast during combat, it removes attackers and blockers from combat.
  - **Aetherize.** Returns only attacking creatures.
  - **Tranquility.** Destroys Pacifism, Spidersilk Armor and Bastion; the
    creature Pacifism was attached to can attack again.
- **Unreachable witnesses.** If a listed witness proves unreachable from
  accepted inputs, substitute the nearest reachable witness, cite why, and
  record a deviation. Do not use a private injected fixture unless the
  substitute would lose the rule.
- **Replay.** Snapshot/restore/replay equality across trace modes for the
  witnesses, including snapshots during X announcement and while Anger's
  replacement is active.
- **Batch.** A frozen 18-game completion batch completes with zero engine faults
  or truncations. Its report counts:
  - wipes cast, by card;
  - creatures removed per wipe;
  - Anger exiles;
  - X values chosen;
  - Massacre Wurm life loss;
  - Volcanic Fallout counter attempts.
- **Earlier profiles.** Catalog, policy and expected IR digests, and the
  historical batch outputs, are unchanged.
- **Checks.** `npm run check:engine` and SDK verification pass.

## Reviewed resource bounds

These capacities apply only to wipes-v1. Earlier profiles' capacities remain
unchanged. As in P6.9, overcount all 400 physical cards as creatures, mana
sources and token producers whenever this increases the bound. A live token
capacity of **65,536** exceeds the lifetime token bound below. Ceased token
identities are retained and never reused.

There are at most **416 turns**: 400 ordinary draws, eight inherited tuck
instructions, four failed draws and four departures. No new effect replenishes
a library. The eight new mass returns (four Evacuations and four Aetherizes)
each return at most 400 physical creatures. Tokens cannot be recast. Thus these
instructions add at most **3,200** physical hand returns. They can also return
Gravedigger: each of four singleton Gravediggers can enter once more per mass
return, adding at most **32** useful recursion instructions. Its trigger cannot
return itself while it is on the battlefield, no player can recur another
owner's Gravedigger, and none of these return spells can be recurred. Including
the inherited eight recursions and eight bounce/tuck returns gives at most
**3,248** additional physical entries. This explicitly includes the secondary
recursion from returned Gravediggers.

Lifetime spendable mana is at most **400×2×416 + 3,248×2 = 339,296**.
Signet input spending adds no production; reversing a failed payment removes
its receipt-recorded production before untapping. Command-origin casts cost at
least 1+2i for the ith previous cast, so sum(n_i²) ≤339,296. Cauchy–Schwarz gives
fewer than 1,165 combined command casts; conservatively allow **1,165**. Admitted
commanders neither create tokens nor produce mana.

Without Angel, at most **(400+3,248)×4 = 14,592** seed tokens can be created.
Singletons, color identity and no control changes allow only one Requiem Angel
per controller at a time. Each non-Spirit death makes at most one terminal
Spirit; Spirit deaths make no further Spirit. At most
**400+3,248+1,165+14,592 = 19,405** non-Spirit deaths therefore create at most
19,405 additional tokens. Total lifetime tokens are at most **33,997**, below
the live capacity; total lifetime creature deaths are at most
**400+3,248+1,165+33,997 = 38,810**.

A nonempty-stack episode cannot recast a creature: none has flash and no
instruction returns one directly to the battlefield. Every physical producer
therefore enters/dies at most once per episode, including Massacre Wurm,
Butcher and Angel. With 65,536 initially live tokens, allow 1,600 tokens from
400 producers and four additional Dissenter tokens, then double for Angel's
terminal Spirit generation: **D = 2×(400+65,536+1,600+4) = 135,080** deaths.
This decreasing non-Spirit stock still proves free outlets and Butcher cascades
finite. Wipes cannot increase it except through the already counted producers.

The capacities are:

- **65,536 live tokens**.
- **131,872 damage occurrences**, 2×(400+65,536), covering combat; a noncombat
  set needs only 400+65,536+4 = 65,940 occurrences. Furnace changes amounts,
  never occurrence counts.
- **65,940 event members**, 400 physical cards + 65,536 live tokens + four
  simultaneous losses. Commander bounce choices precede one simultaneous move.
- **2,442,108 stack plus pending entries** = 400 physical spells + 1,824 Mage
  activations + 24 inherited activations + 16D + 400 enters triggers + four
  Rancor triggers + 4×(400+1,600+4+D/2) Warden triggers. The 16D term retains
  P6.9's 12D and adds four Massacre Wurms per death, conservatively ignoring
  controller/color exclusions. Per-step spendable mana is at most
  **400×2+3,248×2 = 7,296**, funding at most 1,824 Mage activations. Mana abilities
  add no stack entry; equip requires an empty stack.
- **65,536 temporary effects**. Bloodthrone is bounded by 38,810 lifetime deaths.
  Add the inherited 32 slots, at most 400+3,248+1,165 = 4,813 entries
  conservatively treated as Wurm set effects, and four each of Infest and Anger.
  The same per-entry allowance covers extra Healer/Bouncer uses from mass
  returns. The resulting 43,663 bound is below capacity. Each set is one effect with
  incarnation-bound membership, bounded by the live object count.

X is an exact arbitrary-precision quantity. The public announcement accepts any
nonnegative X; ordinary payment validates it, with the existing failed-payment
reversal when unpayable. There is no fixed-width or arbitrary gameplay ceiling.
The completion policy's finite search through X≤7,296 uses the payment proof
above, chooses the smallest value achieving its largest affordable kill count,
and keeps the caster above zero after Furnace multiplication.

All five capacity paths use typed faults and atomic event rollback. Overflow
remains a resource truncation, never a Magic outcome. Snapshot/bundle/projection/
sidecar versions remain **18/19/16/15**. X, death-controller LKI, set-effect
membership, and board-return continuations serialize additively; empty fields
are omitted.
