# Layer 1 game-engine specification

Status: **proposed full design; initial compiler/runtime scenario implemented**.
Only the subset in the [first scenario brief](game-engine-first-scenario.md)
and engine package is implemented. The interfaces below remain design vocabulary
where the package does not yet expose them.
Prepared 2026-10-11. Primary workload: high-volume headless simulation and agent
search **for Commander**. Language stack: [adopted Rust and TypeScript assignments](runtime-language-decision.md).
Delivery gates and benchmark procedure: [implementation plan](game-engine-plan.md).

## 1. Purpose and scope

Build an authoritative, deterministic interpreter of validated card mechanics
and the supported Magic rules. It must support fast branching for search,
explicit player decisions, private observations, reproducible execution, and
explanations derived from actual execution.

The engine evaluates legality and outcomes. Strategies decide which legal
choice to make. The compiler connects parser descriptions to executable
mechanics. The server owns sessions, authentication, persistence, and clocks.
No LLM participates in authoritative rule evaluation.

Initial rules baseline: **Comprehensive Rules effective 2026-09-25**, the edition
used by current parser semantics. References below point to the official
[rules text](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
Pin the actual bytes/hash in fixtures and artifacts; a later edition requires
an explicit compatibility review. Check card facts against frozen Scryfall
source objects, never model memory.

The official text downloaded for this review matches the local rules snapshot:
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Verify this digest when establishing implementation fixtures; an edition label
or URL alone is not an artifact identity.

**Commander is the only product format in scope.** The first playable profile
is four-player free-for-all Commander, with attack-multiple-players and no
limited range of influence, using a curated supported card pool. Four players
is the initial workload choice, not an assertion that Commander requires
exactly four players. Remaining-player counts change as players leave.

Commander setup, deck construction, command-zone rules, tax, commander damage,
multiplayer priority/APNAP, and player elimination are foundational work. They
are not a later format plugin. Generic Magic rules are implemented as needed
to execute Commander. There is no milestone for Standard, Limited, Brawl,
Duel Commander, teams, or another format.

The first pool uses one supported eligible commander per player. Multiple-
commander permissions and unusual commander layouts are later Commander
capabilities, explicitly rejected until implemented. Test scenarios may bypass
setup or use small decks, and must be labeled scenarios rather than legal
Commander matches. This scope does not claim every Commander card/interaction.

## 2. Boundaries and dependency direction

```mermaid
flowchart LR
    Source["Frozen card sources"] --> Parser["Existing TS parser"]
    Parser --> Archive["Versioned AST + source artifact"]
    Archive --> Compiler["Binding + semantic compiler"]
    Compiler --> IR["Validated Card IR + capability manifest"]
    IR --> Core["Deterministic engine"]
    Core --> Runner["Native simulation and search harness"]
    Core --> Server["Game server"]
    Server --> Clients["Agents and browser clients"]
```

The core has no parser imports, network calls, card-data downloads, wall-clock
reads, database calls, or OS-randomness calls. Offline compilation is not in the
simulation hot loop. Card definitions are immutable and shared across games.

The server cannot bypass legality or implement a second version of a mechanic.
The Rust CLI, tests, native agents, Rust server, and browser WebAssembly build
use the same Rust core. The browser application and client SDK use TypeScript.
The existing deck-builder remains independent.

## 3. Card compiler and executable contract

### 3.1 Compilation stages

1. Validate the imported AST/source artifact version, hashes, identity, faces,
   and required source metadata. Preserve failed/excluded records as reports;
   do not silently drop them from coverage denominators.
2. Validate structured characteristics and account for every ability block and
   face. Identify behavior supplied by card types, subtypes, layout, and the
   pinned rules even when it has no parsed Oracle ability. Complete parsing is
   necessary, but parser acceptance alone does not admit a card.
3. Bind source/controller roles, target groups, choices, event results,
   quantities, linked abilities, and nested captures. Unresolved or ambiguous
   dependencies prevent executable emission.
4. Typecheck expressions, cardinalities, functioning zones, timing, and lifetimes.
   Distinguish announcement-time, payment-time, resolution-time, continuous,
   and trigger-match evaluation. A runtime expression can be validly bound
   without its value being known at compile time.
5. Lower to typed executable operations, conditions, selectors, costs,
   continuous-effect components, and event matchers. Preserve sequencing,
   simultaneity, target provenance, actor/chooser, and optionality.
6. Expand keyword behavior or reference a versioned engine primitive. Both
   approaches must identify the actual mechanic implementation and rules
   edition. An atomic keyword name is not sufficient execution support.
7. Derive transitive required capabilities and serialize/read back against the
   schema. A separate admission check resolves these requirements against an
   actual engine build, support profile, and complete pod configuration.

Production executable semantics belong to the compiler/core boundary, not two
independent TypeScript and Rust binding implementations. The existing
TypeScript reference graph is diagnostic input whose correctness and version
must be checked before reuse. See [language boundary](runtime-language-decision.md#5-parser-to-rust-boundary).

### 3.2 Proposed artifact organization

Use canonical Oracle definitions and separate printing/set membership indexes.
This refines the earlier per-set storage proposal: sets can reference canonical
definitions instead of duplicating executable logic for every reprint. Define
face identity explicitly for reversible cards; display names are not keys.
Per-set exports remain possible, but are not required for the first engine.

| Record | Required information |
| --- | --- |
| Build manifest | Artifact/schema version; rules edition/hash; parser and compiler source-content hashes; compiler revision/toolchain/lockfile identity; input corpus/evidence hashes; canonical content digest |
| Card definition | Oracle identity, source provenance, all faces/layout, structured characteristics, color identity, commander/deck-construction permissions, executable abilities, required capability IDs/versions |
| Ability definition | Stable build-local ID, source face/block or intrinsic-rule provenance, kind, functioning-zone policy, typed body, evaluation scopes, bindings, rule references |
| Printing index | Printing identity, Oracle/face identities, set membership and source provenance; no independent executable behavior |
| Compile result | `lowered`, `unsupported`, or `invalid`, with paths, reasons, source association, and required capabilities; lowering alone makes no engine-support claim |
| Admission result | `admitted` or `rejected`, keyed by engine build, IR/catalog digest, profile digest, policy digest, and pod configuration; precise violated constraints |
| Support profile | Versioned closed definition pool, mechanics/dependencies, admitted compositions/layouts, commander configurations, explicit exclusions, rules edition, and evidence references |

Source text is allowed for diagnostics/provenance; it is never an executable
fallback. Names and subtypes are validated data vocabulary, not raw rule bodies.
Unknown semantic fields, operation variants, or capability versions must fail.
Optional metadata extensions have an explicit nonsemantic namespace.

Define a machine-readable schema as the interchange authority and generate
language types/validators where practical. Add shared positive and negative
fixtures to verify generated and handwritten decoders agree. Do not maintain
independently evolving JSON schemas and executable type definitions. The
current TypeScript AST remains authoritative for the source parser until an
explicit source-AST schema export is introduced.

Keep three contracts separate: source archive/AST, executable IR, and external
decisions/observations. Each has its own version and compatibility checks.
Snapshot encoding is an internal versioned contract, not the public protocol.
P1a selects the source/IR authority and generation path for its subset; P8
selects the external protocol contract. Do not build a general schema platform
before the first worked examples.

Strictness applies to semantic fields consumed by the compiler and engine.
The complete external source object remains an opaque provenance payload with
its original bytes/hash; additional unconsumed Scryfall metadata does not by
itself invalidate that payload. Validate every consumed characteristic and
required layout field explicitly. Unknown AST/IR semantics still fail closed.
The schema-3 adapter must verify the archive's existing hash conventions,
including its JavaScript serialization, rather than silently substituting the
new IR canonicalization algorithm. Preserve exact serialized subvalues where
that permits direct verification; do not require a general JavaScript serializer
port merely to rehash already canonical archive bytes. Add cross-language fixtures for ordering,
Unicode, numeric encodings, missing versus null fields, and duplicate-key
rejection at owned semantic boundaries.

### 3.3 Admission policy

A card is admitted only when all faces/abilities/layout rules compile and all
required features and known composition constraints are supported. Deck
admission validates every card, variant, and configuration before starting.
An engine supports a declared **profile**, not an arbitrary mix of individually
implemented operations. Feature presence is necessary, not proof of every
interaction; maintain an interaction matrix and conservative profile limits.

Admission runs over the **whole pod**, not just each deck independently. The
initial mechanism is an explicit allowlist of immutable definition digests plus
typed profile constraints. Start with a small fixed suite of reviewed deck/pod
configurations if arbitrary deck composition cannot yet be admitted safely.
Profiles must state which of those two guarantees they provide. A pool allowlist
alone must not imply all possible combinations are tested.

| Profile component | Admission obligation |
| --- | --- |
| Definition catalog | All selected definitions/faces match allowed digests; permitted generated definitions are closed and validated |
| Mechanic requirements | Required operation, intrinsic rule, layout, trigger, and functioning-zone versions are implemented by the selected build |
| Composition constraints | Typed required/forbidden combinations checked across all decks and reachable grants/copies; diagnostics identify the conflicting inputs |
| Configuration | Supported player/commander arrangements, match versus scenario mode, and any allowed fixed deck/pod manifests |
| Policy | Separate Commander construction/ban checks and explicit session exceptions |
| Evidence | Reviewed scenarios, interaction cases, and known limits for this profile release |

Keep executable admission constraints separate from the test matrix. Pairwise
coverage does not prove arbitrary higher-order interactions. Unknown closure
or composition behavior requires narrowing the profile, not a claim that the
compiler can decide every possible interaction. Never truncate an effect or
cap a legal board size to enforce a support claim; host limits yield a resource
outcome. Post-load admission is rechecked at match creation, even for a cached
compiled artifact. A scenario-only admission cannot be used to start a match.

Capability checks must cover generated tokens, copied definitions, granted
abilities, and definitions discoverable during a game. Effects reaching
outside a closed card pool remain unsupported until the reachable definition
catalog is validated. Encountering an unhandled operation is an engine fault,
never a skipped effect or a player loss.

Closure concerns executable definition/ability families, not enumeration of
every possible numeric value or runtime token instance. Initially allow fixed
token templates only when their complete behavior is supported. Later
parameterized templates require typed, validated characteristics/abilities and
explicit composition constraints. Copy/grant operations cannot introduce an
unvalidated semantic body merely because its containing instruction is known.

Commander deck legality and execution support are separate checks. Pin the
Commander banned-list policy and source date/hash before validating decks;
do not use an engine capability restriction as a ban, or a legality flag as
proof of executability. Rule Zero exceptions are explicit session policy,
default off, with reports and replay provenance. Brackets/power matching are
application policy, not a replacement for game rules or capability admission.
Use the [official banned list](https://magic.wizards.com/en/banned-restricted-list)
as policy evidence; never fetch a live update halfway through a simulation.

### 3.4 Intrinsic behavior and functioning zones

The compiler preserves base characteristics and records required intrinsic
rule families. The core combines them with current derived characteristics to
determine active abilities and permissions. Intrinsic, printed, and granted
abilities have distinct provenance but use the same execution machinery.
Do not permanently bake a dynamic type-derived ability into printed text.

For example, the frozen source definitions for Forest and Plains have only
parenthetical mana reminder text; the current parser produces empty ability
lists. That is valid parsing. The core must still supply mana abilities from
their basic land subtypes (CR 305.6). Once type changes are admitted, adding or
replacing land subtypes and removing abilities must interact through the
characteristic evaluator (CR 305.7, 613). These tests prevent both missing mana
abilities and double-counting a printed/reminder representation.

Every ability has a typed functioning-zone policy derived from the rules and
source, including default behavior and supported exceptions under CR 113.6.
Zone permission is independent of evaluation stage and effect duration. Model
entry-time behavior, characteristic-defining abilities, and pregame construction
permissions explicitly. Unknown exceptions reject the card; an ability must
not become active in every zone because no narrower policy was recorded.

The core owns the general rules for type behavior, state-based actions, casting,
and Commander designation. The compiler owns source-specific expressions and
permissions. Both refer to the same versioned mechanic IDs; the compiler must
not emit a second implementation of general game rules as per-card scripts.
Rule references in this subsection use the [pinned rules text](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).

### 3.5 Minimum executable semantics contract

Before generalizing IR, specify the following for the first supported operators:

| Contract | Required meaning |
| --- | --- |
| Spell program | One spell announcement/target environment and ordered resolution program, composed from its spell-ability blocks; static/triggered/activated abilities keep their own roles |
| Value/expression | Exact type/cardinality, evaluation stage, binding owner, and snapshot-versus-reevaluation policy |
| Selection | Stable occurrence ID, target versus non-target choice, chooser, domain, joint constraints, and when the selected values become available |
| Instruction | Preconditions, proposed events, possible decisions, completion result, and continuation after suspension |
| Outcome | Separate choice/payment facts, attempted action, actual event results, and rule-authorized successor references; no generic success boolean |
| Frame | Program/instruction IDs, stage/cursor, source/controller context, scoped bindings, target legality, child event/choice progress, and return destination |
| Lifetime | When captures, links, trigger instances, and outcomes expire; what survives source movement or departure |

Spell-level target validation covers all applicable target occurrences across
all resolving spell blocks. Do not independently resolve each paragraph or
discard only its targeted paragraph when the entire spell has no legal targets.
Separate later triggered abilities, including reflexive triggers, retain their
own announcement/resolution lifecycle (CR 113.3a, 603.12, 608.2).

The operation vocabulary can begin as typed instruction trees. A minimal frame
is conceptually `program + cursor + stage + environment + child-progress`.
The environment contains typed slots, not arbitrary JSON or raw text. Reading
an unavailable slot is a compiler rejection where statically detectable, or an
engine invariant fault; an absent event result has an explicit empty/absent
value and never falls back to the original object automatically.

### 3.6 Worked contract examples and fixture progression

These are semantic sketches, not implemented IR syntax or claims of current
card support. Names/Oracle identities below were checked in local source data;
P1 must extract exact source objects and their hashes into reviewed fixtures.
All additional faces/abilities remain subject to complete-card admission.

| Fixture | Lowering/runtime obligation | First evidence |
| --- | --- | --- |
| Forest (`b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6`) | Empty parsed abilities plus land characteristics require intrinsic mana behavior; default functioning zone and activation timing remain explicit | P1a lowering; P1b activation and snapshot |
| Unsummon (`837182db-1bf3-4a2c-bd01-1af9d9873561`) | One announced creature target; resolution rechecks its incarnation/legality, computes its owner, and proposes a hand move | P1a lowering; P1b ordinary move; P4 commander replacement variant |
| Cruel Edict (`10c585c4-bf5b-4d8f-94a9-e9a5036a688f`) | Announced opponent target differs from that opponent's resolution-time choice of a creature to sacrifice | P1a scope sketch; P3 another-player decision and restore |
| Mogg Fanatic (`7efd5d62-6da2-428a-98e3-5b842f668656`) | Sacrifice is an activation cost; the resulting ability retains its source identity and required last-known facts for damage | P3 source-movement fixture, only when its complete target domain and damage behavior are admitted |
| Synthetic multi-block spell | Two spell blocks share one stack object, announcement context, and complete target-legality check | P1b interpreter fixture; P3 real-source counterpart |

For Unsummon, the conceptual lowering is:

```text
announce: bind target t0 = one legal creature (record current incarnation)
resolve:  perform spell-wide target-legality check
          if t0 remains legal, read owner(t0) and propose move(t0, owner's hand)
          retain the move outcome through replacement and commit
finish:   perform the spell's normal completion procedure
```

At P4, repeat that fixture with a designated commander. Its owner may replace
the hand move with a command-zone move. The interpreter resumes after the
replacement decision with the actual destination/outcome; it must not also
move the card to hand. Restore once while awaiting that decision and compare
with uninterrupted execution (CR 903.9b, 616).

For Cruel Edict, bind the opponent at announcement, then pause during resolution
for that opponent's non-target creature choice if required. The frame retains
the resolving spell and chooser binding while the active player has no new
priority decision. Validate the choice, propose sacrifice, retain the actual
result, and resume. If the opponent has no creature, follow the rules for the
impossible instruction; do not invent a sacrifice or wait for an impossible
choice. This sketch does not make its chosen creature a spell target.

P1a reviews these shapes and negative cases; it need not implement their entire
mechanic families. P1b executes the smallest land/move slice. Each later gate
adds complete source→AST→IR→runtime fixtures alongside its new mechanics.

## 4. State and identity model

| Domain | Contents and invariants |
| --- | --- |
| Game identity/configuration | Match ID, Commander support/policy profile, rules/IR versions and hashes, original seating and live turn order, setup stage |
| Players | Life, counters, mana with restrictions/provenance, owned zones, failed-draw/loss conditions, concession status |
| Physical cards | Stable physical-card ID, owner, immutable definition ID, persistent metadata explicitly permitted by rules |
| Commander state | Designated physical-card IDs per player, pregame color identities/choices, command-zone cast history, per-recipient/per-commander combat-damage totals |
| Game objects | Zone-incarnation ID, physical-card link if any, controller and control-history facts, face/status, tapped state, marked damage, counters, attachments, combat membership |
| Stack | Spell/ability instances, controller, source identity and needed last-known information, announcement choices, targets, costs paid, continuation |
| Turn scheduler | Active player, priority holder or absence of priority, pass sequence, phase/step, extra/skipped turns/phases, cleanup state |
| Effects/abilities | Instances, timestamps, source/capture scope, affected-set policy, conditions, duration/expiration, use/prevention budgets |
| Event/choice context | Event batches, pending triggers, event-local results, link records, payment/turn history, pending decision |
| Departed players | Persistent player IDs, leave-game status, required last-known/history facts, pending-turn/duration effects; no recycled seat identity |
| Randomness | Versioned PRNG algorithm and complete state, plus simulation stream policy |
| Execution control | Runnable/awaiting-decision/finished/suspended/fault state, serializable frames, scheduler cursor, accepted input awaiting consumption |

Physical-card identity, an object's current incarnation, and an ability on the
stack are distinct. Zone changes normally create new objects; linking to a
successor requires a represented rule exception, not following the physical
card automatically (CR 400.7). Tokens and ability objects may lack a physical
card. Store last-known information only with explicit event/ability scope.

Use owned compact tables and typed IDs. Generation checks prevent stale handles
from resolving to a reused table slot. ID allocation/order is deterministic
within a branch. IDs are not memory addresses. Record owner and controller
separately, and do not overload a zone's owner as an object's controller.

Separate stored base facts from **derived characteristics**. Marked damage is
not a decrement of toughness. Counters, temporary effects, and control changes
remain distinct inputs to the evaluator. Derived caches are disposable and
cannot alter semantic hashes or behavior when invalidated/rebuilt.

Game numbers use exact integer arithmetic, including negative values where
permitted. Propose an arbitrary-precision signed value type, with an optimized
small-value representation evaluated later. Wire game quantities are canonical
decimal strings; bounded indexes/counts use explicitly ranged integers. Do not
silently saturate, wrap, use floating point, or limit player choices to machine
word size. Computational exhaustion is a resource result, not a rules result
(CR 107.1). Structural bounds limit representable storage, not legal game
quantities; exhausting storage returns a resource outcome. The exact arithmetic
backend is a P1a decision.

## 5. Core execution and decisions

### 5.1 Conceptual API

Names below are design vocabulary, not callable interfaces:

| Operation | Semantics |
| --- | --- |
| `prepare_game(config, definitions, budget)` | Validate the complete pod and loaded artifacts; return a prepared configuration or resumable validation work |
| `create_game(prepared, seed)` | Initialize an admitted configuration in runnable setup state; initialization work continues through `advance` |
| `advance(game, budget)` | Run automatic supported rules until a decision, completion, safe suspension, or fault |
| `decision_for(game, player, budget)` | Project that player's authorized current decision/constraints; use resumable query work if needed |
| `check_response(game, player, decision_id, response, budget)` | Validate the current stage without mutating semantic state; return rejection, an opaque validated response, or resumable work |
| `submit(game, validated_response)` | Recheck branch/revision/chooser, consume the validated response once, and make the game runnable; do not execute automatic rules |
| `query_choices(game, viewer, query, budget)` | Return authorized constraints/candidates or resumable query work; incomplete work is not a proof of legality or illegality |
| `resume_work(work, context, budget)` | Resume validation/query work against the same input/state identity, or reject an invalidated work item |
| `concede(game, player)` | Serialize an out-of-band concession at a defined input boundary; invalidate affected work/decisions and schedule immediate rules consequences |
| `observe(game, viewer, budget)` | Produce a visibility-correct view using the query-work protocol; never expose authoritative state by default |
| `snapshot / restore` | Preserve full semantic state, continuation, and PRNG with version/hash checks |
| `fork(game)` | Create an independent branch with identical semantic state and PRNG state |
| `run_batch(jobs, policy, budget)` | Execute many simulations without serializing every internal transition |
| `explain(trace_reference, viewer, budget)` | Project rules evidence through viewer visibility using the query-work protocol |

The conceptual transition is state + input → state + outcome. Internally, Rust
may mutate owned state; returning a new logical state does not require cloning
the whole game on every move. The core is synchronous. Async networking and
agent inference live in adapters.

Only the trusted core can construct a validated response. It is bound to the
branch, decision, chooser, response digest, and current revision; untrusted
clients send ordinary responses, never validation receipts. A receipt certifies
one decision stage, not successful completion of a whole future casting action.
Restored/forked games need their own validation context. Submission may reject
a stale receipt without changing state, but does not repeat expensive searches.
Executor instance tokens used to distinguish forks/restores are not semantic
game IDs and do not enter replay hashes or observations. Forks retain equal
semantic state while accepting only responses validated for their own context.

`runnable` is an internal state after creation or accepted input. `advance`
returns exactly one of these execution outcomes:

- `awaiting_decision`: player, decision ID, stage, allowed information,
  constraints, and continuation are explicit; setup uses this same outcome.
- `finished`: a rules-defined result and reasons.
- `suspended`: resource budget exhausted at a resumable boundary; not a draw.
- `fault`: engine invariant/unsupported-execution error with reproducible
  evidence; not a game result.

Validation/query outcomes are separate: `complete`, `rejected`, or `incomplete`
with a serializable work token and reason. Work tokens belong to the executor,
not game-rule state, and cannot be exposed with private scratch data. A caller
may abandon/restart a query without changing the game. If recovery promises to
resume work, persist its token alongside the game snapshot. Work is invalidated
when its state identity changes. A prepared configuration is likewise immutable
and tied to exact artifact/profile/policy identities.

### 5.2 Decisions and legality

Use stage-specific requests for priority actions, modes/X, targets and
distribution, costs/payments, attackers/blockers, resolution choices, trigger
ordering, replacement ordering, setup/mulligans, and supported special actions.
Not every decision occurs while its player has priority.

Every request has a deterministic branch-local ID, expected state revision,
authorized chooser, constraints, and continuation ID. Responses cannot be
reused after the request changes. IDs/revisions prevent stale submission;
server idempotency is a separate transport concern.

Legal-choice queries expose descriptors, candidate domains, joint constraints,
and validators. They need not enumerate every combination. Multi-target
distinctness, mode dependencies, distributions, mana payment, combat
requirements, and unbounded integer choices make full enumeration expensive
or impossible. A single-candidate eligibility list is not proof a whole action
is legal. Small finite action enumerators can be optional search adapters.

Candidate pages have stable ordering, a state-bound cursor, and an explicit
completeness flag. An interrupted constraint search cannot report an empty
complete domain. A sound witness may establish that one response is legal;
claiming that no response exists requires a completed proof for that query.
Enumeration limits affect strategy tools, never authoritative legality.

Queries must respect information rules. Do not reveal hidden card identities
through candidate IDs, error messages, or an omniscient legality oracle.
Search/fail-to-find and other hidden-zone mechanics need dedicated rules and
tests before profile admission. Agent sampling/pruning is strategy policy;
it never changes the authoritative set of valid choices.

Store execution frames and binding environments as serializable data. They
must survive choices made by another player, nested effects, and restore.
No borrowed closures, suspended native stack frames, or promises in game state.

Concession/leave-game is a supported out-of-band rules input, not restricted to
the conceding player's current decision. Serialize it through the authoritative
executor, record its ordering, and recompute/invalidate affected requests.
Disconnect is a server condition; it does not automatically mean concession.

Observations, explanations, and legality queries are semantically read-only.
They may populate disposable caches, but must not consume game RNG, allocate
semantic IDs, advance a frame, or change a semantic hash. Viewer handles are
derived or updated by deterministic visibility transitions, not allocated in
response to the number/order of observer calls. Query scratch state and cache
warmth cannot affect subsequent game behavior. Test repeated and reordered
queries, including queries from different viewers, against a no-query run.

### 5.3 Casting, activation, and invalid inputs

Casting/activation have ordered proposal, choice, legality, cost-determination,
mana-ability, payment, and completion stages (CR 601–602, 605). Mode-dependent
targets and alternative/additional costs must be validated in their actual
stages. Record the paid cost and announcement values for later queries.

Reject malformed/stale/unauthorized submissions without changing semantic
state or PRNG. Supported legal action proposals then follow rules-specific
staging. Do not equate arbitrary API rollback with reversing an illegal play:
CR 733 has exceptions involving library movement, reveals, and shuffles.
Initially admit only casting/payment features with reviewed reversal behavior;
implement further reversal cases before expanding that profile. Publication
of observations and trigger effects must follow the actual committed rules
stages, including hidden-information restrictions during casting.

Use the event pipeline for payments and resolution from the first executable
slice. A casting transaction retains the information needed for its supported
CR 733 reversal path; proposed, accepted-stage, and completed-action records
are distinct. The input journal retains accepted stages even if the eventual
play is reversed, so replay can reproduce the reversal and information effects.
This transaction is rule-specific, not a generic rollback of everything since
the last client request.

## 6. Rule execution subsystems

### 6.1 Commander and multiplayer foundations

Use CR 903 together with 103, 800.4, 802, and 806. The
[official Commander overview](https://magic.wizards.com/en/formats/commander)
provides format context; the pinned Comprehensive Rules govern exact timing
and exceptions, including command-zone movement.

**Pregame and deck validation.** Implement exactly 100 cards including the
commander configuration; singleton by applicable English/interchangeable name,
basic-land exceptions, admitted card-specific exceptions, color identity,
basic-land-type restrictions, and no sideboards (CR 903.4–903.7). The first
profile rejects unsupported multiple-commander and deck-construction abilities
rather than guessing. Eligibility must follow the pinned edition and card
permissions; do not assume every eligible commander is a legendary creature.

Color identity is established before play and differs from an object's current
colors. Preserve all relevant faces, alternate characteristics, and pregame
choices. Check trusted source metadata against admitted compiler rules; the
compiler must not infer color identity just from a mana-cost string. Setup
uses 40 life, face-up command-zone designation, seven-card opening hands,
the multiplayer free-first-mulligan procedure, and a first-turn draw for the
starting player (CR 103.5c, 103.8c, 903.6–903.7).

**Designation and casting.** Commander status belongs to a physical card and
survives zone changes/control changes; it is not acquired just by copying that
card's characteristics (CR 903.3). Command-zone cast permission and tax belong
to casting: add {2} for each prior command-zone cast by that casting player,
track each commander independently, and record a cast when it is completed,
even if the spell is later countered. Casting from another zone is not a
command-zone cast (CR 903.8). Integrate tax with ordinary cost determination,
additional/alternative costs, and reductions as those mechanics are admitted.

**Returning to command.** Graveyard/exile movement happens first; the owner may
then choose command-zone movement at the relevant SBA check, with the required
newly-arrived tracking (CR 903.9a). It is not a universal dies-to-command
replacement; actual graveyard entry and related triggers must be preserved.
Movement to the owner's hand/library instead has an optional replacement
effect, including its special reapplication exception (CR 903.9b). These paths
must be distinct in events, choices, bindings, and tests. Multi-component
commanders remain unsupported until their movement rules compile.

**Commander damage.** Record actual combat damage dealt to each player by each
designated commander across incarnations/controllers. Life gain does not erase
it; noncombat damage does not add to it; prevention/replacement can change the
actual dealt amount. The 21-or-more loss condition is an SBA (CR 903.10a).
Separate commanders never share one combined threshold. Self-damage and
control-change cases use the designated damage source, not its current owner
or controller. Store enough event provenance for future copying/merged cases.

**Multiplayer scheduling and combat.** Priority follows living players in seat
order; all required passes and trigger/controller choices use the actual live
player set. “Each opponent,” “each player,” selected opponents, and “you” are
different domains. Attack declarations choose defenders per attacker and can
attack multiple opponents in one combat. Collect defending-player choices in
the appropriate order and apply simultaneous events as specified; do not run
three independent two-player combats. Support optional simultaneous choices
without publishing private selections early.

**Player departure.** Implement the CR 800.4 leave-game procedure, including
owned objects leaving, control effects ending, remaining controlled objects,
stack/trigger consequences, and references to departed players. This procedure
is not itself an SBA, even when an SBA caused the player to lose. Preserve
the distinctions between owned and controlled objects and between removing
an object from the game and moving it to a graveyard. If the active player
leaves, the turn can continue without an active player; do not immediately
start the next turn (CR 800.4j). Adjust future turns, priority, choices, and
duration expiration using the relevant departure rules. A pod shrinking to
two players remains the original multiplayer game; do not reinitialize it
with two-player setup or victory rules.

### 6.2 Priority and stabilization

Use an explicit phase/step scheduler and priority state machine (CR 500–514,
117). Untap and cleanup require their own scheduling rules. Passing with a
nonempty stack differs from passing with an empty one. Mana abilities and
special actions are not ordinary stack resolution.

At the appropriate priority checkpoint, perform supported state-based actions
simultaneously, repeat as necessary, place waiting triggers in the required
order with required choices, and repeat stabilization before giving priority
(CR 117.5, 704.3, 603.3). Do not run a generic SBA loop after every individual
instruction during resolution. Trigger detection can happen earlier than
trigger placement on the stack.

### 6.3 Events, replacements, and prevention

Operations propose **event batches** containing typed occurrences, their group
relationship, actor/source, recipients, cause, and needed pre-event facts.
Replacement/prevention processing occurs before the relevant event is
committed. Recompute applicable effects after each replacement, preserve
event-lineage/application history, and ask the correct chooser when ordering
is required (CR 614–616). Prevention has its own budget and damage semantics.

Events are a structured hierarchy rather than an ungrouped list of mutations.
One happening can be observed differently by grouped and per-object triggers
(CR 700.1). Some events contain other events. Preserve simultaneity while
applying the replacement-order rules for contained events. Entry modifiers
are part of entry; entering tapped or with counters is not a later action.

Only actual happenings feed event-trigger detection and historical facts.
Replacing destruction with exile does not also emit an actual destruction.
Keep proposed/replaced/committed evidence distinguishable in debug traces.
Changes requiring look-back information use explicit pre-event views; neither
always matching before nor always matching after is a universal rule.

### 6.4 Triggered abilities

Define event matchers, event grouping, captured values, source/controller
identity, intervening conditions, placement choices, and trigger limits.
Pending triggers are separate from the stack. Apply active-player/nonactive-
player ordering and allow each controller to order their applicable triggers
(CR 603.3); never substitute table iteration order. Intervening-if conditions
have the required checks at trigger and resolution time (CR 603.4).

Triggered instances survive source movement when rules permit. Detect the
appropriate leaves-zone events using last-known information and look-back
rules (CR 603.10). Delayed triggers retain their own captured environment and
lifetime. History records actual events with explicit turn/instruction scopes.

Trigger kinds and scheduling classes are explicit capabilities. Do not force
every trigger through one event matcher followed by the ordinary pending queue:

| Family | Required behavior before admission |
| --- | --- |
| Ordinary event trigger | Match the actual event with the correct pre/post-event view; capture now, place later |
| State trigger | Detect a satisfied state and retain the per-instance retrigger lock until its previous instance has resolved or otherwise left the stack; distinct from SBAs (CR 603.8) |
| Triggered mana ability | Classify under CR 605.1b and execute at its specified immediate timing without the ordinary stack/priority wait (CR 605.4) |
| Trigger caused by another ability triggering | Preserve the two placement groups and their APNAP ordering (CR 603.3b) |
| Delayed/reflexive trigger | Retain its own origin, capture scope, controller rules, trigger lifetime, and later choices (CR 603.7, 603.12) |

The initial profile may reject every family beyond ordinary event triggers.
Record those exclusions in compiler/admission fixtures rather than treating
unsupported triggers as ordinary ones. Sources: [pinned rules, sections 603 and 605](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).

### 6.5 Resolution

Recheck target legality by declared target group, then interpret the body with
bindings and announcement values (CR 608.2). Distinguish all targets becoming
illegal from only some becoming illegal. Non-target choices occur at their
proper execution stage. Unsupported clauses do not become no-ops.

Runtime instructions retain actual event results for “this way,” successor
references, optional-payment outcomes, and later conditions. Distinguish
declining an optional action, choosing it but being unable to carry it out,
and carrying out an action whose result was replaced. Implement the relevant
outcome predicate; do not collapse all cases into one boolean.

### 6.6 Continuous effects and rules modifiers

One evaluator derives characteristics from base definitions, statuses,
counters, and active effects. It represents component layers/sublayers,
characteristic-defining behavior, timestamps, dependency ordering, and
affected-set/lifetime semantics (CR 611–613). Player and game-rule changes
have their own application rules; cost modifications are part of cost
determination. “Layer order” alone is insufficient.

Begin with recomputation as the correctness reference. Later caches/indexes
must be differential-tested against it and have explicit invalidation inputs.
Effects created by resolution may capture a set/value differently from static
abilities that reevaluate. Granted abilities must enter the same ability
system with provenance and executable capability checks.

### 6.7 Combat and state-based actions

Combat stores declared participants, defenders, block relationships, damage
assignment, and removal from combat. Validate a declaration as a group, with
restrictions, requirements, optional costs, and current characteristics.
Individual “can block” checks are insufficient (CR 508–510).

Use the pinned current combat rules; do not import historical damage-assignment
ordering behavior. Combat damage events preserve simultaneous application,
and first/double strike require correctly scheduled damage steps before those
keywords are admitted.

SBA categories are distinct typed operations: zero toughness, lethal damage,
player loss, illegal attachments, token cleanup, uniqueness rules, and others
as admitted (CR 704). A zero-toughness creature going to the graveyard is not
destruction. Damage-caused destruction and indestructibility are separate
semantics. One simultaneous SBA batch can require another batch afterward.

### 6.8 Loops and resource bounds

An execution budget is a host/search resource policy. Exhaustion produces a
resumable suspension or an explicitly truncated simulation result, never an
automatic game draw. A draw from mandatory looping requires the actual rules
condition (CR 104.4b, 732); optional loops and legal shortcuts need separate
choice logic. Repeated hashes alone do not prove the rule applies.

Initially mark unsupported shortcut/loop handling explicitly. Profile fixtures
must either exclude such cases or provide the supported adjudication path.
Preserve continuation and randomness when yielding; do not skip instructions
to fit a budget. A practical budget cannot promise to decide every possible
game's termination.

Core budgets count deterministic units such as instructions or event work.
Wall-clock deadlines belong to adapters, which request cancellation/yield at
safe boundaries. Scheduling or machine speed must not alter a completed job's
rules result. Persist progress needed to resume an interrupted expensive
operation; avoid repeatedly restarting a query that can never fit one budget.

Account for selector visits, constraint-search work, characteristic evaluation,
and event processing, not just top-level instructions. Specify a versioned
work-unit policy and resumable cursors for expensive operations. Validate
allocation growth before committing large event batches; exact arithmetic and
legal token counts do not imply unlimited host memory. Define memory/quantity
encoding resource failures explicitly, with the last coherent state retained.
An OS-level process failure cannot be promised resumable; adapters recover from
the last durable checkpoint/input journal. Transport size limits reject an
oversized request as a resource/protocol limit, never as a Magic rule judgment.

Budget partitions must not change completed rules results: running with one
large budget and repeatedly resuming with smaller sufficient budgets must
produce the same semantic result, PRNG position, and committed event sequence.
Permit a documented minimum work quantum; a budget below it returns a useful
required-minimum result instead of an endless no-progress retry.

## 7. Observation, replay, and traces

Authoritative state contains private hands, library order, hidden choices, and
PRNG state. Player observations expose only information permitted at that
point. Spectator and post-game omniscient views are explicit roles, with
release policies. Internal IDs, RNG seeds, hashes, and traces can themselves
leak hidden information; they are not automatically public.

Use viewer-specific opaque handles for objects a viewer may refer to. Specify
when those handles survive movement/reveals/shuffles and when they must change
to avoid tracking hidden cards. Persist visibility/knowledge needed for legal
observations; do not equate a remembered identity with current permission to
see a card. Server serialization must consume observation types, not full
game-state types with fields removed opportunistically.

The replay authority is configuration + exact card/rules/engine identities +
initial PRNG state + ordered accepted decision-stage inputs and required
external inputs, including concessions and later-reversed casting attempts.
Derived events can verify replay but are not a replacement for the inputs.
Store deterministic transition hashes/checkpoints and fully serializable
frames. Specify canonical field/order/number encoding; collection iteration
and serializer defaults cannot define the hash format accidentally.

Determinism is required across supported native architectures for the same
engine/IR/rules/PRNG versions and inputs. Changes that alter semantics require
new identities. Old replays require their original engine/artifacts; silently
replaying them against new rules is prohibited. Transport retries are not
additional replay decisions.

Record an out-of-band input's deterministic execution boundary as well as its
order relative to other inputs. A concession received during automatic work is
applied at the next supported safe boundary before further ordinary game work;
replay injects it at that boundary, independently of wall-clock arrival. Bound
the executor's yield interval without checking SBAs at extra rules checkpoints.
Normal rules execution cannot continue past a pending leave-game procedure.

Trace entries link decision, ability/instruction, event lineage, rule IDs,
object incarnation, and actual result. Keep three modes: off, compact replay
verification, and detailed diagnostic evidence. Results must be identical
across trace modes. Private evidence is filtered before explanation delivery.

### 7.1 Reproducible bundles and retention

A digest verifies an artifact but cannot recover missing bytes. Every released
profile and retained benchmark/replay run has a manifest referencing immutable
artifacts in a local content-addressed store initially. No cloud service is
required. A bundle contains or can resolve:

- Exact source fixtures/corpus and evidence, parser archive/schema identity,
  rules bytes, and legality-policy bytes with dates and digests.
- IR/catalog/profile/constraint artifacts and the engine build identity.
- Source-content snapshots for dirty builds, toolchain/lockfiles/build flags,
  and either a runnable build for its recorded target or reproducible build
  inputs. A Git revision alone cannot identify uncommitted implementation.
- Game configuration, initial randomness, accepted inputs/boundary positions,
  checkpoints, expected verification hashes, and diagnostic failure context.
- For benchmarks/search, workload/policy/sampler versions, seeds, measurement
  environment, raw samples, and outcome classification.

Bundle verification checks every reference and fails clearly on missing bytes
or incompatible versions. Retained runs/profile releases pin their dependency
closure against garbage collection. Explicit deletion may remove a run and
release its pins; it must not silently break another retained replay. Keep
downloaded/generated bulk artifacts ignored, while committing small reviewed
fixtures and compact evidence summaries. Test restoration from a fresh artifact
directory using a retained bundle, with network access disabled.

Full bundles are private: they include hidden cards, observations, and seeds.
A player-facing replay/explanation export is a distinct projected artifact and
must not expose the authoritative bundle manifest or content store.

## 8. Search and batching

Keep search policy outside rules execution. Provide exact fork/snapshot and
validated choice application; add cheap branch disposal. A fork duplicates
semantic state including PRNG position, but shares immutable card tables.
An optional later apply/undo path must restore state, continuations, IDs,
randomness, and semantic hash exactly, and cannot undo unrelated host effects.

Commander search has multiple independent decision makers and non-zero-sum
intermediate incentives. Return per-player outcomes, elimination order where
defined, and live-player state. Do not build the engine/search API around a
two-player alternating minimax value or one player's negated reward. The
search strategy may use multiplayer tree search, policy rollouts, or another
method; political evaluation and deals do not become authoritative rules.

Separate three notions: authoritative full state, a player's observation, and
an agent's belief/sample of hidden state. Information-set sampling must respect
the player's information and legal source distributions. A search agent does
not get opponents' private cards because it is running in process. Explicit
omniscient benchmark policies are permitted only as labeled test modes.

### 8.1 Initial information model and sampled worlds

The proposed first search workload uses **open decklists** as explicit session
information policy: all four initial deck compositions are supplied to agents,
while hands, library order, and private choices remain private. This is a
simulation policy, not a claim that Commander requires open lists. Unknown-list
inference is deferred until a consumer requires it. Ordinary gameplay agents
can still operate only on observations; they do not need a sampler to play.

The host gives each agent a versioned `SearchContext`: that player's permitted
observation and observation history, permitted decklists, public configuration,
definition/profile identities, and a separate sampling seed. No authoritative
snapshot, private internal ID, game seed, or private execution frame is included.
Remembered reveals constrain beliefs without granting a fresh reveal. Known
library positions, prior choices, and publicly implied movements are constraints
only to the extent that the player can know them.

A strategy-side sampler proposes a hypothetical world from that context. A
trusted simulation builder validates counts, identities, zone/history/capture
consistency, supported decision boundary, and the root player's observation
before constructing an executable simulation. It may use a candidate history
replayed under the same core, or a supported reconstruction schema; it must
not fill unspecified fields from the real hidden state. The sampled world has
its own RNG/ID context and can be forked without affecting the real match.
Sampling weights and inferred opponent policies are strategy assumptions, not
facts certified by the rules engine. Record them in search results.

Start at ordinary priority boundaries for a declared pool with no unresolved
private continuations. If the builder cannot reconstruct a consistent state,
return a distinct unsupported/incomplete sampling result. Never silently switch
to the real state. Later profiles add mid-resolution/private-choice boundaries
with explicit capture reconstruction. During each rollout, each policy receives
only that simulated player's permitted view; sharing a sampled world does not
authorize every policy to inspect it omnisciently.

P2 tests the context boundary; P7 implements the first useful sampler/builder.
Required properties: equal permitted histories yield equal SearchContexts and
sampling outputs for the same sampler seed; every sampled root projects to
the expected root observation under an explicit mapping of public/own-object
handles into the sampled world; inconsistent samples reject; future hypothetical
reveals arise from the sample; no reference or seed is borrowed from hidden
authoritative state. Full-state benchmark policies remain a separate labeled
workload and cannot stand in for this acceptance evidence.

### 8.2 Randomness, transpositions, and batch results

Exact replay uses the same PRNG state. Independent rollouts use an explicit,
versioned derivation from job seed and simulation ID; worker scheduling cannot
choose seeds. Chance handling and any agent RNG are separate from game RNG.
Search transpositions must account for decision stage, captured/history state,
hidden-state assumptions, and the chosen randomness policy. Hashing only the
visible battlefield is not sufficient.

Batch results include completed games/branches, suspensions/truncations,
faults, decisions/transitions, elapsed measurement data, and seed identities.
Do not count faults or truncated games as wins, losses, or completed throughput.
Cancellation is cooperative at safe boundaries. Parallel workers own separate
games/branches; no global mutable caches may influence outcomes.

## 9. Proposed source organization

Add the adopted Rust/Cargo workspace alongside the existing TypeScript/npm
workspaces. Exact crate splitting is a P1 implementation choice; avoid circular
dependencies and needless crates.

| Proposed location | Responsibility |
| --- | --- |
| `packages/game-engine/crates/contracts/` | Executable IR types, mechanic IDs/descriptors, and validation support, no runtime rules; external protocol types may remain adapter-owned |
| `packages/game-engine/crates/compiler/` | Source artifact adapter, binding, semantic lowering, admission reports |
| `packages/game-engine/crates/core/` | State, scheduler, decisions, events, rule evaluation, snapshots/forks |
| `packages/game-engine/crates/sim/` | CLI, scripted policies, observation-based search contexts/sample builder, batch jobs, reproducible bundles and benchmark harness |
| `packages/game-engine/tests/fixtures/` | Small reviewed source/IR/scenario fixtures and expected results |
| `packages/game-server/` | Later hosting adapter; no independent rules implementation |
| `packages/card-data/` | Generated versioned definitions/catalogs; ignored bulk outputs |

Prefer ordinary modules and exhaustive dispatch to a general-purpose plugin
system or ECS framework initially. Choose an ECS, bytecode VM, custom allocator,
incremental layer solver, or JIT only with a demonstrated need. Compilation can
lower to compact typed instruction trees before any bytecode optimization.

Dependency direction is `compiler → contracts`, `core → contracts`, and
`sim → core`; orchestration may invoke the compiler offline. The core does not
depend on the compiler or parser. Compiler capability requirements refer to
shared descriptors; the engine publishes which versions it implements. Schema
generation runs as build tooling, not through a compiler/core dependency cycle.
Keep these as modules until a crate boundary has a concrete benefit.

## 10. Decisions still required

The first slice's source/IR authority, numeric backend, and initial pool are
settled in the [frozen brief](game-engine-first-scenario.md). Broader contracts
remain subject to their delivery gates. Languages are settled: TypeScript for ingestion/parser/archive, the deck-builder,
browser applications, and client SDK; Rust for the semantic compiler, engine,
simulation/search/native policies, and game server. Browser-local rules use the
Rust core compiled to WebAssembly. See the [adopted decision](runtime-language-decision.md).
The remaining choices concern implementation within this stack:

| Decision | Proposed default | Gate |
| --- | --- | --- |
| Compiler contract | Rust owns production executable binding/lowering; finalize supported source/IR mapping | P1a |
| Product scope | Commander only; four-player free-for-all first, separate scenario mode | Established by Josh; refine profile at P0 |
| Commander configuration | One supported eligible commander/player first; multi-commander permissions later | P1a/P6.n |
| Numeric backend | Adopted for first slice: `num-bigint`, canonical decimal strings, checked structural IDs | First slice implemented; broader use at P2 |
| IR authority | Adopted for first slice: Rust types, generated JSON Schema, pinned schema-3 source adapter | First slice implemented; extend at later gates |
| Public protocol | Separate generated client contract once a consumer exists | P8 |
| PRNG and shuffle algorithms | Fixed, versioned, cross-target reproducible; select before the first random operation | P1b/P2 |
| Branch representation | Clone baseline, optimize from measurements | P2/P7 |
| Initial card/profile pool | Small scenario allowlist first; complete pod admission and closed reachable mechanics | P1a/P5 |
| Simulator resource limits | Resumable budgets and explicit truncation | P2/P7 |
| Search information policy | Open lists, private zones, separate SearchContext/sample builder; priority-boundary sampling first | P0/P2/P7 |
| Artifact retention | Local content-addressed bundles, pinned dependency closure, offline restore test | P1b/P2 |
| Server framework and protocol | Rust server calling the core directly; select libraries and transport for the actual consumer | P8 |

Language adoption is complete. Package scaffolding, card-support claims, and
performance results still require implementation evidence. The
[implementation plan](game-engine-plan.md) defines completion evidence.
