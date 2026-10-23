# Creature tokens and +1/+1 counters — `commander-tokens-v1`

P6.3 slice, following `commander-combat-keywords-v1`
(docs/game-engine-combat-keywords.md). Adds creature-token creation and +1/+1
counters, and releases a fourth finite legal Commander profile. Earlier
profiles, catalogs, policies and their bytes stay unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Token creation | 111.1–111.4 | An instruction creates a fixed number of creature tokens whose characteristics are exactly those the instruction defines: colors, creature type, subtypes, power, toughness and admitted keywords. The creator owns and controls each token. Its name is its subtypes plus "Token" (111.4). |
| Tokens leaving | 111.7, 111.8, 704.5d | A token that leaves the battlefield goes to its destination first, so "dies" and leave triggers see it. It then ceases to exist at the next state-based action check and can never move again. |
| Simultaneous entry | 603.6a | Tokens created by one instruction enter simultaneously as one event and are checked for enters triggers together. |
| +1/+1 counters | 122.1a, 122.2, 122.6, 613.4c | Each +1/+1 counter adds 1/1 in layer 7c, alongside Giant Growth and Disfigure. Counters are not retained when the object changes zones. |
| Summoning sickness | 302.6 | Tokens enter without continuous control; haste or attack-readiness rules apply as for cards. |
| Commander | 903.3 | Tokens are never commanders and never deal Commander damage. |

Tokens are physical objects with their own identity, distinct from the 400
seat-owned cards. They are public on the battlefield. Token identity and
creation order come from the deterministic event sequence, never from host
randomness. A bounded token capacity is a checked limit with an explicit typed
fault, sized so that the reviewed decks cannot reach it. Faults are counted
separately from completions; reaching the capacity must not silently drop a
token.

Out of scope: noncreature tokens (Treasure, Food, etc.), copy tokens, tokens
"for each" a count, -1/-1 and keyword counters, counter-moving or counter-
doubling effects, proliferate, and "target opponent creates" effects.

## IR and compiler

IR v14 adds a create-creature-tokens instruction, with fixed count and the
token characteristics above, and a put-+1/+1-counters instruction, with a fixed
amount on one target creature or on its own source. Both are admitted in spells
and in "When this creature enters" triggered abilities. Any other token or
counter shape rejects the whole card. IR v13 and earlier reject both
instructions.

## Profile

`commander-tokens-v1` uses the 59 combat-keywords-v1 definitions plus:

| Color | Card | Behaviour |
| --- | --- | --- |
| W | Raise the Alarm | Instant: two 1/1 white Soldier tokens |
| W | Midnight Haunting | Instant: two 1/1 white Spirit tokens with flying |
| W | Queen's Commission | Sorcery: two 1/1 white Vampire tokens with lifelink |
| W | Attended Knight | First strike; enters: one 1/1 white Soldier token |
| W | Gallant Cavalry | Vigilance; enters: one 2/2 white Knight token with vigilance |
| W | Seller of Songbirds | Enters: one 1/1 white Bird token with flying |
| W | Ironpaw Aspirant | Enters: +1/+1 counter on target creature |
| G | Hornet Queen | Flying, deathtouch; enters: four 1/1 green Insect tokens with flying and deathtouch |
| G | Spore Swarm | Instant: three 1/1 green Saproling tokens |
| G | Ferocious Pup | Enters: one 2/2 green Wolf token |
| G | Crested Herdcaller | Trample; enters: one 3/3 green Dinosaur token with trample |
| G | Battlegrowth | Instant: +1/+1 counter on target creature |
| G | Satyr Grovedancer | Enters: +1/+1 counter on target creature |

That makes 72 definitions. Each card's oracle text and AST must be verified
against its archive record before admission. The white deck replaces seven
Plains with the white cards (78 Plains). The black/green deck replaces six
Forest with the green cards (31 Forest, 35 Swamp). Commanders, admission,
setup and loss rules are those of the earlier profiles. The token profile alone
has the capacity bounds below; earlier profiles retain their exact limits.

## Reviewed resource bounds (P6.3 amendment)

The lifetime token capacity is 64, with a reserved physical ID range starting at
2^31 and no ID reuse. The source-backed pool creates at most nine tokens per
seat, so even an all-white or all-black/green pod creates at most 36. No admitted
spell returns or recurs a token-producing card, and neither commander produces
tokens. Ceased identities remain historical records; their game objects vanish.

The largest admitted creature pool per deck is 27 (black/green; white has 16),
including its commander. Let C = 4×27 + 64 = 172 simultaneous creatures. The
following conservative bounds deliberately include mutually exclusive deck
choices:

- Damage occurrences per step: attackers + 2×blockers ≤ 2C = **344**. Each blocker
  blocks one attacker and deals one occurrence. Attacker assignments have at most
  one edge per blocker plus one player edge per attacker (trample or unblocked).
- Stack plus pending triggers: **1,892** = 400 physical spell entries + four
  Samite Healer tap activations + 4C Blood Artist death triggers + 4C Essence
  Warden enters triggers + 108 self-enters triggers + four Bouncer self-dies
  triggers. Each Artist/Warden is singleton per seat. In one nonempty-stack
  episode, each noncommander creature card can enter and die at most once and
  total token creations are bounded by 64. Commander recasting requires an empty
  stack, so repeated command casts cannot accumulate episodes. The self-enters
  allowance exceeds the actual number of such abilities. This also covers a
  maximal simultaneous four-token instruction's sixteen Warden triggers.
- Event-group members: **468** = all 400 physical cards + 64 tokens + four player
  losses. This covers simultaneous deaths, cessation, untaps and departures;
  damage itself uses an aggregate event. Effect capacity remains 32: each BG
  deck has three temporary-effect spells; each white deck has Healing Salve and
  one once-per-turn Samite activation, and neither pool recurs/untaps them during
  a nonempty-stack episode.

Tokens, damage, stack/pending entries and event-group members have explicit
serializable typed capacity faults (`Game::typed_fault`). A rules step rolls
back atomically on overflow. Tests compute the maximum from the compiled pool
and verify these bounds and the unchanged earlier-profile limits. These are
finite admitted-pool guarantees, not bounds for arbitrary Magic cards.

## Player interface

Observations show tokens as public battlefield objects with their defined
characteristics and a token marker, and show counters on permanents. Earlier
profiles' projections keep their existing bytes: the new fields are additive
and omitted when absent. Snapshot, bundle, projection and sidecar version
numbers do not change.

## Evidence

- Compiler positive/negative pairs, including v13 rejecting the instructions
  and a "for each" token card rejected as a whole.
- Reviewed runtime witnesses:
  - spell and enters-trigger token creation, with exact characteristics, owner,
    controller and summoning sickness;
  - simultaneous creation of several tokens;
  - a token dying triggers Blood Artist, then ceases to exist;
  - a token returned to hand by Unsummon (a private witness domain, since
    Unsummon is not in the admitted pool) ceases to exist and is never
    playable from the hand;
  - tokens with flying, deathtouch and trample use the existing keyword rules;
  - a +1/+1 counter combines with Giant Growth and Disfigure in layer 7c;
  - a counter is lost on a zone change;
  - a lethal counter-modified toughness check;
  - the token-capacity fault is explicit.
- Snapshot/restore/replay equality across trace modes for the witnesses.
- A frozen 18-game completion batch with seed domain
  `commander-tokens-v1/completion-batch/` completes with zero engine faults or
  truncations, and its report counts tokens created and counters placed.
- Earlier profiles' catalog, policy and expected IR digests are unchanged, and
  `npm run check:engine` passes.
