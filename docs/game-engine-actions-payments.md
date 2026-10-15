# Explicit mana spending, opponent sacrifice and self-sacrifice abilities

Contract frozen October 15, 2026 after independent source, rules and profile
review. This is a bounded P3 continuation of the
Commander damage/library slice. It adds three connected mechanic families:
explicit mixed-color spending, an opponent-targeted sacrifice sorcery, and one
nonmana activated ability whose source is sacrificed as its cost. Arbitrary
decks, general activated costs, triggers, replacement ordering and continuous
effects remain outside admission. Historical profiles and their exact catalogs
retain their reviewed behavior.

## Sources and executable scope

Use the historical schema-3 archive with retained manifest SHA-256
`b20406acec7b9af4610324af8848c76a2966315f1bb4043de2b03d53aa1a6f1b`.
Independent review verified the following complete records against that archive:

| Source | Oracle identity | Source index | Source SHA-256 |
| --- | --- | --- | --- |
| Swamp | `56719f6a-1a6c-4c0a-8d21-18f7d7350b68` | 12962 | `51c17e4939aa3f3fcce238d3a1d6f7d4646c218d8bb124c07d66b5e1ce4736d1` |
| Cruel Edict | `10c585c4-bf5b-4d8f-94a9-e9a5036a688f` | 2558 | `20646c8a1569e733a8d1db930fc6ed4fa1d3dfa6c3030fea0bb1d2c09c9c3f0a` |
| Hapless Researcher | `240a349a-79f8-4e11-9a05-a78148e70a62` | 5423 | `9b7c1f4bbd45e5a5b2d96856a028d25eb23035c87c42866874fb74d8d20c2610` |

Swamp's exact raw record SHA-256 is
`bcbe2c30e38a590cbd441c564586532e3d0e6dc1fc4cedcdb363a95d96983dd7`;
its empty AST is the previously reviewed
`cf1cbb66a638b4860a516671fb74850e6ccf787fe6c4c8d29e9c04efe880bd05`.
It supplies only its intrinsic black mana ability. Cruel Edict's unchanged
historical raw record SHA-256 is
`16659c771b7443c23b9193a364891410aa1efd1d0e308f3094499fe60b843fef`,
with AST SHA-256
`6ed597e9deae79bc975ef443eb5264b2e9d6ea18d7f4359c248c305c77fff513`.
Its complete source is the {1}{B} sorcery that makes a targeted opponent
sacrifice one creature of their choice. It becomes supported only through
explicit opponent targeting and resolution-time choice semantics; its earlier
unsupported-fixture status is historical evidence, not permission to approximate
it as a targeted-creature spell.

Hapless Researcher's exact raw record SHA-256 is
`d047cc02d838a248585a596d13b60185911082a2897a1a0211310c2fe57591fd`,
with AST SHA-256
`b2f0f066f520788c04676d044fd7cfb0b7c2ed81d8e43101031aeee9cd569cc0`.
It is a normal {U} 1/1 creature whose complete activated ability sacrifices
itself, draws one card, then discards one card. There is no tap cost, mana
activation cost, target, timing restriction or optional discard. Compiler
admission must preserve that entire source, including the ability on a creature
that also has an ordinary permanent-spell program. No source-specific runtime
dispatch, parser rerun, raw-text execution fallback or ignored clause is allowed.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
The local bytes independently hash to
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules: 109.5, 113.7a/8, 117.3–5, 121, 400.7, 601.2, 602.1–2/5a,
608.2b–d/m/n, 701.9, 701.21, 704.3–4, 733.1–2, 800.4a/g/j, and 903.8–10.

## Exact catalogs, profiles and public protocol

The new exact catalog is schema v5 with eleven definitions: the existing five,
Plains, Time Ebb, Yargle and Multani, Swamp, Cruel Edict and Hapless Researcher.
The twelve archived action records include Blood Artist as a paired whole-card
unsupported trigger fixture. Compilation now lowers Cruel Edict; earlier exact
catalogs still exclude it and preserve their original definition digests and
scenario semantics. Admission requires complete catalog identity plus exact
whole-configuration equality. No player owns more than sixteen objects.

All four new profiles use four seats at 40 life, P0 active in turn-1 precombat
main with priority, unused land play, empty stack/mana/histories, incarnation 0,
untapped objects and no control grants. Initial battlefield creatures meet
continuous-control eligibility. All use explicit mana spending, ordinary combat,
ordered libraries and existing Commander lifecycle rules. Both commander
designations in the mixed profile are independent physical cards with separate
owners, histories and taxes; this is not partner/multiple-commander deck admission.

### `actions()` / `actions_borrowed()` / `actions_empty_library()`

| IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P1 | Isamaru | battlefield; designated commander |
| 1 | P1 | Grizzly Bears | battlefield |
| 2 | P0 | Swamp | battlefield |
| 3 | P0 | Forest | battlefield |
| 4 | P0 | Cruel Edict | hand |
| 5 | P1 | Hapless Researcher | battlefield |
| 6 | P0 | Time Ebb | hand |
| 7–9 | P0 | Island | battlefield |
| 10 | P1 | Plains | battlefield |
| 11 | P1 | Island | battlefield |
| 12 | P2 | Island | battlefield |
| 13 | P3 | Island | battlefield |
| 14 | P1 | Unsummon | hand |
| 15,16 | P0 | Island | library, ordered **15,16** |
| 17,18 | P1 | Plains | library, ordered **17,18** |
| 19,20 | P2 | Island | library, ordered **19,20** |
| 21,22 | P3 | Island | library, ordered **21,22** |

The borrowed profile changes only researcher 5's owner to P2 while P1 remains
its default controller. The empty-library profile instead removes cards 17/18
entirely, leaving P1's library empty. These are distinct exact configurations.
No other ownership, control, library order or card substitutions are admitted.

### `mixed_mana()`

| IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Yargle and Multani | command; designated commander |
| 1–4 | P0 | Swamp | battlefield |
| 5–9 | P0 | Forest | battlefield |
| 10 | P0 | Cruel Edict | hand |
| 11 | P1 | Isamaru | battlefield; designated commander |
| 12 | P1 | Grizzly Bears | battlefield |
| 13 | P1 | Hapless Researcher | battlefield |
| 14 | P1 | Island | battlefield |
| 15 | P1 | Unsummon | hand |
| 16 | P2 | Island | battlefield |
| 17 | P3 | Island | battlefield |
| 18,19 | P0 | Swamp, Forest | library, ordered **18,19** |
| 20,21 | P1 | Island | library, ordered **20,21** |
| 22,23 | P2 | Island | library, ordered **22,23** |
| 24,25 | P3 | Island | library, ordered **24,25** |

The source-backed Yargle cost {3}{B}{B}{G} is now actually payable, with different
legal generic-color spending plans. Bounce to command and a later untapped turn
permit a taxed {5}{B}{B}{G} recast. P0's nine initial lands can pay that exact
cost. No seeded mana or command-cast history creates these witnesses.

New player-facing inputs include `PlayerTargets { players }`,
`ActivateAbility { object, ability }`, `Sacrifice { object }`,
`ResolveDiscard { objects }` and explicit `SpendMana`. Player target IDs are
public original-seat identities, never physical card IDs. Resolution choices
carry the actual chooser and candidate handles; queries cannot mutate domains.
A visible stack entry distinguishes optional spell and activated-ability
representations. Noncard abilities have an opaque stack handle, compiled ability
index, Oracle identity and historical source reference, whose current binding
is absent after sacrifice. The graveyard source card is not the stack object.

Recovery advances to snapshot v11, bundle v12, projection/context v9 and query
sidecar v8. The executable action vocabulary requires IR v5; previous pools
continue to select their own IR v3/v4 definitions. Budgeted queries, observations,
saved work and replay must support every new suspended decision and continuation.

## Explicit mixed-color mana spending

New profiles replace automatic generic-color selection with an explicit
spending decision after the casting mana window. Existing monochromatic
profiles retain their reviewed payment behavior. The decision exposes locked
total cost and the caster's current pool; it does not grant priority or permit
another player to pay. Colored requirements are paid with matching colors;
generic requirements may use the remaining admitted mana colors. Use exact
quantities. The submitted map must have nonnegative amounts, respect pool
bounds, satisfy every colored symbol and pay exactly the total cost. Reject
underpayment, overpayment, unavailable colors and wrong-player/stale decisions
without any semantic mutation. Spending maps contain positive entries only;
zero entries are rejected rather than retained as alternate encodings.

`FinishMana` determines whether complete payment is possible. If not, use the
existing whole-cast reversal and keep-all/undo-all casting-mana behavior. If it
is possible, the spending choice cannot cancel the payable cast. A bad spending
response rejects and leaves the same spending decision pending. A successful
response commits the chosen actual color amounts atomically with completion;
it must not silently substitute another valid spending plan.

Keep total cost locked from the casting announcement, including exact physical
Commander tax. Only successful command-origin casting advances its per-caster
history. Reversal restores the original zone/incarnation and pre-cast mana,
undoing only receipt-recorded casting-time activations when requested. Legal
retained activations still interrupt prior passes. External departures never
reverse or resurrect removed players/sources. Queries and saved validation work
at the spending decision obey existing budget, executor and state-token rules.
No hybrid/Phyrexian costs, reductions, alternate costs, variable costs or mana
abilities with additional costs enter this slice.

## Opponent targeting and sacrifice during resolution

Cruel Edict announces a live opponent as its target, not a creature. Any live
opponent is legal even if they control no creatures. Reject self/departed/unknown
targets and preserve player-target slots separately from incarnation-bound
object targets. Ordinary sorcery timing, whole-spell target legality, mana
payment and stack priority remain in force.

At resolution, if the sole targeted player is now illegal, the spell does not
resolve. Otherwise, the targeted player chooses one currently controlled
creature, if any. Ownership is irrelevant to sacrifice eligibility; movement
goes to the chosen creature's owner's graveyard. Sacrifice is a distinct typed
action under CR 701.21, not destruction, damage, bounce or a cast-time target.
A zero-creature domain completes the instruction without a choice or sacrifice.
Only the affected player answers; the caster cannot choose for them. Preserve
the frozen resolving invocation, cursor, target-legality facts and outcome
through the suspended decision. No player receives priority or an SBA check
between resolution instructions.

If the spell's source owner leaves after resolution has started, continue the
detached invocation under CR 608.2m. If the target leaves while their sacrifice
choice is pending, departure removes or exiles all their controlled creatures.
The original sacrifice domain becomes empty and the impossible personal action
does nothing. CR 800.4g does not transfer the sacrifice requirement to another
player's creatures. General reassignment of still-meaningful departed-player
choices remains outside this exact effect's reachable scope. Revalidate already
accepted choices and discard stale proposals; never sacrifice an object removed
by departure. A sacrificed commander actually enters its owner's graveyard,
then receives its owner's fresh-entry return choice at the next SBA checkpoint,
after the entire Edict invocation has completed.

## A noncard ability with a sacrificed source

Activating Hapless Researcher creates a distinct noncard stack instance with
its own stack ID, activator/controller, compiled ability index/program and
historical source reference. Sacrifice the current controlled battlefield
source as its activation cost; the card itself never moves to the stack. The
source's controller may activate whenever they have ordinary priority, including
the turn the creature entered: no tap/untap cost makes summoning sickness
irrelevant. Reject a noncontroller, stale incarnation, graveyard source,
unsupported ability index, or attempt to activate again after it was sacrificed.
The bounded self-sacrifice cost has no mana window or spending decision.

Complete activation and any required stabilization before returning priority
to the activator. The ability does not resolve immediately; it uses normal
stack ordering and all-pass rules. Once activated, it exists independently of
the source (CR 113.7a). A borrowed source's physical owner leaving must not
remove an ability controlled by a surviving activator. A departing activator's
noncard stack ability ceases to exist before resolution (CR 800.4a), even when
the source card belongs to someone else. If a legally resolving ability leaves
the stack after starting, continue its frozen invocation (CR 608.2m); completion
removes the ability and never moves or resurrects its historical source card.

The resolution's draw and discard apply to the activator under CR 109.5. Draw
one actual top card, then request exactly one discard from their current hand
if nonempty. A failed empty-library draw still proceeds to discard an existing
hand card, and only then does the player lose at the next SBA check. No failure
or budget cutoff may skip the later discard. A commander drawn from a library
first requires its owner replacement choice under CR 903.9b; after acceptance
or decline, resume the same ability and still perform the mandatory discard.
If a commander is discarded to graveyard, offer its fresh-entry SBA return only
after the whole ability finishes. Preserve hidden-library privacy and the
optional visible handle/public designation request contract.

If the activator leaves after resolution has started, their library and hand
leave with them. Continue the invocation while skipping impossible personal
draw/discard actions; do not apply “you” to another player's hand or library.
No general draw effect, arbitrary discard count, random discard, ability target,
tap cost, paid mana activation cost or Commander-eligible Hapless designation
is admitted. A hypothetical sacrificed commander as an activation cost would
receive its SBA choice before priority, unlike a sacrifice during resolution;
Hapless itself is not an eligible commander in this pool.

## Required evidence

Freeze exact profile tables, the new executable vocabulary/catalog and version
changes before acceptance. Verify source-to-runtime mixed-color Yargle casting
and taxed recasting, two valid spending plans with distinct retained colors,
failed-payment keep/undo, invalid maps, stale and foreign query work/receipts,
and spending choices across pending concessions.

Verify Edict opponent announcement, zero-creature opponents, noncaster choices,
controlled-versus-owned sacrifice, commander grave return ordering, illegal
targets before resolution, target/source departure during a paused choice, and
accepted-choice revalidation. Verify noncard ability stack order, sacrificed
source independence, newly cast creature activation, borrowed source-owner
versus activator departure, ordinary and failed draws followed by mandatory
discard, nested draw replacement and commander discard/SBA timing.

Compare uninterrupted, one-unit budgeted, cloned, restored and journal-replayed
paths at every new choice and decisive event boundary. Reject rehashed forgeries
of spending maps, target bindings, activation source/cost, noncard stack identity,
resolving cursors, hidden-draw resumes and discard domains. Querying cannot mutate
semantic state or mint hidden handles. Off/compact/detailed traces must agree
semantically. Complete formatting, Clippy, debug/release workspace tests,
cross-language fixture contracts, source-to-runtime demo and a fresh retained
offline rebuild/replay. Keep general matches, arbitrary pools and unsupported
rules mechanics explicitly outside admission.
