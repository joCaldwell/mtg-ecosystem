# Evergreen creature keywords — `commander-keywords-v1`

P6.1 slice. Adds six intrinsic creature keywords to the compiler, IR and
runtime, and releases a second finite legal Commander profile whose decks use
them. `commander-foundations-v1` stays byte-for-byte reproducible: its IR v11
catalog, policy fixture, digests, projection bytes and search/server scope do
not change.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Keyword | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Flying | 702.9b, 509.1b | An attacker with flying can be blocked only by a creature with flying and/or reach. Checked per blocker/attacker pair when blockers are declared. |
| Reach | 702.17b | Lets its creature block an attacker with flying. No other effect. |
| Vigilance | 702.20b, 508.1f | Declaring the creature as an attacker does not tap it. |
| Haste | 702.10b–c, 302.6 | The creature may attack, and may pay tap/untap costs, without continuous control since its controller's most recent turn began. |
| Lifelink | 702.15b–e, 120.3f, 510.2 | Damage dealt by the source also makes its controller gain that much life, as part of the same simultaneous damage event, using the final post-replacement/prevention amount. Simultaneous lifelink sources produce separate gain results. |
| Deathtouch | 702.2b, 704.5h | A creature with toughness > 0 that was dealt nonzero damage by a deathtouch source since the last SBA check is destroyed as a state-based action. Uses final dealt damage, so fully prevented damage does not count. |

Multiple instances are redundant (702.2f, 702.9c, 702.10d, 702.15f, 702.17c,
702.20c). Keyword presence is read through one runtime helper
(`has_keyword(object, keyword)`), never by inspecting definitions at each call
site. Damage from a source that has left uses last-known information
(702.2e, 702.15c). Evasion is evaluated at declaration only (509.1b); no
effect in the pool grants or removes keywords.

Lifelink and damage combine within one event. A player at 2 life who takes 3
combat damage while their lifelink creature deals 2 damage ends at 1 life
before SBAs are checked. Commander combat damage is still recorded in full.

Deathtouch is consumed by the next SBA check. No permanent in the pool has
indestructible or regeneration, so a flagged creature with toughness > 0 is
always destroyed at that check, and a toughness ≤ 0 creature is put into the
graveyard by 704.5f in the same wave. The deathtouch-damage fact is therefore
never live on a battlefield object after a completed SBA wave; tests assert
this invariant. Admitting indestructible or regeneration later requires
explicit flag clearing at every SBA check.

Out of scope: trample, first strike, double strike, menace, keyword counters,
keyword-granting or -removing effects, protection, and any change to the
foundations profile, search capture or game server.

## IR and compiler

IR v12 extends `KeywordAbility` with `flying`, `reach`, `vigilance`, `haste`,
`lifelink` and `deathtouch`, each with its own capability. IR v11 stays
Defender-only, and v3–v10 still reject keywords. The compiler lowers each bare
keyword atom of a creature face. Any other keyword, or one with parameters,
rejects the whole card. Former negative tests that used Flying or Trample as
the unsupported example now use keywords that remain unsupported.

## Profile

`commander-keywords-v1` uses a separate 50-definition IR v12 catalog: the 34
foundations definitions plus the 16 source records below, taken byte-for-byte
from the pinned oracle AST archive (same manifest as every existing fixture).

| Color | Card | Keywords |
| --- | --- | --- |
| W | Serra Angel | Flying, Vigilance |
| W | Healer's Hawk | Flying, Lifelink |
| W | Ajani's Sunstriker | Lifelink |
| W | Alpine Watchdog | Vigilance |
| W | Angelic Wall | Defender, Flying |
| B | Vampire Nighthawk | Flying, Deathtouch, Lifelink |
| B | Typhoid Rats | Deathtouch |
| B | Banehound | Lifelink, Haste |
| B | Crazed Skirge | Flying, Haste |
| B | Child of Night | Lifelink |
| G | Giant Spider | Reach |
| G | Canopy Spider | Reach |
| G | Deadly Recluse | Reach, Deathtouch |
| G | Greenwood Sentinel | Vigilance |
| G | Dragon Sniper | Reach, Vigilance, Deathtouch |
| G | Wall of Vines | Defender, Reach |

The keywords policy is a separate fixture with the same ban and offensive-card
sources and its own 50-row admitted-pool review. Commanders remain Isamaru and
Yargle and Multani. The white deck adds the five white cards to the foundations
white nonland cards with 89 Plains. The black/green deck adds the eleven
black/green cards to the foundations black/green nonland cards with 38 Forest
and 38 Swamp. Admission, setup, mulligans, randomness, capacities and loss
rules are those of the foundations profile. Profile identity (name, catalog
digest, IR version, policy, commander whitelist, default decks) comes from one
per-profile table rather than duplicated constants.

## Player interface

The blockers request for this profile tells each defending player, for each
candidate blocker, which attackers it can legally block. Foundations requests
keep their existing bytes. Snapshot, bundle, projection and query-sidecar
version numbers do not change: new state and request fields are additive and
omitted when empty, so every foundations byte and digest is unchanged.

The completion policies use only the acting player's observation and the public
catalog. The develop policy blocks when a legal blocker survives the block or
has deathtouch; otherwise it declines. Policies remain completion machinery,
not strategy.

## Evidence

- Compiler: each new card lowers to the expected keywords; mutations with
  unsupported or parameterized keywords reject; v11 catalogs reject the new
  keywords; foundations IR is unchanged.
- Runtime witnesses with reviewed expected results:
  - flying attacker cannot be blocked by a ground creature, can be blocked by
    reach or flying;
  - a vigilance attacker stays untapped;
  - a haste creature attacks the turn it enters, and a creature without haste
    still cannot;
  - lifelink gains are simultaneous with damage, follow Fog, Samite Healer and
    commander damage, and are separate per source;
  - deathtouch destroys a high-toughness blocker or attacker, not when the
    damage is fully prevented;
  - Defender plus flying/reach can block but never attack.
- Snapshot/restore/replay equality across trace modes for each witness.
- A frozen 18-game completion batch with seed domain
  `commander-keywords-v1/completion-batch/` (same seat/mulligan pattern as
  foundations) completes with zero engine faults or truncations.
- Foundations catalog, policy and expected IR digests are unchanged, and
  `npm run check:engine` passes.
