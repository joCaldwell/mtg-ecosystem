# Ordinary triggers, APNAP placement and source history

> Historical capacity proofs: D1b-2 supersedes this slice's per-profile bounds
> with the [generic capacity policy](game-engine-capacity-policy.md). The original
> arithmetic and evidence below describe the historical release only.

Contract frozen October 15, 2026 for five exact four-seat scenarios. This P4
slice adds three mandatory ordinary creature triggers through the existing typed
event path. It closes their collection, SBA scheduling, ordering, targeting,
stack interaction and departure behavior. It does not complete the broader P4
replacement/prevention gate or admit arbitrary decks or legal Commander matches.

## Frozen source and executable meaning

Use complete records from the retained schema-3 historical archive. No parser
changes, fresh card fetches, name dispatch or raw-text runtime interpretation are
part of this slice.

| Card | Source index | Oracle identity | Source SHA-256 |
| --- | --- | --- | --- |
| Blood Artist | 7315 | `310f141c-7f37-4729-aed6-dd9c09db448d` | `888f992471e8dbd626d5a2935bef8df11507ad382b077a4674deac2610a9e8ac` |
| Elvish Visionary | 29855 | `c6a3a882-a127-4590-93d7-679ef4313efe` | `26f75484b163d8f6a482e1d15719623f7e87729fb5dbf0f57f89d67373e8c74f` |
| Essence Warden | 16311 | `6ca2a89e-7032-4864-b4e9-66f3178f90ab` | `6c4225ff3a8e52c25e1e1f79eaf5a355f6b1d6ff24884503e010a10a7a91eef8` |
| Deathgreeter, unsupported | 27581 | `b7ad11dc-c962-4034-b15a-a080c6fff75f` | `16d63c5fc326de874035c39bd32960030d022636482d816e51778430a60490de` |

Their exact raw-record SHA-256 values, in table order, are
`16472af1ca8bba34a89df28bbbcdd313b0aa2e4aa5a1594d32f1666e1435b358`,
`55c4ba511575399ea23bcda388c460fda2f578e146818a240fb98d27aa6d0471`,
`eebedc961887a7f7b6cc9a998485b4615dfc485401843a5aede8da053e61e220`,
and `903ef9fc0727b4c98ce9e7d6b36408291cf6433ad437651dc713758eadcca01d`.

Blood Artist is a {1}{B} 0/1. Its single trigger matches its own death or another
creature's death, then makes one target player lose 1 life and its trigger
controller gain 1. These two trigger alternatives form a disjoint partition of
creatures relative to the source incarnation; the source's own death produces
one trigger, never two. Elvish Visionary is a {1}{G} 1/1 whose own entry draws
its trigger controller one card. Essence Warden is a {G} 1/1 whose trigger gains
its controller 1 life for another creature's entry. Its own entry is excluded.
Deathgreeter is a {B} 1/1 with an optional life-gain effect; preserve that clause
and reject the complete card because optional trigger effects are not admitted.
Flame Rift remains the aggregate-scope negative from the preceding slice.

The new catalog is exactly the prior 21 damage/life definitions plus Blood
Artist, Visionary and Warden: 24 definitions in IR v8. Its 26 distinct source
records reuse the unchanged Blood Artist record and add Visionary, Warden and
Deathgreeter. Historical catalogs retain their exact earlier definitions and
records. Blood Artist can now lower successfully while remaining unadmitted in
those historical pools; do not fabricate an unsupported compiler result to
preserve an old admission exclusion.

Historical `binding.fullyBound` is diagnostic archive metadata. Artist's
alternative events and Artist/Warden's `other` filter were historically marked
unbound. The Rust semantic compiler independently binds the exact complete AST:
`this` means the source incarnation, `other` excludes that incarnation, and the
Artist union exhausts creature deaths without overlap. Retain original archive
bytes and diagnostics. Unrelated unresolved references and broader union/filter
forms still reject. Visionary's archived self reference is already bound.

`Definition.triggered` carries mandatory battlefield abilities with the finite
conditions `SelfEntersBattlefield`, `OtherCreatureEntersBattlefield` and
`AnyCreatureDies`, and complete ordered executable programs. Targets and the
meaning of `you` belong to the triggered invocation. Existing `DrawCards`,
`LoseLife`, `GainLife` and `AnyPlayer` instructions/domains are reused. Unknown
conditions, optional effects, state triggers, triggered mana abilities, delayed
or reflexive abilities, intervening-if clauses and trigger-on-trigger conditions
remain explicit unsupported/invalid cases. This is an ordinary-trigger scheduler,
not a universal queue for those different rule families.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).
The independently verified pinned TXT SHA-256 is
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules include 101.4, 109.5, 113.7a/8, 117.2a/5, 400.7, 405.4,
603.2c/g, 603.3a/b/d, 603.6a/c, 603.10a, 608.2b/c/h/m, 700.4,
704.3/4, 800.4a/d/j and 903.9a.

## Exact configurations and capacity

All five profiles begin with P0 active at turn-1 precombat-main priority, original
four-seat order, empty stack, pending-trigger set, histories and mana pools,
incarnation 0, no control grants, untapped objects and initial-creature continuous
control eligibility. Initial setup is not a creature-entry event. Explicit mana
spending, ordinary combat, ordered libraries and the existing Commander lifecycle
remain enabled. Base life is 40; the low-life variant starts every player at 2.

### `triggers()`

| IDs | Owner/controller | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Isamaru | battlefield; designated Commander |
| 1,2 | P0 | Blood Artist | battlefield |
| 3 | P0 | Essence Warden | battlefield |
| 4 | P0 | Elvish Visionary | hand |
| 5 | P0 | Hapless Researcher | battlefield |
| 6 | P0 | Prodigal Sorcerer | battlefield |
| 7 | P0 | Swamp | battlefield |
| 8,9 | P0 | Forest | battlefield |
| 10 | P0 | Plains | battlefield |
| 11,12 | P0 | Island | battlefield |
| 13 | P0 | Mountain | battlefield |
| 14,15 | P0 | Forest, Island | ordered library **14,15** |
| 16 | P1 | Blood Artist | battlefield |
| 17 | P1 | Essence Warden | battlefield |
| 18 | P1 | Grizzly Bears | battlefield |
| 19 | P1 | Lightning Bolt | hand |
| 20 | P1 | Unsummon | hand |
| 21 | P1 | Counterspell | hand |
| 22 | P1 | Stifle | hand |
| 23 | P1 | Mountain | battlefield |
| 24,25 | P1 | Island | battlefield |
| 26,27 | P1 | Island | ordered library **26,27** |
| 28 | P2 | Blood Artist | battlefield |
| 29 | P2 | Essence Warden | battlefield |
| 30 | P2 | Grizzly Bears | battlefield |
| 31 | P2 | Forest | battlefield |
| 32,33 | P2 | Forest | ordered library **32,33** |
| 34 | P3 | Blood Artist | battlefield |
| 35 | P3 | Forest | battlefield |
| 36,37 | P3 | Forest | ordered library **36,37** |

`triggers_low()` changes only initial life to 2. `triggers_borrowed()` instead
changes Artist 1 to owner P2 while P0 remains its default controller.
`triggers_empty_library()` instead removes P0's library cards 14/15 entirely.
`triggers_nonactive()` instead changes Hapless 5 to owner P2 and default
controller P1. All other facts remain exactly the base setup. No seat owns more
than sixteen cards.

The trigger profiles use a 128-entry reviewed capacity; the four-entry bound of
earlier profiles is unchanged. There are fourteen physical creatures, five
Artists and three Wardens. During one nonempty-stack episode each creature can
die at most once: creature casting requires an empty stack, libraries contain
lands only, and no tokens, copies, recursion or battlefield-return effects are
present. Thus all deaths in that episode generate at most 14 × 5 = 70 Artist
entries. At most one permanent entry starts an episode and creates at most four
ETB entries (Visionary plus three Wardens). At most seven ordinary proposals or
activated entries are additionally available: four instant cards, one permanent
proposal, one Sorcerer tap activation and one Researcher sacrifice activation.
The conservative combined pending/placed/ordinary bound is 81, below 128.
Control/ownership variants add no objects or activations; removal/life-loss
variants cannot increase this bound. Overflow is a resource fault, never a game
outcome or silent dropped trigger. This proof is for these exact tables only.

## Actual-event collection and stabilization

Collect triggers from actual committed outcomes, preserving the triggering
batch and occurrence identity. A proposed or replaced movement that did not
happen produces no trigger. Creature death is actual battlefield-to-graveyard
movement; sacrifice and lethal-damage SBA movement use the same semantics.
Leaving the game, exile and battlefield-to-hand/library replacement do not count
as death. A Commander that entered its graveyard died even if its owner later
returns it to command at the SBA checkpoint.

For death batches, match all sources and affected creatures against one common
pre-batch view. Each Artist present before simultaneous deaths sees every
matching creature, including itself; do not remove one observer before examining
a later occurrence. Retain its source incarnation, frozen trigger controller,
ability program and public source history. For ETB, match the post-event
battlefield including newcomers: Visionary sees itself; a Warden sees other
creatures, including other simultaneous newcomers, but not its own incarnation.

Matching creates distinct pending trigger identities without resolving effects
or granting priority. Abilities may trigger during costs or resolution. Exhaust
all applicable SBA waves, including Commander grave/exile owner-return choices,
before beginning placement. A declined return consumes only that incarnation's
return opportunity; it does not remove an already-triggered Artist ability.
No mid-program SBA or trigger-placement window interrupts loss/gain, draw/discard,
spell completion or activation costs. In particular Researcher's sacrifice cost
creates pending Artist triggers; the paid activated ability is below them when
they are placed before activation's priority recipient receives priority.

After ordinary trigger placement, repeat SBA/trigger checks until stable, then
restore the player who was due priority. This may be a nonactive activator.
Cleanup retains its repeat/stabilization continuation when applicable. Finite
programs in this pool do not create creatures during trigger resolution; later
extensions must prove their own chains and bounds rather than assuming this
admission covers them.

## APNAP order, identities and targets

Group pending ordinary triggers by their controller at trigger time, independent
of source ownership or its later controller/zone. Place controllers in APNAP
order anchored to the original turn owner and seating. If that player has left,
continue from the next surviving seat; do not restart or recycle seats.
Within a controller's group, `OrderTriggers { order }` specifies a complete
permutation from bottom to top. Later controllers' groups sit above earlier
ones. Distinct triggers from the same source or event cannot collapse into one.

Player responses use viewer-scoped `TriggerHandle`s; trusted host responses use
`TriggerId`s. Ordering candidates expose `VisiblePendingTrigger` with handle,
historical/current source reference, oracle identity and ability index. These
are pending identities, distinct from physical object and stack identities.
Queries cannot mint identities or handles. Reject missing, duplicate, stale,
foreign-viewer and other-controller entries without mutation. Source departure
may remove the current source reference while the historical reference persists.

Target choices occur separately for each triggered invocation during placement,
using the existing any-player `PlayerTargets` API and a dedicated
`TriggerTargets` continuation. The source card never moves to the stack. There
is no cast, activation, mana window, payment or intervening ordinary priority.
Completed entries receive their own stack identity and project through a
separate optional `VisibleStackEntry.triggered`, preserving the existing spell
and activated projections and historical handle contract.

The Artist target may be its controller. Empty or illegal responses reject while
legal choices exist. If an accepted but unapplied target response becomes invalid
because its player leaves, clear that response and reissue the mandatory choice;
do not reverse a cast or drop a trigger that still has legal choices. Once target
binding completes, later target departure preserves the binding: the invocation
fails its whole-program target recheck and performs neither life loss nor gain.
An actually empty mandatory target domain removes that ability under CR 603.3d;
with AnyPlayer and a living controller this is not naturally reachable in these
profiles and must not be simulated by accepting an empty response.

Stifle can target a completed triggered stack entry; Counterspell cannot.
Pending/uncompleted entries are not ordinary stack targets. Countering removes
only the noncard invocation, performs none of its instructions and never refunds
the event or cost that caused it. Lower entries and other pending triggers retain
their identities. Source death, bounce, owner departure or reincarnation does not
remove an ability controlled by a surviving player.

## Departure, resolution and recovery

A trigger's controller remains the player who controlled the source when it
triggered; `you` never follows its owner or a later incarnation. Pending triggers
controlled by a departed player are not placed (800.4d). Their already-placed
noncard entries cease to exist on departure (800.4a). A borrowed source owner's
departure preserves triggers controlled by a survivor. Started resolution retains
its frozen invocation under 608.2m; skip personal life/draw actions for departed
players without redirecting them. Resolve ordered Artist loss/gain completely
before SBAs, including self-target loss through zero followed by gain.

Visionary draws through the existing ordered-library/Commander-replacement
continuation. An empty library causes failed-draw loss after complete resolution,
then removes that player's pending and placed noncard objects. Simultaneous
losses/departures must filter every controller group and preserve original order,
source history and unaffected lower stack objects. No departure is a creature
death merely because its owner controlled creatures.

Snapshot v14, bundle v15, projection/context v12 and query sidecar v11 retain
pending IDs, source facts, event provenance, placement order/cursors, accepted
inputs, frozen controller/program, historical handles and the original priority
continuation. Standalone restore and independent replay reject rehashed forgeries
of these fields. Trace diagnostics do not enter semantic state or player views.
All historical catalog families and gameplay hashes remain separate references.

## Exit evidence

Require complete-source lowering and strict vocabulary/capability closure, with
optional Deathgreeter, aggregate Flame Rift and unsupported trigger families
rejected. Preserve archive diagnostics honestly while proving Rust's exact new
reference bindings, and reject nearby union/filter mutations that change meaning.

Natural combat proof: P0's Artists 1/2 attack P1/P2, whose Bears block. Both
Artists die in one SBA wave; all five pre-event Artists see both deaths, yielding
ten pending instances, distributed P0 four and P1/P2/P3 two each. Prove APNAP,
each controller's chosen bottom-to-top order, dying-source history and ten actual
life programs rather than merely counting queue entries. Also verify Visionary
self-entry plus all Warden observers, Warden self exclusion, source sacrifice and
nonactive activation, Commander death/return ordering, Stifle/counter-domain
separation, illegal-target whole-program failure, source/controller departure,
empty-library loss and low-life elimination cascades without false death triggers.

Independent adversarial tests cover order permutations and malformed/foreign
handles, query purity and resumable work, accepted-input departure boundaries,
source reincarnation and look-back, pending versus placed identity, target
reissue versus completed-binding fizzle, preserved nonactive priority, and forged
snapshot cursors/controller/source/event data. Compare uninterrupted, one-unit
budgeted, cloned, restored and replayed execution at every new request and decisive
boundary. Check off/compact/detailed semantic equivalence, then formatting,
Clippy, debug/release workspace tests, cross-language fixtures, source-to-runtime
witnesses and fresh retained offline rebuild/replay before acceptance.
