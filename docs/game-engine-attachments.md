# Equipment, Auras and granted keywords — `commander-attachments-v1`

P6.7 slice, following `commander-four-colors-v1` (docs/game-engine-blue-red.md).
Adds attachments (Equipment and creature Auras), keyword grants from static
abilities (layer 6), hexproof and shroud, and "can't be blocked" and "can't
attack or block" restrictions. It releases an eighth finite legal profile.
Earlier profiles, catalogs, policies and their bytes stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Equipment | 301.5a–d, 702.6a, 602.5d | Equipment enters unattached. "Equip [cost]" is an activated ability on the stack: attach to target creature you control, only as a sorcery. On resolution an illegal target means nothing happens (608.2b). Equipment can't equip itself; control of the Equipment and the creature are independent. |
| Attaching | 701.3a–d | Attaching to a new creature moves it; attaching to the creature it already equips does nothing. An Equipment that becomes unattached stays on the battlefield. |
| Auras | 303.4a–c, 702.5a, 608.3b | A creature Aura spell targets a creature as it is cast. It resolves attached to that creature, or is put into its owner's graveyard if the target became illegal. |
| Attachment SBAs | 704.5m, 704.5n | An Aura attached to an illegal object or nothing goes to its owner's graveyard. An Equipment attached to an illegal permanent becomes unattached. Both are checked in every SBA wave. |
| Granted keywords | 613.1f, 611.3a | Layer 6. Static grants from an attachment, or from a "creatures you control" enchantment, apply continuously and stop when the source leaves or unattaches. Every keyword check goes through the existing `has_keyword` helper, which must now include granted keywords. A creature that gains or loses evasion after blocks are declared keeps its block (509.1b). |
| P/T from attachments | 613.4c | Layer 7c, alongside counters, anthems, Giant Growth and Disfigure. |
| Hexproof | 702.11b | The creature can't be targeted by spells or abilities its controller's opponents control. Its own controller can still target it, including with equip. |
| Shroud | 702.18a | The creature can't be targeted by any spell or ability, including its controller's equip abilities and Auras. |
| Can't be blocked | 509.1b | The equipped creature can't be declared as a blocked attacker. |
| Can't attack or block | 508.1c, 509.1b | Pacifism's enchanted creature is never a legal attacker or blocker. |
| Rancor return | 603.6c, 603.10a | When Rancor is put into a graveyard from the battlefield, its trigger returns it to its owner's hand. If it has left the graveyard by then, nothing happens. |

Targeting changes reach every existing targeted effect: Murder, Lightning
Bolt, Unsummon, Time Ebb, Disfigure, Giant Growth, counters and Raise Dead are
all checked when targets are chosen and again on resolution. Hexproof and
shroud are evaluated with the controller of the spell or ability, not its
owner.

Out of scope: Auras that enchant non-creatures or players; Aura "for each"
counts (Blanchwood Armor, Ethereal Armor); hybrid costs (Gift of Orzhova);
"activated abilities can't be activated" (Arrest); equipment with triggers
(Mask of Memory, Explorer's Scope); umbra armor; indestructible; protection;
control-changing effects.

## IR and compiler

IR v17 adds:

- the Equipment and Aura subtypes;
- an equip activated ability with a fixed mana cost;
- an enchant-creature targeting requirement for Aura spells;
- static modify abilities on "equipped creature", "enchanted creature" and
  "creatures you control", carrying any combination of fixed +N/+N,
  admitted keyword grants (all keywords supported so far, plus hexproof and
  shroud) and the restrictions "can't be blocked" and "can't attack or block";
- the Rancor leaves-battlefield return-to-hand trigger.

Hexproof and shroud are also admitted as intrinsic keywords. IR v16 and
earlier reject all of these.

## Profile

`commander-attachments-v1` uses the 106 four-colors-v1 definitions plus:

| Card | Text |
| --- | --- |
| Bonesplitter | Equipment {1}: equipped creature gets +2/+0; equip {1} |
| Short Sword | Equipment {1}: +1/+1; equip {1} |
| Vulshok Morningstar | Equipment {2}: +2/+2; equip {2} |
| Loxodon Warhammer | Equipment {3}: +3/+0, trample and lifelink; equip {3} |
| Swiftfoot Boots | Equipment {2}: hexproof and haste; equip {1} |
| Lightning Greaves | Equipment {2}: haste and shroud; equip {0} |
| Whispersilk Cloak | Equipment {3}: can't be blocked, shroud; equip {2} |
| Holy Strength | W Aura: +1/+2 |
| Pacifism | W Aura: can't attack or block |
| Unholy Strength | B Aura: +2/+1 |
| Rancor | G Aura: +2/+0 and trample; returns to hand from graveyard |
| Spidersilk Armor | G enchantment: creatures you control get +0/+1 and have reach |

That makes 118 definitions; verify each record's oracle text, AST and Commander
legality before admission. All four decks add the seven Equipment, and each
deck adds its colored cards:

| Deck | Adds | Basics (total 100) |
| --- | --- | --- |
| White | Equipment, Holy Strength, Pacifism | 62 Plains |
| Black/green | Equipment, Unholy Strength, Rancor, Spidersilk Armor | 20 Forest, 20 Swamp |
| Blue | Equipment | 73 Islands |
| Red | Equipment | 77 Mountains |

Re-derive the resource bounds with the P6.3 method, including equip
activations on the stack and Rancor's return trigger.

## Player interface

Observations show each permanent's attachments and what it is attached to,
and the keywords and restrictions a creature currently has, including granted
ones. Blocker legality hints account for "can't be blocked", granted flying
and reach, and Pacifism. Target candidate lists already exclude illegal
targets; they now honour hexproof and shroud. Earlier profiles' projections
keep their bytes. Snapshot, bundle, projection and sidecar versions do not
change.

The completion policies use only observation and the public catalog. They:

- equip their highest-power creature with the best available Equipment at
  sorcery speed, preferring Boots or Greaves on the commander;
- cast Pacifism on the highest-power opposing creature;
- cast their other Auras on their own highest-power creature.

## Evidence

- Compiler positive/negative pairs, including Blanchwood Armor, Gift of
  Orzhova, Arrest and Mask of Memory rejected as whole cards, and v16
  rejecting the new abilities.
- Reviewed runtime witnesses:
  - equip at sorcery speed only, re-equip moving the Equipment, and equip
    onto a shrouded creature being illegal;
  - the Equipment staying when its creature dies;
  - an Aura going to the graveyard when its creature leaves (704.5m), and an
    Aura spell whose target becomes illegal before resolution;
  - Boots' hexproof stopping an opponent's Murder but not its controller's
    Giant Growth;
  - Greaves' shroud stopping both;
  - Loxodon Warhammer granting lifelink and trample, combining with Furnace
    of Rath and first strike;
  - Whispersilk Cloak unblockable;
  - Spidersilk Armor granting reach to a creature entering later;
  - Pacifism on an attacker removed before blocks;
  - Rancor returning to hand, including when Unsummon bounces its creature,
    and its trigger countered by Stifle;
  - a granted keyword lost mid-combat not undoing a legal block.
- Snapshot/restore/replay equality across trace modes for the witnesses.
- A frozen 18-game completion batch with seed domain
  `commander-attachments-v1/completion-batch/` completes with zero engine
  faults or truncations. Its report counts equips, Aura attachments,
  attachment SBAs, candidate targets excluded by hexproof or shroud, and Rancor
  returns.
- Earlier profiles' catalog, policy and expected IR digests and historical
  batch outputs are unchanged. `npm run check:engine` and SDK verification
  pass.
