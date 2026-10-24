# Permanent removal and exile — `commander-removal-v1`

P6.8 slice, following `commander-attachments-v1` (docs/game-engine-attachments.md).
Since P6.5 and P6.7, the pool contains artifacts, Equipment, Auras and
enchantments, but the only removal is creature-only (Murder, Lightning Bolt, Day
of Judgment, Unsummon, Time Ebb). This slice adds the standard Commander answers:
- artifact and enchantment removal;
- removal of any permanent;
- exile;
- "its controller" compensation (life, tokens, damage);
- removal on creature triggers and on sacrifice abilities;
- a typed counterspell.

It releases a ninth finite legal profile. Earlier profiles, catalogs, policies
and their bytes stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Destroy | 701.8a–b | Move the permanent from the battlefield to its owner's graveyard. The permanent dies only if it is a creature. |
| Exile | 701.13a, 406.1–3, 704.5d | Move the object to exile. Exile is a public zone, and cards there are face up. A token that is exiled ceases to exist as an SBA. |
| Commander in exile or graveyard | 903.9a | Uses the existing state-based command-zone choice. |
| Target legality | 115, 601.2c, 608.2b | Permanent filters cover these classes: artifact, enchantment, creature, land, permanent, nonartifact, nonblack, and "an opponent controls". Hexproof and shroud apply to every new targeted spell and ability. If the target is illegal on resolution, the spell or ability does nothing. That includes no life, no token and no damage. |
| Last-known information | 608.2h, 113.7a | These values come from the target as it last existed on the battlefield or stack: "its controller", Swords' "its power", Feed the Swarm's "mana value", and Smash's "that artifact's controller". Power includes counters, the anthem and attachments. A token's mana value is 0 (202.3a). The command-zone choice does not change these values. |
| Can't be regenerated | 608.2c, 701.19c | Pongify carries this flag. The pool has no regeneration effects, so it has no runtime effect. The IR records the flag, and the compiler admits it only together with destroy. |
| Optional targeted triggers | 603.3d, 603.5 | Reclamation Sage's target is chosen as the trigger is put on the stack, and "may" is decided on resolution. A trigger with no legal target is removed from the stack. That applies to Chupacabra too. |
| Sacrifice with a target | 601.2c, 601.2h, 602.2b | Viridian Zealot's target is chosen before costs are paid. Sacrificing it is a cost, and its death triggers Blood Artist. Stifle can counter the ability. |
| Typed counterspell | 701.6a, 608.2b | Swan Song can target only enchantment, instant or sorcery spells, so creature and artifact spells are excluded. Its Bird goes to the controller of the countered spell. A commander spell that is countered uses the existing choice. |
| Damage to a player | 120.3a, 903.10a | Smash to Smithereens' 3 damage is noncombat damage from the spell, so Furnace of Rath doubles it. It is not commander damage. |

Removing an attachment uses the existing P6.7 rules:
- **Equipment:** the creature loses its grant and stays on the battlefield.
- **Aura:** removing it frees the creature, so Pacifism no longer restricts it.
- **Creature with an Aura:** removing the creature sends the Aura to its owner's graveyard by SBA 704.5m. Rancor still returns to hand.

Destroying a land or mana rock removes it from mana windows at once.

Out of scope:
- regeneration;
- indestructible;
- protection;
- "until this leaves" exile (Banishing Light, Oblivion Ring, Fiend Hunter);
- overload (Vandalblast, Cyclonic Rift);
- additional sacrifice costs (Bone Splinters);
- searches by another player (Path to Exile and Assassin's Trophy are unparsed);
- multi-target spells (Return to Dust);
- planeswalker cards.

## IR and compiler

IR v18 adds:

- `exile` of a targeted permanent;
- permanent filters with type sets, `nonTypes`, `nonColors` and control
  `opponent`;
- the reference "controller of the target", used for the life gain, token
  creation and damage recipient;
- last-known attribute amounts (`power`, `mana-value`) of the target;
- `regenerationForbidden` on destroy, admitted only in that position;
- optional targeted enters triggers;
- targeted effects on sacrifice-self activated abilities;
- a counter step whose spell filter is a set of card types, followed by a token
  for the controller of the countered spell.

IR v17 and earlier reject all of these.

## Profile

`commander-removal-v1` uses the 118 attachments-v1 definitions plus:

| Color | Card | Text (abbreviated) |
| --- | --- | --- |
| W | Swords to Plowshares | {W} instant: exile target creature; its controller gains life equal to its power |
| W | Disenchant | {1}{W} instant: destroy target artifact or enchantment |
| W | Revoke Existence | {1}{W} sorcery: exile target artifact or enchantment |
| W | Generous Gift | {2}{W} instant: destroy target permanent; its controller creates a 3/3 green Elephant |
| G | Naturalize | {1}{G} instant: destroy target artifact or enchantment |
| G | Nature's Claim | {G} instant: destroy target artifact or enchantment; its controller gains 4 life |
| G | Beast Within | {2}{G} instant: destroy target permanent; its controller creates a 3/3 green Beast |
| G | Reclamation Sage | {2}{G} 2/1: when it enters, you may destroy target artifact or enchantment |
| G | Viridian Zealot | {G}{G} 2/1: {1}{G}, sacrifice it: destroy target artifact or enchantment |
| B | Go for the Throat | {1}{B} instant: destroy target nonartifact creature |
| B | Doom Blade | {1}{B} instant: destroy target nonblack creature |
| B | Feed the Swarm | {1}{B} sorcery: destroy target creature or enchantment an opponent controls; you lose life equal to its mana value |
| B | Ravenous Chupacabra | {2}{B}{B} 2/2: when it enters, destroy target creature an opponent controls |
| R | Shatter | {1}{R} instant: destroy target artifact |
| R | Smash to Smithereens | {1}{R} instant: destroy target artifact; 3 damage to its controller |
| U | Pongify | {U} instant: destroy target creature, it can't be regenerated; its controller creates a 3/3 green Ape |
| U | Swan Song | {U} instant: counter target enchantment, instant or sorcery spell; its controller creates a 2/2 blue flying Bird |

That makes 135 definitions. Before admission, verify each record against the
archive:
- oracle text;
- AST;
- power and toughness;
- Commander legality;
- color identity.

Archive line numbers are 26652, 25245, 6084, 37833, 28499, 16407, 17909, 517,
22286, 6973, 13482, 13209, 18539, 19924, 33763, 868 and 21329, in table order.
Costs and stats come from the source records, not this table.

Each deck adds its colored cards:

| Deck | Adds | Basics (total 100) |
| --- | --- | --- |
| White | Swords, Disenchant, Revoke Existence, Generous Gift | 58 Plains |
| Black/green | the five green and four black cards | 16 Forest, 15 Swamp |
| Blue | Pongify, Swan Song | 71 Islands |
| Red | Shatter, Smash to Smithereens | 75 Mountains |

The default pod and the batch pods are those of attachments-v1, with seed
domain `commander-removal-v1/completion-batch/`.

### Resource bounds

Re-derive the resource bounds with the P6.3 method. The changes to account for
are:
- the BG creature pool grows by three (Sage, Zealot, Chupacabra), which changes C;
- two more self-enters triggers per BG seat;
- one Zealot activation per physical Zealot;
- tokens created for another player by Generous Gift, Beast Within, Pongify
  and Swan Song;
- the larger lifetime token allowance.

None of these spells can be recurred: Raise Dead and Gravedigger return
creature cards only. Earlier profile capacities stay unchanged.

## Player interface

- **Exile.** Observations add an exile list for each player's owned cards. It
  is public in every seat, using the additive serde field convention. It is
  omitted when empty, so earlier profiles keep their bytes.
- **Targets.** Candidate lists include artifacts, enchantments, lands and tokens
  where the filter admits them, and honour hexproof and shroud.
- **Swan Song.** Its candidates list only enchantment, instant and sorcery
  spells on the stack.
- **Versions.** Snapshot, bundle, projection and sidecar versions do not change.

The completion policies use only observation and the public catalog:
- **Swords, Doom Blade, Go for the Throat and Pongify** target the highest-power
  opposing creature they can legally target.
- **Disenchant-class spells and Reclamation Sage** target the opposing artifact
  or enchantment with the highest mana value, preferring Pacifism on their own
  creature when present.
- **Beast Within and Generous Gift** target the opposing nonland permanent with
  the highest mana value.
- **Zealot** activates when an opposing artifact or enchantment exists.
- **Swan Song** counters the first opposing enchantment, instant or sorcery
  spell it can target.

## Evidence

- Compiler positive/negative pairs:
  - Return to Dust (multi-target), Vandalblast (overload) and Bone Splinters
    (additional sacrifice cost) reject as whole cards;
  - Banishing Light ("until") rejects as a whole card;
  - `regenerationForbidden` outside destroy rejects;
  - v17 rejects every new capability.
- Reviewed runtime witnesses:
  - Swords on a commander equipped with Bonesplitter: its controller gains the
    LKI power, including the +2, then gets the 903.9a choice;
  - Swords on a token: the token ceases to exist in exile, and its controller
    gains life equal to its power;
  - Disenchant on an Equipment, which leaves the creature on the battlefield
    without its bonus;
  - Disenchant on Pacifism, after which the creature can attack;
  - Naturalize on Glorious Anthem, with P/T updated at once;
  - Beast Within on a land and on Sol Ring (its controller gets the Beast), and
    on the caster's own permanent;
  - Generous Gift whose creature target is bounced by Unsummon in response:
    the spell does nothing, and no token is created (608.2b). Equip is
    sorcery-speed (702.6a), so shroud cannot be granted in response;
  - Nature's Claim life going to the artifact's controller;
  - Feed the Swarm on a token (MV 0) and on a commander (printed MV, not tax);
    it can't target the caster's own permanents;
  - Doom Blade unable to target black creatures, and Go for the Throat's filter
    in the compiler;
  - Ravenous Chupacabra's trigger removed when every opposing creature is
    hexproof or shrouded;
  - Reclamation Sage declining its "may";
  - Viridian Zealot sacrificed as a cost: Blood Artist triggers, and Stifle
    counters the ability with Zealot already gone;
  - Smash to Smithereens with Furnace of Rath dealing 6 to the artifact's
    controller, not counted as commander damage;
  - Pongify on a token and on a commander;
  - Swan Song countering Pacifism and Day of Judgment, with the Bird going to
    their controller;
  - Swan Song unable to target creature or Equipment spells;
  - destroying a creature with an Aura and an Equipment attached, in one SBA
    wave.
- If any other listed witness proves unreachable from accepted inputs in the
  admitted pool, substitute the nearest reachable witness, cite why, and record
  the substitution as a deviation. Do not use a private injected fixture unless
  the substitute would lose the rule being tested.
- Snapshot/restore/replay equality across trace modes for the witnesses,
  including a snapshot with cards in exile.
- A frozen 18-game completion batch completes with zero engine faults or
  truncations. Its report counts:
  - destructions by permanent type;
  - exiles;
  - tokens created for a player other than the caster;
  - Swan Song counters;
  - targets rejected for hexproof or shroud.
- Earlier profiles' catalog, policy and expected IR digests and historical batch
  outputs are unchanged. `npm run check:engine` and SDK verification pass.

## Reviewed resource bounds

These capacities apply only to removal-v1. Earlier profile limits, catalog and
policy digests, fixture bytes and version numbers remain unchanged. The compiled
color-legal BG pool has 34 creature cards per seat, three more than P6.7. With
512 live tokens let C = 4×34 + 512 = **648**. This deliberately conservative live
allowance is larger than the lifetime proof below. Ceased tokens free live
capacity; their monotonic historical identities are never reused.

No admitted creature has flash and no instruction returns a creature to the
battlefield. Recursion, bounce and tuck return cards to hand/library; recasting
requires an empty stack. Thus each physical creature enters/dies and each
physical Zealot sacrifices at most once per nonempty-stack episode. Sage and
Chupacabra add two self-enters trigger sources per BG seat; the conservative
136 self-enters term covers every creature card, including both new triggers.

P6.7's mana bound is retained: at most 800 mana from 400 physical sources plus
32 from the eight recursion and eight bounce/tuck returns across a step, with
haste allowing a returned dork to tap. Destruction/exile add no mana and cannot
return a source. All mana together funds at most **208 Azure Mage activations**.
Signet input spending does not increase production. Mana abilities add no stack
entries (605.3b). Repeated commander casts and equips require an empty stack.

- Damage occurrences per combat step: 2C = **1,296**. Furnaces multiply final
  amounts without multiplying occurrences. Smash creates one separate noncombat
  occurrence. First/double-strike damage steps remain separate transactions.
- Stack plus pending triggers: **6,381** = 400 physical spell entries + 24
  inherited once-per-episode activations + four Zealot activations + 208 Mage
  activations + 8(C+52) Artist/Warden triggers + 136 self-enters triggers + four
  Bouncer self-dies triggers + one equip activation + four Rancor return triggers.
  Here 52 = 36 inherited new tokens plus 16 compensation tokens (four new
  one-token spells times four seats), in addition to initially live tokens.
  Counting mutually exclusive colors and creature trigger kinds overestimates
  the pool. Swan Song counters remove entries and do not refund costs or copy
  spells. Token recipients need not be the caster; this changes trigger
  controllers/ownership but not the total number of entries or occurrences.
- Event-group members: **916** = 400 physical cards + 512 live tokens + four
  simultaneous player losses. Destruction of noncreatures, exile and attachment
  SBAs stay within this object count. Compensation is an ordered instruction,
  not an additional member of the destruction group.
- Temporary effects: **32**, retaining P6.7's proof: 24 effects plus at most
  eight additional Bouncer deaths or Healer activations from bounce/tuck.
  New removal effects create no temporary effects and recur no spell. Static
  attachment modifiers require no temporary-effect slots.

A conservative lifetime token bound is **116**: P6.7's 36 initial tokens plus
at most eight useful creature recursions and eight bounce/tuck returns, each
adding at most four tokens, plus at most sixteen new compensation tokens:
36 + (8+8)×4 + 4×4. Raise Dead and Gravedigger return only creature cards in
one's own graveyard; none of the four compensation spells can be recurred.
A recurred Sage, Zealot or Chupacabra does not create tokens. Gravedigger cannot
choose itself when its enters trigger is placed, and commanders neither create
tokens nor recur cards. These restrictions preserve the finite recursion proof.

Token, damage, stack/pending, event-group and temporary-effect overflow in this
profile uses serializable typed resource faults with atomic event rollback;
capacity exhaustion is a host truncation, never a rules outcome. Existing
historical effect-capacity behavior remains unchanged. The completion batch
must have zero such faults and truncations.
