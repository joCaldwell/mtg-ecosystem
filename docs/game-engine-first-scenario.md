# First compiler and engine implementation brief

The follow-up [Commander/departure contract](game-engine-departure-slice.md)
adds Isamaru to the exact reviewed catalog and advances IR to version 2 and
snapshots to version 3. The subsequent [event slice](game-engine-event-slice.md)
uses snapshot/bundle version 4 with separate diagnostic evidence. The original
gameplay trace below remains unchanged;
its earlier version/hash evidence is historical.

Status: **P0 inputs frozen, 2026-10-13; initial P1 scenario implemented**.
This work delivers a native, four-seat source-to-runtime scenario. Complete
Commander matches, broader card support, and search remain later gates in the
[implementation plan](game-engine-plan.md).
The subsequent [P2 slice](game-engine-p2-slice.md) adds budgets, private
observations, query recovery, and offline retained rebuilding on this same pool.

## Frozen inputs and product choices

Use the audited October 10 schema-3 archive, independently of ongoing parser
edits. The small exact records and their digests are retained in
`packages/game-engine/tests/fixtures/provenance.json`. The archive manifest is
also retained. Corpus SHA-256 is
`5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60`;
parser source SHA-256 is
`98980a967a70d57008847ce0952edf0cc277af9ad26b8cb8739a555d97082b8d`.
These are historical input identities, not claims about current parser checks.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
with local bytes SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules are 106.3–4, 113.6, 117.3–4, 305.6, 400.7, 601.2,
605.3, 608.2, 704.3, and 733.1. No live rules or card refresh occurs during a run.

The first consumer is the native scenario CLI and test harness. Later search
uses open decklists with private hands, library order, and choices, sampling at
ordinary priority boundaries. That policy does not grant access to authoritative
hidden state. One eligible commander per player is the eventual first match
configuration; this scenario has no designated commanders. Construction,
color-identity, and pinned ban-policy validation must precede P5 match admission.

The reference machine is an x86_64 AMD Ryzen 9 9950X, 16 physical cores and
32 threads, running Linux. Pin Rust 1.99.0 and retain Cargo.lock; Node is
26.11.1 for parser contract checks. Initial action/query/clone timings are
observations only. The plan's provisional 10,000 ordinary transitions/s/core,
100 microsecond p95 fork, and 2 GiB for 10,000 retained branches remain
calibration hypotheses for representative P2/P5/P7 workloads, not acceptance
criteria for this tiny scenario.

## First scenario and expected behavior

Seats P0, P1, P2, P3 start with 40 life, P0 active and holding priority in a
main-phase scenario. Card 0 is P0's untapped Island on the battlefield. Card 1
is P0's Unsummon in hand. Card 2 is Grizzly Bears owned by P1 and controlled
by P2 on the battlefield. All have initial incarnation 0. No other objects,
abilities, replacement effects, pending triggers, or commanders are present.

1. P0 activates Island's intrinsic mana ability. Commit tap and blue-mana
   events; the ability does not use the stack, and P0 retains priority.
2. P0 proposes casting card 1. It moves to the stack, becoming a new
   incarnation, and execution pauses for its creature target. Save here.
3. P0 chooses card 2's current incarnation. Pay one blue mana, finish casting,
   and return priority to P0. The frame retains its target binding and payment.
4. P0, P1, P2, P3 pass consecutively. Recheck the target once for the complete
   spell; resolve its ordered program through the event pipeline. Card 2
   moves to **P1's hand**, becoming a new incarnation. Card 1 moves to P0's
   graveyard, also becoming a new incarnation. P0 receives priority again.

Expected event order: tap; mana added; spell proposed; hand-to-stack move;
mana paid; spell cast; four priority passes; battlefield-to-owner-hand move;
stack-to-graveyard move; spell completed. Queries and snapshots add no events.
The uninterrupted, restored, replayed, and cloned executions must agree on
semantic state and events. No random operation is supported or consumes RNG.

Forest exercises a second intrinsic subtype. Grizzly Bears supplies validated
base creature characteristics; casting permanent spells and combat are outside
this scenario. Cruel Edict is a source-backed unsupported fixture: its opponent
target is announced while the creature choice belongs to that opponent during
resolution. It must not be lowered as an ordinary creature-target spell.

## Compiler and runtime contracts

Rust IR types are the single executable schema authority; export JSON Schema
with [Schemars](https://docs.rs/schemars/latest/schemars/). Source AST types remain
the TypeScript parser's authority. The archive adapter is pinned to schema 3
and the frozen parser identity. It verifies source, binding, and successful-AST
hashes using exact serialized subvalues, preserving JavaScript archive ordering
with [RawValue](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html).
Complete Scryfall source objects remain provenance; consumed characteristics
and all source AST semantics are checked. The diagnostic TypeScript binding
graph does not certify or supply executable semantics.

The first lowering vocabulary is empty normal basic lands, empty normal
creatures with constant positive toughness, and normal instant spells whose
complete program consists of single-creature-target returns to the owner's
hand. Each target occurrence has an explicit slot, announcement timing, and
incarnation binding. Spell blocks assemble into one resolution program.
Unsupported words, clauses, layouts, faces, and keyword families reject the
whole card. No card-name dispatch or raw-text fallback is permitted.
The initial source adapter consumes ASCII rule text and balanced reminder
parentheticals; richer normalization is explicitly unsupported. Readback/schema
validation establishes structural interchange, while semantic validation and
exact definition-digest admission establish execution support.

Game quantities use [num-bigint](https://docs.rs/num-bigint/latest/num_bigint/)
signed exact integers with canonical decimal-string serialization. Structural
IDs and revisions are checked bounded integers. Overflow is an engine resource
error, never wraparound. Intrinsic land behavior is derived by the runtime
characteristic query and restricted to the battlefield; spell programs function
on the stack. Source/controller, owner, target slots, zone incarnations,
actual event outcomes, and resumable frame stages are separate typed values.

Use a Cargo workspace with contracts, compiler, core, and scenario CLI crates.
Compiler and core depend on contracts; core never imports compiler/parser.
The CLI orchestrates offline compilation and calls core. Core has no network,
filesystem, clock, or OS-randomness access. Clone branching is the first baseline.
Serializable continuations, proposed/committed events, a characteristic query,
and stabilization are production paths from the first slice. For this closed
pool, positive-toughness creatures and unchanged life make further state-based
actions unreachable; triggers and replacements are absent by admission.

Lowering produces `lowered`, `unsupported`, or `invalid` reports. Admission is
separate and checks the scenario mode, four seats, build identity, exact
definition digests, and initial configuration. Only reviewed scenario
configurations are admitted; legal-match mode and arbitrary definitions reject.
The engine must fault on unsupported reachable behavior rather than skip it.

## Exit checks and deferred gates

P1 checks include exact source/AST/IR fixtures, deterministic readback and schema
export, unknown fields/variants/versions, duplicate keys, corrupt hashes,
incomplete faces, unresolved target occurrences, unsupported keywords/layouts,
source-characteristic failures, wrong-functioning-zone activation, invalid and
unauthorized choices, stale incarnations, spell-wide target legality across a
synthetic multi-block program, and insufficient payment. Supported cast reversal
returns the spell to its original hand without spending mana; the accepted
input journal retains the attempted stages (CR 733.1).

Snapshot restore checks build/catalog/profile identities and frame invariants.
Replay bundles retain exact source records, IR/catalog, engine source identity,
toolchain/lockfile, initial scenario, accepted inputs, and expected final state.
First-slice acceptance needs the runnable demo plus debug/release Rust tests, formatting,
Clippy, and TypeScript/Rust archive-hash contract checks. Record actual results
in a gate summary; do not infer full P1 completion from a successful demo alone.
The retained `expected-scenario.json` fixture checks the full event sequence,
accepted decisions, and final semantic hash. The first profile permits one
spell on the stack and mana floated before proposal. Empty-stack step advance,
mana activation during announcement, permanent casting, and stacked responses
fault or reject explicitly; their rules remain P3 work. Recovery uses replay
from this small scenario's initial state to reject arbitrary rehashed snapshots.

P2 hardens execution budgets, complete observations and visibility handles,
receipt isolation, general snapshot validation, artifact-store recovery, and
SearchContext. P3 expands casting/payment and resolution choices. P4 delivers
Commander lifecycle and event interactions. P5 validates complete legal decks
and pods. PRNG selection is deferred until the first random operation; public
protocol, server framework, UI, and search optimization remain later decisions.
No product decision blocks this first work package.
