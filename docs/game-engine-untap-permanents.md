# Ordinary permanent untapping and creature tapping

## Creature tap and controller draw

Schema 1 also supports `TapPermanent { target_slot, source_block }` with
`TapPermanentV1`: one unqualified battlefield-creature target in an ordinary
instant/sorcery spell, followed by at most one mandatory controller draw-one.
Other instructions, target filters, additional costs, modes and activated/triggered
contexts reject in this ordinary-spell form. The complete Pressure Point source is pinned at
`tests/fixtures/d4bbdd0c-e578-49ed-b2f7-b1decb04ee87.json`, archive physical line
32118/source index 32116: `{1}{W}`, Instant, “Tap target creature. Draw a card.”

A legal already-tapped target produces no second tap event and still permits
the draw. If its sole target is illegal as resolution starts, the entire spell
fails to resolve, including the draw. Later departure cannot rebind a target or
repeat that legality check. Execution reuses the strict tap event and existing
draw, payment, visibility and replay machinery (pinned CR110.5,608.2b/c,701.26a).
This source/runtime increment adds no pool, hosting or search admission, and
does not qualify downstream consumers for the changed schema/build.

## Activated artifact tap

The same tap instruction also supports a separately fenced artifact activation
with `ActivatedTapPermanentV1` alongside `TapPermanentV1`. Its existing composite
cost must include finite mana and tapping the source, without sacrifice. The
battlefield ability has exactly one tap instruction and one battlefield-permanent
target matching the artifact/creature/land union. Other filters, target domains,
effects, modes and triggered contexts reject; ordinary tap spells keep their
creature-only scope. No IR version, instruction field or runtime event is added.

The complete Icy Manipulator record is
`tests/fixtures/3608f1f7-8dc5-4dd1-ae91-c830e1de9529.json`, physical archive line
8054/source index 8052: artifact `{4}`, with `{1}, {T}: Tap target artifact,
creature, or land.` Costs, type union and provenance come from that exact source.
The source may target itself: payment taps it, so resolution produces no second
tap. A paid ability remains on the stack after its source leaves; a departing
sole target instead makes the ability fail to resolve (pinned CR113.7a,
205.2b,602.1a/2b,608.2b,701.26a in the retained 2026-09-25 rules).

Focused private source configurations verify real mana/tap payments, the target
union and atomic invalid-target rejection, source/target bounce responses,
snapshot restore, accepted-input replay and budget/trace parity. These are not
100-card pod witnesses. No pool, hosting/search or consumer qualification follows
from this source/runtime slice; both tap capabilities remain outside the reviewed
hosting/search sets.

## Permanent untap

Card IR remains at schema **1**. `UntapPermanent { target_slot, source_block }`
requires `UntapPermanentV1`, alongside the existing permanent-filter capability.
Its ordinary spell program is one instant/sorcery spell, one instruction and
one unqualified battlefield-permanent target. Modes, triggered programs, additional
costs, qualified ordinary targets and extra instructions reject as complete
programs. Activations require the separate tap-source creature contract below.
Capability derivation and rejection inspect modal descendants too;
labels cannot authorize another execution context.

The compiler consumes the complete retained Burst of Energy record at
`tests/fixtures/b795ecfd-31ac-4a2a-9679-97b088932d93.json`: `{W}`, Instant,
white identity, retained Commander legality `legal`, and “Untap target permanent.”
Its single AST instruction binds one target with `filter: { cls: "permanent" }`.
The record was extracted from the accepted `d705222b…` archive at physical line
27637/source index 27635 and verified against the full archive hash. Exact raw,
source, AST and binding pins remain in fixture provenance. It is a compiler and
runtime witness. Subsequent [headless admission](game-engine-blocking-matches.md)
adds it to the existing development pool without expanding hosting/search.

## Resolution

The rules basis is the pinned [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt):
110.5 defines status, 601.2c and 608.2b govern target legality, 701.26b permits
untapping only tapped permanents, and 506.4b keeps a tapped or untapped attacker
in combat.

An already-untapped permanent is a legal target. Its spell resolves successfully
without producing an `Untapped` event. A currently tapped target uses the existing
strict, atomic untap event; that event's guard is unchanged. Execution respects
resolution-start legality and the exact current battlefield incarnation. It
cannot affect a successor incarnation after departure and reentry.

This reuses existing target handles, query validation, payment, event outcomes and
snapshot/replay contracts. No public or trusted-state format changes are added.
Hosting and search do not admit the new capability. The later headless increment
separately qualifies a narrow defensive Develop policy; no client qualification
follows from this compiler/runtime increment.

## Source-backed witnesses

A genuine Burst response untaps an attacker after a paid Swift Response targets
it. Untapping preserves its attacking membership/readiness, Swift Response fails
its tapped-target recheck, and the attacker still deals combat damage. Both
spells use ordinary target and mana decisions. A second witness pays for Burst on
an already-untapped Plains and checks successful resolution with no false untap
event. Other witnesses cover owner departure, bounce/reentry, equipment protection
and atomic rejection of unauthorized target responses.

These are explicit private source-backed configurations using real checked
inputs, not admitted 100-card pods or injected untap events. Snapshots, forks,
accepted-input replay, trace retention and execution budgets are checked where
claimed by the focused tests. The earlier synthetic status change during payment
remains separately qualified; an ordinary spell response occurs after payment.

The initial implementation at `566b5f04` passed 31 contracts, 142 compiler, five untap, five tapped-target,
34 adjacent removal and one historical lifetime-proof test: 218 Rust tests.
Ajv accepted the generated Burst catalog and one synthetic valid IR shape, and
rejected 13 scope violations. Workspace all-target compilation, formatting,
structure checks, production compiler/core/simulator Clippy and a debug simulator
build passed. Contracts Clippy was omitted because its unchanged grouped-damage
`collapsible_if` finding is already retained; it is not reported as passing.

The copied simulator's core build is
`7570504418e82f99b578178a53f330ed9f25af08cf32ff4e13df5267a0102af7`.
Its compiled source closure, small smoke and explicit replay passed. Source
fingerprints, command receipts, schema probes, failed attempts and the exact
executable remain under `generated/permanent-untap/`. Full Node/runtime/tooling
suites, completion/release/profile matrices, offline retention and client/server
requalification were omitted. Earlier consumer qualification keeps its exact
producer identity.

## Tap-source creature untap

Schema 1 additionally admits `TapSourceUntapCreatureV1` for one separately fenced
creature activation: battlefield source, pure `TapSource` cost, instant-timed stack
instructions, one `UntapPermanent` at slot zero and one mandatory
`BattlefieldPermanent` target with exactly the creature type filter. All other
filter fields are empty/false. Target binding remains controller-chosen at
announcement. Additional sacrifice, modes, triggers, extra instructions and other
activation costs reject. Ordinary Burst keeps its unqualified permanent target;
the shared instruction does not widen ordinary spell semantics.

The complete Seeker of Skybreak fixture is
`tests/fixtures/704723f3-0490-4d96-930a-aacc6d19ba8d.json`, extracted at physical
line 16867/source index 16865 from the fully hashed accepted `d705222b…` archive.
Its retained source is `{1}{G}`, Creature — Elf, 2/1, green identity, with exactly
“{T}: Untap target creature.” Raw record/source/AST/binding hashes are pinned by
the fixture and provenance; compilation retains the complete source. The retained
2026-09-25 CR302.6,602,608.2b,113.7a and701.26b govern continuous-control readiness,
announcement/payment, exact target legality, independent paid abilities and
untapping. Existing generic target routing, tap payment and untap execution need
no production core changes.

Four private source-backed runtime tests use accepted inputs, budgets 1/4096,
detailed/off traces, trusted snapshot restoration and accepted-input replay. The
principal case casts Seeker for real `{1}{G}` from two Forests, rejects a fresh
activation despite an available creature target and independent untapped mana,
ages it through a natural turn, and obtains two real green mana by tapping
Llanowar Elves, resolving Seeker's paid untap, then tapping that Elf again. Other
cases prove self-targeting while initially untapped, two explicitly requested
self invocations without automatic looping, source bounce after payment, and
target departure followed by genuine Flash Research Thief reentry without
rebinding. Research Thief uses its own `cards-data-d2ce` archive header. Initial
battlefield zones/readiness in the response cases are declared private setup;
no gameplay outcome is injected. Invalid target responses remain atomic.

This is a source/runtime slice; pod, hosting, search and downstream consumer
admission remain separate. The producer
`31e667f275d2a49f6ee0503ec48503ae0fb5d47449b69720a59ab6fcc72f927a`
qualifies the isolated source based on `fafbd9f5`, with 249 focused Rust tests,
four schema positives/38 negatives, smoke and explicit replay. Formatting,
workspace checking, structure and compiler/core production Clippy passed.
Contracts Clippy retained the unchanged grouped-damage `collapsible_if` failure.
Raw source identities, receipts, omissions and copied artifacts remain under
`generated/tap-source-untap/`; this evidence does not qualify other integrated
changes or renew full acceptance.
