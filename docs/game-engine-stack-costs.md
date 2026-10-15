# Stack targets, counters and additional activated costs

Contract frozen October 15, 2026 after independent source, rules and profile
review. This continues the exact action/payment
profiles with card-spell and nonmana-ability stack targets, countering, a tap
activation cost and a mana activation cost. It does not admit general triggers,
uncounterable objects, arbitrary activated costs or legal Commander matches.
Earlier pools, profiles and gameplay fixtures retain their original semantics.

## Frozen inputs and executable vocabulary

Use the historical schema-3 archive with retained manifest SHA-256
`b20406acec7b9af4610324af8848c76a2966315f1bb4043de2b03d53aa1a6f1b`.
Independent review verified all four complete records and their ASTs:

| Card | Oracle identity | Source index | Source SHA-256 |
| --- | --- | --- | --- |
| Counterspell | `cc187110-1148-4090-bbb8-e205694a39f5` | 30694 | `d4bd6c5d1470721752426189b105655dc650c11e9277bcdf81ac519c397d2ea2` |
| Stifle | `b3b00911-ece7-4484-bc36-f211ce72b6cc` | 27004 | `ff9b29de13defa88690e55d6cfee7e7686062731c0a8dde954ac9c9f28f62665` |
| Merfolk Looter | `67362406-b1ca-49e2-800d-9050bfe8742a` | 15469 | `e5cc9faa5c2cad2e4800a6931a54506f90820d7b4c4a668162dfb13b0c11c4a9` |
| Azure Mage | `bbf9fd47-10e1-4749-b06f-4fe72200afc7` | 28212 | `fc5671620b35d5f63698b8994b730641fe5e2d9824ee9d31fe5270cdf7a03791` |

Exact raw record SHA-256 values, in the same order, are
`1870e08a76c871aaa537cb0061edd894ce88b98af62c6c5db8c2745a85275656`,
`2cd78366919e27100154fdff2b7537a49d13e48f3d234196ed5a006558b405bf`,
`7aa11f7f6acb6bd235123f70b319943d55fe8277c34214375bbc75d0b7830fb0`,
and `8f352d11379d036fc16c2b4786055f137bab170a218d59866706f40b5aa03a83`.
The corresponding AST SHA-256 values are
`8369b05d2e6bba06abdb738be5eb1d3dd1853350725b6e2ad5e8739b12409ab0`,
`c0828a3ba76f867d98971cc7d6f4ce5beefafc0c3adecaa36ab4753f35d46f12`,
`4d8264216d2b613c00056c0574dac7a0289743c579ced4ff68808d6f409ec59c`,
and `059433c7906888d46bd837fa2fdb5a143df73dde8931ebb6f2f538a355f5f051`.

Counterspell is a normal {U}{U} instant countering one spell. Stifle is a normal
{U} instant countering one activated or triggered ability; its mana-ability
target exclusion remains part of the source contract. Merfolk Looter is a
normal {1}{U} 1/1 whose tap ability draws one card, then discards one. Azure Mage
is a normal {1}{U} 2/1 whose {3}{U} ability draws one card. Neither creature has
an additional source clause, timing restriction or keyword in these records.
Lower their complete spell and ability semantics or reject the card.

Extend the vocabulary with `StackSpell`, `StackActivatedOrTriggeredAbility`,
`CounterStackObject`, `TapSource` and a mana activation cost. Stifle's source
target union must not be silently narrowed to only activated abilities: current
exact admission supplies activated entries only, while general triggered
ability production remains unsupported. Intrinsic land mana abilities resolve
immediately and never become stack-target candidates. No source-specific
runtime dispatch, parser rerun, card refresh or raw-text execution fallback
is introduced.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
The pinned bytes independently hash to
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules include 113.7a/8, 115.5, 117.3–5, 302.6, 400.7, 601.2,
602.2/5a, 605, 608.2b/m/n, 701.6, 704.3–4, 733.1–2, 800.4 and 903.8/9a.

## Exact admission and profiles

The new catalog has exactly fifteen complete definitions: the prior eleven
action definitions plus these four additions. IR schema v6 is required for the
new targets/counter instruction and cost variants. Sixteen source records retain
Blood Artist as a paired unsupported trigger fixture; it is not admitted.
Earlier catalog families retain their own exact source/definition identities.

All four profiles use four seats at 40 life, turn 1 with P0 active in precombat
main and holding priority; empty stack, pools and histories; unused land play;
incarnation 0; untapped objects; and no control grants. Initial battlefield
creatures meet continuous-control eligibility. Explicit spending, ordinary
combat, Commander lifecycle and explicit ordered draws are enabled. P0 has
fifteen owned objects in the base profile; no player exceeds sixteen.

### `stack_costs()`

| IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Isamaru | command; designated commander |
| 1–4 | P0 | Island | battlefield |
| 5 | P0 | Plains | battlefield |
| 6 | P0 | Forest | battlefield |
| 7 | P0 | Merfolk Looter | battlefield |
| 8 | P0 | Azure Mage | battlefield |
| 9 | P0 | Hapless Researcher | battlefield |
| 10 | P0 | Grizzly Bears | hand |
| 11 | P1 | Counterspell | hand |
| 12 | P1 | Stifle | hand |
| 13 | P1 | Unsummon | hand |
| 14 | P1 | Time Ebb | hand |
| 15–18 | P1 | Island | battlefield |
| 19 | P2 | Counterspell | hand |
| 20,21 | P2 | Island | battlefield |
| 22 | P3 | Island | battlefield |
| 23,24 | P0 | Island, Forest | library, ordered **23,24** |
| 25,26 | P1 | Island | library, ordered **25,26** |
| 27,28 | P2 | Island | library, ordered **27,28** |
| 29,30 | P3 | Island | library, ordered **29,30** |
| 31,32 | P0 | Island | battlefield |

`stack_costs_borrowed()` changes only creatures 7/8/9 to owner P2 while P0
remains their default controller. `stack_costs_empty_library()` instead removes
P0 library cards 23/24 entirely. `stack_costs_library()` instead starts Isamaru
0 on the battlefield. Every other fact remains the base profile's exact setup.

`StackTargets { entries }` uses opaque viewer-scoped stack handles in player
responses and trusted stack IDs only inside the host API. Every new-profile
visible stack entry has its own optional general handle; an activated ability's
existing ability handle is the same identity, not another target. Retain
announced and current stack-target references separately. Unpaid spell/ability
announcements have no ordinary priority window, and lower completed entries
retain their identity throughout those announcements.

Recovery advances to snapshot v12, bundle v13, projection/context v10 and query
sidecar v9. IR v6 applies only to the new exact catalog; earlier catalogs and
profiles retain their separately versioned executable behavior.

## Stack target identity and countering

Stack targets bind the specific stack instance, using viewer-scoped opaque
stack handles. Keep these bindings separate from physical source IDs,
incarnation-bound object targets and player targets. A card's source handle or
a historical announced source reference cannot authorize a stack target.
Every candidate must be a current completed entry of the required kind; the
announcing spell itself is never eligible (CR 115.5). Counterspell may target
another spell controlled by its caster. Stifle cannot target a card spell or
intrinsic mana ability; Counterspell cannot target a noncard ability.

Use the existing announcement, mana window, exact spending and all-pass stack
machinery. No opponent receives priority during target selection or payment.
Lower entries remain completed while an upper counter spell is announced.
Resolve top to bottom; a legal counter removes only its bound target. A
countered card spell moves to its owner's graveyard. A countered noncard ability
ceases to exist without moving its physical source. Countering never refunds
mana, source taps or sacrifice costs (CR 701.6b), and no countered effect runs.

Apply the usual whole-spell target recheck once when resolution begins. If the
bound target is gone, the counter spell does not resolve and does not rebind to
another stack entry, a recast card, or the target's current physical source.
Counter wars, source/target departures and another counter removing a lower
target must preserve the other completed entries and their historical target
references. Started invocations retain their detached frozen program under
CR 608.2m if their own source subsequently leaves.

A countered commander actually enters its owner's graveyard. Its owner's
fresh-entry return choice occurs at the next SBA checkpoint, after the complete
counter spell has finished, rather than during its counter instruction. The
completed command-origin cast remains in physical per-caster history even
though countered; a later command cast pays the increased tax. Decline consumes
the new graveyard incarnation's return opportunity once. Owner departure removes
the card, never resurrects it or turns countering into a direct command move.

## Tap and mana activation costs

Merfolk Looter uses a tap-source cost with no mana component. Require a current
untapped controlled battlefield source that has been controlled continuously
since its controller's most recent turn began. A newly cast or bounced/recast
Looter cannot activate immediately; tapped or stale incarnations reject without
mutation. A legal activation atomically creates a noncard stack entry, taps the
source and completes activation, then stabilizes before returning priority to
the activator. No artificial zero-cost mana window is added. It resolves through
the existing draw/mandatory-discard continuation and nested Commander returns.

Azure Mage's cost is mana only: {3}{U}, with no tap or summoning-sickness
restriction. A new Mage may activate immediately and multiple times if every
activation is paid. Proposal creates a distinct incomplete noncard stack
instance with frozen source, ability program and controller. Its mana window
permits only the activator's intrinsic mana abilities, followed by the existing
explicit exact spending decision. No other player receives priority until the
activation completes or reverses. Each successful activation gets its own stack
identity and independently resolves or can be countered.

Source functioning zone and control are checked when the ability is proposed.
After a mana-only ability has been announced, the physical source may leave
while its surviving activator pays: the mana cost does not require moving or
rechecking the source. Preserve the frozen ability and pay from the surviving
activator's pool. A borrowed source owner's departure must not abort that
announcement or remove a completed ability. A departing activator removes their
noncard stack entries; a started resolution still follows CR 608.2m, with
impossible personal draws/discards skipped rather than redirected.

An unpaid activation that cannot complete reverses only that announced noncard
entry and its own attempted payment. Preserve the original source zone,
incarnation, tapped/readiness facts, prior mana and unrelated lower stack entries.
Keep-all or undo-all applies only to receipt-recorded mana abilities activated
during this attempt; undo surviving land taps/mana exactly, never resurrect a
removed source or player. Retained legal mana activations interrupt prior passes.
A payable attempt cannot be canceled through `FinishMana`; an invalid spending
map rejects in place. Distinguish spell-origin and activated-origin announcement
frames so reversal cannot move a creature card from battlefield to hand.

Current sources have separate tap-only, mana-only and sacrifice-only costs.
Combined tap-plus-mana, additional sacrifices, alternate costs, reducers and
general activated-cost ordering are not admitted incidentally.

## Required evidence

Freeze exact scenario tables, catalog identity, executable schema and recovery
versions before acceptance. Preserve all earlier exact catalogs and profiles.
Require complete-source positive and paired invalid/unsupported compiler checks.

Verify stack target kind separation, self-target rejection, own-other-spell
targeting, stale stack handles, counter wars, targets disappearing before and
after accepted target input, countered Commander grave/SBA/tax history, and
countered noncard abilities without refunds. Check taps and mana still paid when
Stifle removes an ability, and source departure without deleting its targetable
completed noncard entry.

Verify Looter readiness, repeated tapped activation rejection, actual newly cast
rejection and next-turn eligibility; Azure immediate/repeated activations,
successful exact spending and failed-payment keep/undo; source-owner/activator
departures during mana and spending; preserved lower entries/passes during
reversal; historical versus current source projections; and nested draw/discard
and Commander replacement continuations. Countering an ability must not falsely
emit an ordinary successful ability-completion event or perform its draw.

At every new choice and decisive rules boundary compare uninterrupted,
one-unit-budgeted, cloned, restored and replayed execution, observations, query
work and actual outcomes. Querying cannot mint handles or mutate semantic state.
Reject rehashed forgeries of stack kind/identity, target bindings, activation
cost payment, historical source, continuation cursors and readiness facts.
Off/compact/detailed traces must agree semantically. Complete formatting,
Clippy, debug/release workspace tests, cross-language fixture checks,
source-to-runtime demonstrations and a fresh retained offline rebuild/replay.
