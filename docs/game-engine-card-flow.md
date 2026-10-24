# Card flow, library manipulation and ramp — `commander-card-flow-v1`

P6.11 slice, following `commander-wipes-v1` (docs/game-engine-board-wipes.md).

Today the pool draws cards only through fixed-count draws (Night's Whisper,
Sign in Blood, Village Rites, looters). This slice adds the card-flow engines
Commander decks lean on:

- scry and look-and-take;
- putting cards from hand back on top of the library;
- upkeep and draw-step triggers;
- "whenever an opponent draws";
- Rhystic Study's "unless that player pays";
- discard as an additional cost;
- symmetric wheels and draws for every player.

It also adds Cultivate and Kodama's Reach, whose split search was an open
parser request. It releases a twelfth finite legal profile. Earlier profiles,
catalogs, policies and their bytes stay unchanged.

## Source archive

The archive that P6.1–P6.10 copied from (parser `41863dc`, corpus `5e97d5c4…`)
was regenerated in place on 2026-10-24. That was ordinary parser workflow,
which `docs/data_schemas.md` now documents in mtg-ecosystem as a rebuildable
snapshot. Earlier slices are unaffected: their records are committed fixtures,
and no engine check reads the live archive.

This slice pins the new archive:

| Field | Value |
| --- | --- |
| File SHA-256 | `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282` |
| Parser revision | `327b3360ea8e5cffb75b136223e75a348e903cbe` |
| Parser source SHA-256 | `316c9deaba977533329b903e9d9ba48b6d02057651702c05efa460fb651fbd3a` |
| Corpus SHA-256 | `f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738` |
| Preserved read-only copy | `/home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl` |

- Copy the new records byte-for-byte from the preserved copy, not from
  `mtg-ecosystem/generated/`, which may be regenerated again.
- Commit this archive's manifest line as a second pinned manifest beside
  `archive-manifest.json`.
- Each new record's provenance row names that manifest. Earlier records keep
  their original provenance.
- Card facts for new records come from this corpus.
- Mark the Cultivate/Kodama's Reach entry in `docs/parser-requests.md` resolved
  by parser revision `327b336`.

## Rules (pinned 2026-09-25 Comprehensive Rules)

| Topic | Rules | Engine meaning in this slice |
| --- | --- | --- |
| Drawing | 121.1, 121.2, 121.2c, 121.4, 704.5b | Cards are drawn one at a time; each draw is a separate event for triggers. When several players draw, the active player draws first, then the others in turn order. Drawing from an empty library makes that player lose at the next SBA check. |
| Scry | 701.22a–b | The player looks at the top N cards, puts any number on the bottom in any order, and the rest on top in any order. Scry 0 is no event. |
| Look and take | 401.4, 701.20b | Impulse: the player looks at the top four, puts one into their hand and the rest on the bottom in any order. Looking at cards does not reveal them. |
| Putting cards back | 401.4 | Brainstorm puts two cards from hand on top in an order its owner chooses. That order is hidden from other players. |
| Discard as a cost | 118.8, 601.2a, 601.2h, 701.9a | Tormenting Voice and Cathartic Reunion discard cards chosen from the caster's hand while paying costs. The spell itself is already on the stack (601.2a), so it can't be discarded to its own cost. The spell is uncastable with too few other cards in hand. |
| Beginning-of-step triggers | 503.1a, 504.1, 603.2 | These trigger at the start of the step, and the triggers are put on the stack before the active player gets priority. Phyrexian Arena triggers in its controller's upkeep. Howling Mine triggers in each player's draw step, after the turn-based draw. |
| Intervening "if" | 603.4 | Howling Mine's "if this artifact is untapped" is checked when it triggers and again when it resolves. |
| "Unless a player pays" | 118.12a | Rhystic Study means "that player may pay {1}; if they don't, you may draw a card". The opponent who cast the spell decides first, and may activate mana abilities to pay. Then Rhystic Study's controller decides whether to draw. A payer who has left the game doesn't pay. |
| Optional payment | 118.12a, 603.5 | Mind's Eye: when the trigger resolves, its controller may pay {1}. If they do, they draw. The trigger fires once per card an opponent draws. |
| Each player / each other player | 101.4, 121.2c | Wheel of Fortune: every player discards their hand at the same time, then each player draws seven, in the 121.2c order. Words of Wisdom: the caster draws two, then each other player draws one, in turn order. |
| Searching with reveal | 701.20a, 701.23a–b, 701.23e, 701.24 | Cultivate and Kodama's Reach search for up to two basic land cards and reveal them to every seat. One goes onto the battlefield tapped and the other into its owner's hand, then the library is shuffled. The player may find fewer. With one card found, follow the AST's from-pool selection and state the outcome in a witness. If the pinned rules support another reading, flag it. Kor Cartographer's optional search for a Plains card is not revealed. |
| Summoning sickness | 302.6 | Archivist's {T} ability needs the usual continuous-control condition. |

Out of scope:

- Fact or Fiction-style piles;
- Sylvan Library;
- Ponder's optional shuffle;
- flashback;
- delve;
- "triggers only once each turn" (Esper Sentinel's "first each turn");
- Clue and Treasure tokens;
- cumulative upkeep (Mystic Remora);
- draw replacement effects.

## IR and compiler

IR v21 adds the following.

- **Library selection.** `scry N` and `look-at` the top N of the controller's
  library. They are followed by a simultaneous split: a `from-pool` count goes
  to hand, and the `rest` go to the library bottom in any order.
- **Hand to library top.** `move-zone` of a counted selection from the
  controller's hand to the top of their library in any order.
- **Discard costs.** `additional-cost` discard of a fixed count from hand.
- **Phase triggers.** `phase` triggers for `upkeep` (controller's) and
  `draw-step` (each player's), with an optional intervening `status: untapped`
  condition on the source.
- **Draw triggers.** A `draws` trigger by `an-opponent`, firing once per card.
- **Cast triggers.** A `cast` trigger by `an-opponent`, carrying a `pays`
  condition of kind `unless` for `that-player`.
- **Optional payment.** `pay-cost` that is `optional`, followed by a `you-do`
  conditional step.
- **Each-player effects.** `discard` of the whole hand for `each-player`, and
  `draw` for `each-player` or for `each-player` filtered to `other`.
- **Split land search.** A revealed search for up to two cards, then a
  simultaneous split between battlefield (tapped) and hand.
- **Optional Plains search.** An `optional-group` search for a Plains-subtype
  card, put onto the battlefield tapped.

IR v20 and earlier reject all of these.

## Profile

`commander-card-flow-v1` uses the 168 wipes-v1 definitions plus the cards
below. Line numbers refer to the preserved new archive.

| Color | Card | Archive line | Text (abbreviated) |
| --- | --- | --- | --- |
| U | Opt | 17039 | {U} instant: scry 1, draw a card |
| U | Preordain | 25951 | {U} sorcery: scry 2, then draw a card |
| U | Brainstorm | 8172 | {U} instant: draw three, then put two from hand on top |
| U | Impulse | 37309 | {1}{U} instant: look at the top four, take one, rest on the bottom |
| U | Divination | 5887 | {2}{U} sorcery: draw two |
| U | Rhystic Study | 12509 | {2}{U} enchantment: whenever an opponent casts a spell, you may draw unless that player pays {1} |
| U | Cloudkin Seer | 1491 | {2}{U} 2/1 flying: when it enters, draw a card |
| U | Augury Owl | 10337 | {1}{U} 1/1 flying: when it enters, scry 3 |
| U | Archivist | 31549 | {2}{U}{U} 1/1: {T}: draw a card |
| U | Words of Wisdom | 31357 | {1}{U} instant: you draw two, then each other player draws one |
| B | Phyrexian Arena | 36046 | {1}{B}{B} enchantment: at the beginning of your upkeep, draw a card and lose 1 life |
| G | Llanowar Visionary | 37422 | {2}{G} 2/2: when it enters, draw a card; {T}: add {G} |
| G | Cultivate | 21034 | {2}{G} sorcery: split basic-land search |
| G | Kodama's Reach | 3296 | {2}{G} sorcery: split basic-land search |
| R | Tormenting Voice | 36756 | {1}{R} sorcery, additional cost: discard a card; draw two |
| R | Cathartic Reunion | 2291 | {1}{R} sorcery, additional cost: discard two cards; draw three |
| R | Wheel of Fortune | 25414 | {2}{R} sorcery: each player discards their hand, then draws seven |
| W | Kor Cartographer | 25006 | {3}{W} 2/2: when it enters, you may search for a Plains card, put it onto the battlefield tapped, then shuffle |
| C | Howling Mine | 31744 | {2} artifact: in each player's draw step, if untapped, that player draws an extra card |
| C | Mind's Eye | 15011 | {5} artifact: whenever an opponent draws, you may pay {1}; if you do, draw |

That makes 188 definitions. Before admission, verify each record's oracle
text, AST, power and toughness, Commander legality and color identity in the
new corpus. Cultivate has a second archive record at line 18922, "Cultivate //
Cultivate", which is excluded. Use line 21034.

| Deck | Adds | Basics (total 100) |
| --- | --- | --- |
| White | Kor Cartographer, Howling Mine, Mind's Eye | 51 Plains |
| Black/green | Phyrexian Arena, Llanowar Visionary, Cultivate, Kodama's Reach | 6 Forest, 5 Swamp |
| Blue | the ten blue cards, Howling Mine, Mind's Eye | 57 Islands |
| Red | the three red cards, Howling Mine, Mind's Eye | 59 Mountains |

The default pod and the batch pods are those of wipes-v1. The seed domain is
`commander-card-flow-v1/completion-batch/`.

### Resource bounds

Re-derive the bounds with the P6.3 method. The new terms to account for are:

- draws per episode, which libraries bound (at most 400 cards in total);
- one Mind's Eye trigger per opponent draw;
- Mind's Eye chains between seats, bounded by library size and by the mana
  each payment consumes;
- Wheel of Fortune's 28 draws and their triggers;
- Rhystic Study triggers per cast spell;
- Howling Mine triggers per draw step.

Lifetime tokens are unchanged. Earlier profile capacities stay unchanged.

## Player interface

**Privacy.**

- Only the scrying or looking player sees the cards a scry or Impulse looks at.
  Every seat sees how many cards went to the top and how many to the bottom.
- Every seat sees that Brainstorm put two cards on top, but not which cards.
- Cards revealed by Cultivate and Kodama's Reach are public.
- Library order and contents stay hidden, including from the owner after the
  effect. Remembering earlier knowledge is the agent's job, not the
  projection's.
- Discard-cost choices and discarded cards are public once discarded.

**Requests.**

- The Rhystic Study payment is requested of the paying opponent, through the
  existing mana payment and undo machinery. The draw choice is requested of
  Rhystic Study's controller.
- New request and response variants are additive. The SDK's exhaustive gates
  list them, and the attachments-pinned SDK policy fails closed on them.
- Snapshot, bundle, projection and sidecar versions do not change.

**Completion policies.** They use only observation and the public catalog.
They are completion machinery, not strategy.

- **Scry:** bottom lands when the hand holds two or more lands. Otherwise keep
  lands on top and bottom the highest-mana-value card.
- **Impulse:** take the first castable spell, otherwise a land.
- **Brainstorm:** put back lands first when holding two or more, otherwise the
  highest-mana-value cards.
- **Discard costs:** discard the highest-mana-value card it cannot cast next
  turn, otherwise a land.
- **Wheel of Fortune:** cast only with two or fewer cards in hand.
- **Rhystic Study (opponent):** pay whenever an untapped source can pay.
- **Rhystic Study (controller):** draw unless the library holds five or fewer
  cards.
- **Mind's Eye:** pay when it is not the controller's turn and a source is
  untapped.
- **Draw-and-lose-life effects** (Phyrexian Arena): no choice is involved.

## Evidence

**Compiler.** Positive and negative pairs:

- Fact or Fiction, Sylvan Library, Ponder, Esper Sentinel, Mystic Remora and
  Think Twice reject as whole cards.
- v20 rejects every new capability.

**Reviewed runtime witnesses.** Each must be reachable from accepted inputs.

- **Opt:**
  - scry 1 to the bottom, then the draw takes the next card;
  - with an empty library, the draw loses at the SBA check (704.5b).
- **Preordain and Augury Owl:** split top and bottom with chosen order,
  verified after snapshot and restore.
- **Brainstorm:**
  - puts two chosen cards on top in the chosen order;
  - the next draws take them;
  - other seats see only counts.
- **Impulse:**
  - with fewer than four cards in the library;
  - the order of bottom cards is preserved.
- **Tormenting Voice and Cathartic Reunion:**
  - uncastable with too few other cards in hand;
  - the discard is public;
  - the spell is not in hand during payment.
- **Rhystic Study:**
  - the opponent pays using a land tapped during the request;
  - the opponent declines and the controller draws;
  - the controller declines;
  - the payer concedes during the request.
- **Mind's Eye:**
  - triggers once per card from a Words of Wisdom or a Wheel;
  - two Mind's Eyes in different seats chain until mana runs out.
- **Howling Mine:** each player draws an extra card in their own draw step.
- **Phyrexian Arena:**
  - triggers in its controller's upkeep;
  - kills its controller at 1 life;
  - with an empty library, its controller loses by drawing.
- **Wheel of Fortune:**
  - discards are simultaneous;
  - draws happen in active-player-first order;
  - a player with fewer than seven cards in library loses.
- **Cultivate and Kodama's Reach:**
  - two basics: one goes to the battlefield tapped, one to hand;
  - one basic;
  - zero basics;
  - the revealed cards are public in every seat;
  - the shuffle uses the existing seeded private shuffle.
- **Kor Cartographer:** declining, and accepting.
- **Archivist:** summoning sickness.
- **Inherited mechanics:** first and double strike, blocker hints, sacrifice
  payoffs and attachments still work in this profile.

**Substitutions.** If a listed witness proves unreachable, substitute the
nearest reachable one, cite why, and record a deviation. Do not use a private
injected fixture unless the substitute would lose the rule.

**Replay.** Snapshot, restore and replay are equal across trace modes for the
witnesses. That includes snapshots inside a scry choice, inside a Brainstorm
put-back, and inside the Rhystic Study payment.

**Batch.** A frozen 18-game completion batch completes with zero engine faults
or truncations. Its report counts:

- cards drawn per seat;
- scry top and bottom counts;
- Rhystic payments and draws;
- Mind's Eye draws;
- Wheel casts;
- deck-out losses;
- lands fetched by Cultivate and Kodama's Reach.

**Unchanged outputs.** Earlier profiles' catalog, policy and expected-IR
digests and historical batch outputs are unchanged.

**Checks.** `npm run check:engine` and SDK verification pass.
