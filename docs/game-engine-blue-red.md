# Blue and red decks — `commander-four-colors-v1`

> Historical capacity proofs: D1b-2 supersedes this slice's per-profile bounds
> with the [generic capacity policy](game-engine-capacity-policy.md). The original
> arithmetic and evidence below describe the historical release only.

P6.6 slice, following `commander-mana-v1` (docs/game-engine-mana-permanents.md).
Every profile so far plays only white and black/green decks. This slice
admits a mono-blue deck and a mono-red deck alongside them, using the blue and
red definitions already compiled for earlier scenario catalogs. Their earlier
scenario-only behaviour must now hold inside a legal four-player match, next
to every mechanic added since. It releases a seventh finite legal profile.
Earlier profiles, catalogs, policies and their bytes stay unchanged.

No new IR instruction is required. The profile's catalog stays at IR v16,
because every new record lowers with existing capabilities. If a new record
needs a capability v16 lacks, reject that card; do not widen the IR in this
slice.

## Definitions reused from earlier catalogs

The current 90-definition v16 catalog already contains Counterspell, Stifle,
Unsummon, Time Ebb, Merfolk Looter, Azure Mage, Prodigal Sorcerer, Hapless
Researcher, Lightning Bolt, Furnace of Rath, Island and Mountain. They were
verified only in scenario profiles. This slice must verify each in match mode,
with the rules below, and with reviewed witnesses.

| Card | Match-mode behaviour that needs a witness |
| --- | --- |
| Counterspell | Countering a commander spell sends it to its owner's graveyard, with the existing 903.9a command-zone choice; commander tax still counts that cast (903.8). Countering a token-making spell creates no tokens. |
| Stifle | Counters triggered and activated abilities from the newer slices: Gravedigger, Hornet Queen, Blood Artist, Commander's Sphere and Mind Stone. It never targets mana abilities (605.3b). |
| Unsummon | Bouncing a commander offers the existing 903.9b hand-or-command choice. A bounced token ceases to exist (111.7). Counters and anthem effects are lost (122.2, 611.3b). |
| Time Ebb | Putting a commander on top of its owner's library offers the existing 903.9b library replacement. A token put on a library ceases to exist. |
| Lightning Bolt | Any target, including a player, a creature, a token, or a commander; with Furnace of Rath, doubled. |
| Prodigal Sorcerer | Its tap-to-ping obeys summoning sickness (302.6). Ping plus deathtouch never mix (it has none). It kills X/1 tokens. |
| Furnace of Rath | Doubles all damage, including combat damage from first strike, double strike and trample, before lifelink and Commander-damage accounting, which use the final doubled amount (120.4b, 702.15b, 903.10a). |
| Merfolk Looter, Azure Mage, Hapless Researcher | Draw and discard abilities on the stack in match mode; Hapless Researcher's sacrifice cost interacts with Blood Artist. |

## New records

| Color | Card | Behaviour |
| --- | --- | --- |
| U | Caelorna, Coral Tyrant | Legendary 0/8; admitted commander |
| U | Storm Crow | Flying |
| U | Zephyr Sprite | Flying |
| U | Goliath Sphinx | Flying |
| U | Wall of Air | Defender, flying |
| U | Hover Barrier | Defender, flying |
| U | Dragoon's Wyvern | Flying; enters: one 1/1 colorless Hero token |
| R | Rorix Bladewing | Legendary 6/5 flying, haste; admitted commander |
| R | Raging Goblin | Haste |
| R | Goblin Trailblazer | Menace |
| R | Charging Monstrosaur | Trample, haste |
| R | Quaketusk Boar | Reach, trample, haste |
| R | Goblin Instigator | Enters: one 1/1 red Goblin token |
| R | Beetleback Chief | Enters: two 1/1 red Goblin tokens |
| R | Chimney Rabble | Haste; enters: one 1/1 red Phyrexian Goblin token |
| R | Dragon Trainer | Enters: one 4/4 red Dragon token with flying |

That makes 106 definitions. Verify each record's oracle text, AST,
Commander legality and color identity before admission. Reject any card whose
token is created for another player ("target opponent creates", e.g. Hunted
Dragon) rather than admitting it.

## Profile

The commander whitelist adds Caelorna, Coral Tyrant and Rorix Bladewing. The
white and black/green decks are those of `commander-mana-v1`. The two new
decks are:

- **Blue:** Caelorna plus the eight blue spells and creatures above
  (Counterspell, Stifle, Unsummon, Time Ebb, Merfolk Looter, Azure Mage,
  Prodigal Sorcerer, Hapless Researcher), the six new blue creatures, Sol Ring,
  Arcane Signet, Commander's Sphere, Mind Stone and Command Tower, then Islands
  to 100.
- **Red:** Rorix plus Lightning Bolt and Furnace of Rath, the eight new red
  creatures, the same four artifacts and Command Tower, then Mountains to 100.

The default pod is white / black-green / blue / red. The frozen batch covers
the default pod in every seat rotation, plus all-blue and all-red pods. Its
seed domain is `commander-four-colors-v1/completion-batch/`. Re-derive the
resource bounds for this pool with the P6.3 method. Furnace of Rath and the
red token makers change the damage and token terms, and Stifle and
Counterspell change the stack terms.

## Player interface

Nothing new is projected. Earlier profiles' projections keep their bytes, and
snapshot, bundle, projection and sidecar versions do not change. The completion
policies use only observation and the public catalog:

- Counterspell targets the most expensive opposing spell when the caster can
  pay.
- Stifle targets the first opposing nonmana ability on the stack.
- Unsummon and Lightning Bolt target the highest-power opposing creature;
  Lightning Bolt goes at the lowest-life opponent when no creature is worth it.

## Evidence

- Compiler: the 16 records lower; Hunted Dragon rejects as a whole card;
  earlier catalogs are unchanged.
- Admission: blue and red decks pass with their commanders, and a blue card in
  the red deck is rejected for color identity.
- Reviewed witnesses for each row of the reused-definitions table, in match
  mode.
- Snapshot/restore/replay equality across trace modes for those witnesses.
- The frozen 18-game completion batch completes with zero engine faults or
  truncations. Its report counts counters (spells and abilities), bounces,
  library tucks, damage doubled by Furnace, and commander casts by color.
- Earlier profiles' catalog, policy and expected IR digests and historical
  batch outputs are unchanged. `npm run check:engine` and SDK verification
  pass.

## Reviewed resource bounds

This derivation applies only to four-colors-v1. Earlier profile capacities are
unchanged. The largest color-legal creature pool is still BG at 31 per seat;
white, blue and red have fewer. With 256 live tokens let C = 4×31 + 256 = 380.
Every mana source taps, produces at most two mana, and cannot untap in a step.
No admitted haste creature has a mana ability. Bounced/tucked dorks cannot tap
again after reentry that turn (302.6); artifacts and lands cannot be bounced by
these creature-only spells. Across all 400 physical cards, at most 800 mana can
be produced in a step, including mana floated before a nonempty-stack episode.
This funds at most 200 Azure Mage activations at {3}{U}. Signet input spending
cannot increase that production bound. Mana abilities add no stack entries
(605.3b). Counterspell and Stifle are already included in physical spell entries;
countering removes entries and never generates additional copies or refunds.

- Damage occurrences per combat step: 2C = **760**. Up to four Furnaces multiply
  quantities by at most sixteen, but do not create extra occurrences. Both
  combat steps are separate transactions; exact quantities have no fixed-width
  damage ceiling. Lifelink and Commander history consume final damage amounts
  (120.4b, 702.15b, 903.10a).
- Stack plus pending triggers: **4,080** = 400 physical spell entries + 24
  once-per-episode activations (four each Healer, Looter, Prodigal, Researcher;
  eight Sphere/Mind Stone) + 200 Azure Mage activations + 4(C+36) Artist death
  triggers + 4(C+36) Warden enters triggers + 124 self-enters triggers + four
  Bouncer self-dies triggers. Each creature still enters and dies at most once
  per nonempty-stack episode: recursion, bounce and tuck return it to hand or
  library, and no flash or battlefield-return effect exists. Recasting requires
  an empty stack. The extra 36 covers new tokens even when initial live tokens
  cease. The terms deliberately overcount mutually exclusive color-legal pools.
- Event-group members: **660** = 400 physical cards + 256 live tokens + four
  player losses. Bounces, tucks and counters are ordered effects below this bound.
- Temporary effects: **32**. P6.4's 24-effect pod bound gains at most eight extra
  Bouncer deaths from the four Unsummons and four Time Ebbs. Bouncing/tucking
  Gravedigger to recur Bouncer uses the same one extra return allowance per
  bounce/tuck. No new effect recurs an instant/sorcery or untaps Samite Healer.

A deliberately conservative lifetime token proof is **100**: at most nine
initial tokens per seat (36), at most eight useful creature recursions per pod,
and at most eight bounce/tuck uses, each adding at most four tokens by returning
a producer or Gravedigger (36 + (8+8)×4). New red producers make five initial
tokens per seat; blue makes one. None of the four commanders creates tokens or
recurs cards. A Gravedigger cannot target itself while its enters trigger is
placed; targets are chosen before priority and no source dies as an enters SBA.
These observations preserve P6.4's finite recursion argument. Ceased tokens free
live capacity and keep monotonic, unreused historical identities. The existing
serializable typed capacity faults and atomic rollback remain in force; cast or
activation validation refuses a full stack before mutation.
