# Mana rocks, dorks and Command Tower — `commander-mana-v1`

P6.5 slice, following `commander-staples-v1` (docs/game-engine-staples.md).
Adds artifacts, colorless mana, and mana abilities on nonland permanents:
fixed, any-color and commander-identity mana, including a mana ability that
has its own mana cost. It also adds one nonbasic land. It releases a sixth
finite legal profile. Earlier profiles, catalogs, policies and their bytes stay
unchanged.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Colorless mana | 106.1b, 107.4c | Colorless is a sixth mana type. It pays generic costs; no admitted cost requires {C}. |
| Mana abilities | 605.1a, 605.3a–c, 601.2g | Activated abilities that add mana without targets are mana abilities. They don't use the stack and may be activated with priority or during a mana payment, including while casting or activating. |
| Mana ability with a mana cost | 605.3a, 601.2g | Golgari Signet's {1} is paid from mana already available, recursively within the same payment window; mana it adds is then available. Tapping is part of the cost. |
| Choice of color | 106.4, 605.3b | "One mana of any color" and "any color in your commander's color identity" are chosen as the ability resolves, by its controller. |
| Commander identity | 903.4, 903.4a, 903.4f, 106.5 | Identity comes from the commander card as the game began. Every admitted pod has one commander per player. If identity were undefined, the ability would add no mana. |
| Summoning sickness | 302.6, 602.5a, 702.10c | A creature's {T} abilities need continuous control since its controller's most recent turn began, unless it has haste. Artifacts and lands are unaffected. |
| Sacrifice-to-draw abilities | 602.2, 701.21a | Commander's Sphere and Mind Stone use the stack, like the existing Hapless Researcher activation. Sacrifice is a cost. |
| Deck construction | 903.5c–d | Colorless artifacts and Command Tower fit any color identity. |

Mana pools empty at the end of each step and phase, as before (106.4).

Out of scope: {C}-specific costs, mana abilities with targets or triggered mana
abilities (Utopia Sprawl), "could produce" mana (Fellwar Stone), damage-dealing
mana abilities (Talismans), no-maximum-hand-size statics (Thought Vessel is
unparsed), filter lands, and partner/multiple commanders.

## IR and compiler

IR v16 adds the artifact card type. It also adds activated mana abilities on
creatures, artifacts and lands with:

- costs: tap-self, an optional fixed mana cost, and an optional sacrifice-self;
- effects: fixed symbols (including C), one mana of any color, or one mana of
  a color in the controller's commander's color identity.

Non-mana activated abilities that sacrifice the source and draw a card reuse
the existing source-sacrifice activation path. IR v15 and earlier reject these
abilities.

## Profile

`commander-mana-v1` uses the 81 staples-v1 definitions plus:

| Card | Text |
| --- | --- |
| Sol Ring | Artifact {1}: {T}: Add {C}{C}. |
| Arcane Signet | Artifact {2}: {T}: Add one mana of any color in your commander's color identity. |
| Commander's Sphere | Artifact {3}: commander-color mana ability; Sacrifice: Draw a card. |
| Mind Stone | Artifact {2}: {T}: Add {C}. {1}, {T}, Sacrifice: Draw a card. |
| Golgari Signet | Artifact {2}: {1}, {T}: Add {B}{G}. |
| Command Tower | Land: {T}: Add one mana of any color in your commander's color identity. |
| Llanowar Elves | Creature {G} 1/1: {T}: Add {G}. |
| Elvish Mystic | Creature {G} 1/1: {T}: Add {G}. |
| Birds of Paradise | Creature {G} 0/1: Flying; {T}: Add one mana of any color. |

That makes 90 definitions; verify each record's oracle text, AST and Commander
legality before admission. The white deck adds Sol Ring, Arcane Signet,
Commander's Sphere, Mind Stone and Command Tower, with 71 Plains
(29 + 71 = 100). The black/green deck adds all nine, with 24 Forest and 26
Swamp (50 + 50 = 100).

## Player interface

Mana windows offer every activatable mana ability the player controls, not
only basic lands. An ability that needs a color choice asks for it from the
legal set: all five colors, or the commander identity colors. A mana ability
with a mana cost opens a nested payment that can be cancelled without side
effects, using the existing keep-all/undo-all failed-payment rules. Undo
reverses only receipt-recorded mana and untaps surviving sources, as with
lands. Earlier profiles' projections keep their bytes. Snapshot, bundle,
projection and sidecar versions do not change.

The completion policies use only observation and the public catalog. They tap
rocks and dorks before lands when that lets them cast a more expensive spell,
and otherwise follow the existing spending order.

## Evidence

- Compiler positive/negative pairs, including Fellwar Stone, Talisman of
  Hierarchy and Utopia Sprawl rejected as whole cards, and v15 rejecting the new
  abilities.
- Reviewed runtime witnesses:
  - Sol Ring paying generic costs;
  - Arcane Signet and Command Tower offering exactly the commander's identity
    colors (white only for Isamaru; black or green for Yargle);
  - Birds offering five colors;
  - Golgari Signet paid with mana from a land inside the same cast, and
    undone cleanly;
  - Llanowar Elves unusable the turn it enters, usable next turn;
  - Mind Stone and Commander's Sphere sacrifice-draw on the stack, responded
    to with Murder;
  - mana emptying between steps;
  - Command Tower legal in a white deck, with deck construction rejecting a
    black/green card in that deck.
- Snapshot/restore/replay equality across trace modes for the witnesses,
  including restore inside a nested mana payment.
- A frozen 18-game completion batch with seed domain
  `commander-mana-v1/completion-batch/` completes with zero engine faults or
  truncations. Its report counts mana-ability activations by source type and
  the turn of first commander cast.
- Earlier profiles' catalog, policy and expected IR digests and historical
  batch outputs are unchanged, and `npm run check:engine` passes.
