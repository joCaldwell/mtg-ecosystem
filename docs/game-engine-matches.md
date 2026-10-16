# Commander foundations profile

This contract names the first finite legal-deck release,
`commander-foundations-v1`. Implementation and verification passed; retained
artifact identities and checks are recorded in `game-engine-progress.md`.
It supports four-player free-for-all Commander with explicit open decklists.
The release does not claim representative Commander strategies.

## Frozen inputs and admission

The exact 34-definition IR v11 catalog has SHA-256
`fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f`.
Its 41 semantic source records extend the previous catalog with Wall of Wood's
complete Defender ability. Wall of Vines is rejected as a whole card because
Reach is outside this profile. The source-backed construction and ban policy
has digest `cfe83bd72e477756a7b4f778e22c6ec8e901489671d84d122574ecdc7b90caad`.
It binds 42 named bans, seven offensive-card exclusions and the reviewed
positive pool to their exact archived source objects. Companion-only policy
is distinct from a whole-card ban. Policy facts are separate from semantic
lowering records. The current policy was checked against the official
[banned and restricted list](https://magic.wizards.com/en/banned-restricted-list).

`MatchConfig` names the profile, four decklists, starting seat and a trusted
32-byte seed. A decklist includes its commander in its exactly 100 cards.
Admission checks supported commander eligibility, one commander, positive
counts, singleton English names, full color identity, independent basic-land
production restrictions and exact definition closure before allocating objects.
Known banned cards receive a banned diagnostic before outside-pool lookup;
other unsupported cards receive an unsupported diagnostic. Duplicate basic-land
rows normalize to the same configuration. The whitelist admits Isamaru, Hound
of Konda and Yargle and Multani as commanders; it does not classify all other
commanders as illegal under general Magic rules.

The default white deck contains one each of Isamaru, Yoked Ox, Samite Healer,
Sacred Nectar, Healing Hands and Healing Salve, plus 94 Plains. The default
black/green deck contains one each of Yargle and Multani, Cruel Edict, Grizzly
Bears, Fog, Blood Artist, Giant Growth, Essence Warden, Disfigure, Soul Feast,
Bouncer, Elvish Visionary and Wall of Wood, plus 44 Forest and 44 Swamp.
The default pod is white/black-green/white/black-green. All-white and all-black/
green pods are also admitted. Blue, red and multicolor definitions in the
shared engine catalog are not automatically legal members of either deck.

`Prepared::admit_pod` is the legal constructor. Injected scenario zones, life,
mana and tax histories cannot enter match mode. Physical IDs are canonical:
each seat owns 100 cards, its commander starts in command and its remaining
99 cards start in the library. All players start at 40 life.

## Setup and trusted randomness

Setup follows the pinned September 25, 2026 rules, especially CR 103.5,
103.5c, 103.8c and 903.6–903.7. The starting seat anchors declaration order.
All unfinished players declare before any redraw. Each redraw returns the
hand, shuffles and draws seven; paid mulligans bottom the required number
before the next declaration round. The first multiplayer mulligan is free.
After eight mulligans a player bottoms seven and keeps an empty hand.
Bottom inputs preserve explicit top-to-bottom order within the library bottom.
The starting player takes the normal first-turn draw in multiplayer.

`Mulligan.bottom_if_mulligan` states the cost of the next redraw;
`BottomOpening.count` states the current required bottom input.
Concessions remain trusted recorded inputs and revalidate pending setup choices.
If the chosen starter leaves before turn one begins, the next living seat
starts turn one. Departures after a turn starts preserve ordinary turn handling.

The core uses pinned `rand_chacha` and `rand_core` 0.10.0 with default features
disabled. ChaCha20 receives only the explicit host seed, stream zero and a
versioned canonical word position. A checked 2^68-word exhaustion sentinel
prevents wraparound. Descending Fisher–Yates uses rejection sampling, charging
each random word to the work budget. Known vectors and continuation restores
are tested. The core never reads OS entropy or a clock.

Automatic initial shuffle/draw and complete redraw rounds expose no player
queries or intermediate observation history. Ready setup requests expose only
permitted information. Actual seed, random cursor, hidden library order and
opponents' hands remain trusted-host state. SearchContext continues to support
ordinary priority roots; setup reconstruction is outside this release's search
contract.

## Composition and evidence

Defender excludes an attacker even when temporary effects give it positive
power. Match-only capacities are reviewed against all-white and all-black/green
reachable compositions: 256 pending/stack entries, 64 combat damage occurrences,
32 effects and 400 members in an atomic event group. Historical scenario limits
and semantic references remain separate.

When several commanders have fresh graveyard/exile return opportunities,
owners choose in APNAP order before one simultaneous return wave. The next
required chooser sees prior answers through public commander designations;
no card moves early. Declines consume the current incarnation's opportunity.
Departures remove missing participants without resurrecting their cards.
Snapshot v17, bundle v18, projection/context v15 and query-sidecar v14 retain
these continuations and setup randomness.

The acceptance batch is frozen before execution: seed indices 0 through 15,
starting seat `index % 4`, mulligans `(index + seat) % 3`, plus all-white index
16 and all-black/green index 17. Each seed is SHA-256 of the ASCII domain
`commander-foundations-v1/completion-batch/` followed by the decimal index.
All 18 required games must complete with zero engine faults or host truncations.
The simple deterministic policies use only the acting player's observation and
the public compiled catalog. They are completion machinery, not strong agents.

Each game has a 1,000-turn, 100,000-decision and shared 2,000,000 host-call cap;
advance and every initial/resumed query call charge before execution. The batch
uses quantum 1,024 and fresh evidence directories. Every result is retained
before acceptance and independent replay checks. Live journals stream accepted
inputs so interrupted runs can recover a prefix without claiming the exact
interruption boundary. Completed evidence binds winner, typed loss sequence,
survivors, terminal digest and selected setup/midgame/final snapshots to the
same accepted journal. Timing and filesystem paths are host diagnostics.

A separate all-white, no-action index-18 witness checks natural failed draws
through pod completion. Separate source-labeled witnesses check simultaneous
Yargle combat deaths and a pumped Wall of Wood; these do not replace any fixed
completion seed. Full debug, release, Node, adversarial, retained-source offline
rebuild and independent replay gates must pass before the release claim.


The long no-action witness requires retaining the complete observation history.
State digests therefore emit the same sorted canonical JSON bytes incrementally,
canonicalizing one history view at a time. All nonhistory State/Knowledge fields
still come from the ordinary serializer. The generic canonical implementation
remains the exact test oracle; no history entry is dropped or coalesced. The
initial full-Value attempt and its streamed accepted-input journal are retained
as resource-failure diagnostics, separately from completed evidence.
