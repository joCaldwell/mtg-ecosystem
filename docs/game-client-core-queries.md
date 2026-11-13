# Core action beginnings for player sessions

This is a bounded native M2 prerequisite, independent of serving/client M1. Base
source is `0b674d8872087bc96f6078d0daeb1fb2d6c17758` (merged server PR14, SDK
PR13 and inspection PR15). Implementation owns the dedicated core query module,
its tests/schema-export example, a narrow `engine.rs` module attachment,
`PlayerSession` methods and a permitted-handle iterator in `visibility.rs`.
Ownership was confirmed with the active engine lane before edits. No server,
SDK, client, compiler, contracts, admission pool or shared planning document is
changed by this feature. The concurrent Card IR reset is separately owned at source checkpoint
`ef237b21db575a9f06518dc1f7cb4a6f99cc4147`, integrated in main
`2c18fd30006f902ac616daf805a6311fdf014779`. Before native verification this
lane rebased cleanly onto that committed main: current IR version 1, regenerated
source-identical catalog bodies and updated admission digests. The query diff
contains no reset implementation; test/schema identity is qualified below against
this integrated baseline, not pre-reset artifacts.

## Native contract

`PlayerSession::action_beginnings(token, scope, page_size, budget)` binds the
already host-authorized viewer. `ActionScope` is the whole `Window` or one
currently permitted `Object { object: ObjectHandle }`. Object scope omits all
global beginnings. The host must derive the viewer from authentication; core
provides no caller-authentication or HTTP implementation.

`ActionQueryResult::outcome()` borrows the permitted result.
`into_parts()` consumes it into `(ActionQueryOutcome, Option<ActionContinuation>)`.
Only `ActionQueryOutcome` implements serialization and JSON Schema. The host
stores the opaque continuation, associates a separately generated authenticated
cursor, and calls `PlayerSession::resume_actions(continuation, budget)`. It cannot
serialize, inspect, clone or manufacture this continuation through the public API.
Dropping it or calling `cancel()` discards scratch work only. It is not a pending
command, validation receipt, durable recovery sidecar or semantic cancellation.

The serving owner agreed this separation for a **later transport integration**.
M1 does not export a new query route or incorporate this schema. Cursor format,
expiry, per-seat concurrency, byte limits and transport failures remain host-owned.
A restart can restart the pure query; there is no serialized query recovery here.

| Outcome | Meaning |
| --- | --- |
| `complete { page }` | This scope's finite initiation domain has been exhausted. Earlier partial pages plus this page are the result. |
| `incomplete { page, required_minimum: 1 }` | Budget or page capacity stopped work. A private continuation exists. This is never an empty legal-action conclusion. |
| `stale` | No matching current viewer decision/token, or resumed work belongs to another viewer/executor/state epoch. No hidden reason is supplied. |
| `unavailable { reason }` | Invalid viewer/page size, unavailable boundary, unsupported window, absent/foreign/stale object scope, or invariant failure. No partially trusted menu is returned with this outcome. |

Every page echoes the submitted token/scope and contains an incremental
`beginnings` vector. Resuming **never repeats** previously emitted entries.
A page that exactly fills its capacity is incomplete even if its last entry was
the final available beginning. Resume must run to `complete`; its final page can
be empty. Budget zero does not initialize the request or enumerate objects; its
incomplete page has `context: null`. After one initialization unit, successful
pages have context. Token/scope checking also requires this first unit; invalid
viewer/page size and existing setup/concession guards can reject before it.
An echoed caller token/scope in a zero-budget result is not validation.

`page_size` accepts 1 through native `ACTION_PAGE_MAX = 64`. This caps the number
of output entries per call, **not bytes, wall time, or total result cardinality**.
Caller-supplied `Budget` caps deterministic work steps; `Budget::unlimited()` is
available to trusted native callers. No 10,000-unit HTTP allowance, 64-KiB reply,
60-second expiry, or one-continuation-per-seat rule is established here. Those
were investigation bounds in the earlier integration report.

## Exact completeness and context scope

The supported request variants are `PlayerRequest::Priority` and `::Mana`.
The latter includes the existing nested permanent-mana payment request. Other
request variants explicitly return `unsupported_window`, including targets,
spending, mana color, X, modes, combat, resolution options and opening mulligans.
Automatic setup and pending concessions preserve the existing query boundary
rejections. A submitted pending input has no usable decision token.

Priority's syntactic domain is global Pass, then each permitted object in lexical
handle-string order: PlayLand, Cast, ActivateMana, then each existing compiled
activated-ability index in ascending order. Mana's domain is FinishMana(false),
FinishMana(true), then ActivateMana for each permitted object. A one-object scope
uses the same local order. These are syntax candidates, not a duplicate legality
table: **every emitted initiation passes the existing budgeted
`check_player_response`/`check_response` path**. No rejected-response text is
returned. The receipt is immediately discarded. Submission must validate again.

`ActionInitiation::response()` mechanically produces the corresponding existing
`PlayerResponse`. A listed beginning establishes only that initiation. It does
not promise successful eventual targets, costs, payment or resolution. In
particular Cast is not an affordability claim, and FinishMana's two choices
retain native payment/reversal semantics; they are not a generic Undo feature.
The API supplies neither strategy ranking nor all target/payment/X combinations.
Future response families need a fresh discovery-coverage review; “complete”
means the explicitly described supported initiation domain, not every possible
future gameplay action.

Minimal `ActionDecisionContext` is `priority`, or `mana { cost, source }` with
exact cost copied from the existing permitted request. The source is `available`
with its existing object handle only when that request already supplies
`mana_ability_source`; ordinary spell/ability payment source context is explicitly
`unavailable`. No stack-source, source-block, mode or resolution-context mapping
is guessed. Initiation object/ability references are present directly in its typed
variant. Every `ActionBeginning.label` is `unavailable`: compiler-owned semantic
source-label mappings do not exist in this build. No Oracle parsing, source-text
heuristics, localized prompts or card-name branching occur in runtime discovery.

## Privacy, progress and ownership invariants

The new helper traverses the viewer's **existing published handle map**, which
publication already prunes for visibility/incarnation changes. It never scans
private object IDs for discovery order, generates a handle or republishes a view.
Handle resolution rechecks current incarnation and visibility. Continuations bind
viewer plus private executor/state identity using existing identity objects;
fork/restore/semantic changes invalidate them even if a public token happens to
match. Neither those identities nor any state digest enters the permitted schema.

One initialization step builds context, one step creates each syntax candidate,
and subsequent steps resume that candidate's existing validator one unit at a
time. One final step detects exhaustion. At most one candidate/validator work,
one object cursor and request context are retained. Entries already emitted do
not accumulate in the continuation. No private object count/iteration influences
the public budget transcript. This is deterministic logical progress, not a hard
CPU-instruction bound: shared native predicates have their existing internal
costs. No claim of constant-time execution or timing-side-channel resistance is
made by the focused paired-state tests.

The core has no clock, HTTP, auth, database or server DTO dependency. A query
cannot advance rules, submit a receipt, change a resource clock, publish history,
move a cursor, or turn resource exhaustion into a game outcome. Host scheduling,
quotas and response serialization remain outside this API. The client must render
permitted results and request human intent; it must not fill unavailable context
using executable IR or printed card text.

## Verification and source qualification

Focused execution completed on the integrated schema-1 baseline plus this
feature. Exact native-source hashes are retained with the tested executables. The pinned toolchain is Rust/Cargo 1.99.0 and Node
26.11.1 via `scripts/engine-env.sh`. Doctor reports missing root TypeScript/Ajv
and workspace dependencies; this native-only lane does not install/change root
dependencies or share node_modules. The structure/dependency guard and targeted
rustfmt/whitespace checks pass. Native work ran after serving M1 and held `~/.local/share/mtg-ecosystem/engine-verification/cargo.lock` across cache
preflight, Cargo and executable preservation. Coordinated process-local cache:
`/home/josh/.t3/worktrees/mtg-ecosystem/perf-engine-verification-loop/target`,
`ENGINE_CACHE_MAX_GIB=32`, `CARGO_INCREMENTAL=0`. The inherited empty default
cache and all historical evidence remain untouched. The native slot was released
to the engine lane after copying both executables; no native process remains.

Explicit schema export is the native example `action_query_schema`; building or
running it is never an implicit client/SDK action. It exports a standalone Draft7
schema for `ActionQueryOutcome`, not the server protocol. Schema-relevant types
are ActionScope, ActionInitiation, ActionLabel, ActionBeginning,
ActionSourceContext, ActionDecisionContext, ActionPage, ActionUnavailable and
ActionQueryOutcome; referenced token/handle/cost types retain core authority.

Remaining gates: separate server transport/cursor lifecycle and quota tests,
generated SDK validation, session ownership/recovery, client interaction coverage,
compiler source-label mapping and richer source/decision context. No full engine
matrix, release/performance/retention run, arbitrary-card admission, natural game,
browser interaction or deployment is claimed by this core prerequisite.


## Executed checks and retained evidence

On 2026-11-12 America/Chicago, pinned Rust 1.99.0:

| Check | Result |
| --- | --- |
| `cargo test --locked -p mtg-core --lib action_query::tests -- --nocapture` | 11 passed, 714 filtered; test execution 0.16 seconds after compilation |
| Same copied test binary, `player_session::tests` | 4 passed |
| Same copied test binary, `payment_query_tests` | 3 passed |
| Same copied test binary, `visibility::index_tests` | 3 passed |
| `cargo test --locked -p mtg-core --doc ActionContinuation` | 1 compile-fail test passed; continuation does not serialize |
| `cargo clippy --locked -p mtg-core --lib --example action_query_schema --no-deps -- -D warnings` | Passed |
| `cargo build --locked -p mtg-core --example action_query_schema` | Passed; copied exporter executed explicitly for schema and `--identity` |
| `python3 scripts/engine-structure-check.py`, targeted `rustfmt --check`, `git diff --check` | Passed |

All 21 runtime tests executed, not merely compiled. New coverage includes budget
partitions 1/2/7/10,000, pages 1/2/64, zero-budget resume, cancellation, snapshot
and all-view purity, stale/foreign token/viewer/executor/epoch, current/foreign/
retired handles, object scopes, setup/pending/concession guards, unsupported target
and mulligan windows, invariant failure, schema privacy and outcome roundtrip.
A paired hidden-hand witness changes hidden card identity, physical ID and
incarnation while preserving the viewer observation; complete outcomes and every
partial-budget/page transcript remain equal. Restore preserves discoveries and
rejects pre-restore private continuations. Existing adjacent tests additionally
exercise retained payment work and incarnation/visibility index invalidation.

The two exact admitted four-deck profiles are `commander-foundations-v1` and
`commander-attachments-v1`, seed `[0; 32]` in trusted test setup only. Both run
through opening keep choices to priority/main, play a land, verify a second land
beginning is absent, begin a cast, and inspect its mana window. These are bounded
natural prefixes, not completed games or every reachable decision. The separate
private attachment board uses exact definitions from the admitted attachments
catalog to witness equip, ordinary activated abilities, land/permanent/creature
mana and nested Signet payment/source context. It does not broaden public pod
admission. Beginnings are compared against the native validator over the full
stated candidate domain, including a nonexistent ability index negative.

Measured small-witness bounds (not universal maxima):

| Witness | Entries | Deterministic one-unit resumes | Encoded output |
| --- | ---: | ---: | --- |
| Existing land-return priority scenario, window | 3 | 52 | Complete compact JSON 326 bytes; largest one-unit outcome 224 bytes |
| Private attachments nested-mana window, page size 1 | 6 | 67 | No whole-window payload bound inferred |
| Admitted foundations / attachments cast-mana prefix | 3 each | Not measured separately | No whole-profile payload bound inferred |

The 64-entry ceiling is a native paging policy, tested alongside smaller page
partitions and invalid 0/65/u16::MAX requests. These witnesses do not establish
64 KiB as a safe transport bound or 10,000 units as a sufficient completion
allowance. Byte caps must reject explicitly in the later transport contract,
without truncating or treating incomplete as a legal empty menu.

The source-backed scenario's actual permitted example (token/handles are scoped
test references, never reusable authorization) is:

```json
{
  "status": "complete",
  "page": {
    "token": "v0-d0",
    "scope": { "scope": "window" },
    "context": { "window": "priority" },
    "beginnings": [
      { "initiation": { "action": "pass" }, "label": "unavailable" },
      { "initiation": { "action": "activate_mana", "object": "v0-o0" }, "label": "unavailable" },
      { "initiation": { "action": "cast", "card": "v0-o1" }, "label": "unavailable" }
    ]
  }
}
```

All native evidence lives in this lane's ignored
`generated/core-queries/attempt02/`. Exact schemas and binaries were copied while
the shared lock was held; later cache changes do not invalidate those copies.
The before/after native source manifest matched byte-for-byte after the build.

| Artifact | SHA-256 |
| --- | --- |
| `action-query.schema.json` | `5eb6c30f6939eadb3192eb86b3f7e2f63f6ca769aea7abefe7ddd49a5e2125e5` |
| `action_query_schema` executable | `681dd1b590fb18edead421ef19dbca9e4f4d812ebca6efe945e5bb17e38a9095` |
| `mtg-core-tests-01` executable | `d9dda276b578b8f9760dc4fa40af5800cb8af670d892178345196f09a921404f` |
| `priority-example.json` | `9b8e3ffbd8ac302b00937d8c98be867d8ca87a85b100063117b48a6601470250` |
| `native-source-sha256.json` | `f052c389ae0d3faecb8a50f11f5d5acb1ea383536dc53fb3c16e0415c7f0da56` |
| `expected-ir.json` fixture file | `39d52718896cae7eea5fd538f5e70b7ed917c98f8281541425c29bd853176751` |
| `expected-match-ir.json` fixture file | `d60a2e36fc65db247c907ea237fac406c9ab2405f9acf006329bc51cb02d896d` |
| `expected-attachments-ir.json` fixture file | `fe30a6718b7277bde0c173b987f9019caf3fc3679557770826aa1c5614846794` |

Test engine build identity (debug profile, opt-level 2):
`3358a490195c117cea0a0f94371ebb4661563cfad558b25c076afdbac55dfea9`.
Schema-export engine build identity (debug profile, opt-level 0):
`76f5c0b792ff7488605ba811d76a2e7203c92413c6b9fe7bbbb18b59c09e1e02`.
Both are x86_64-unknown-linux-gnu, debug assertions enabled, current IR version 1.
Different build settings intentionally produce different identities. The native
schema is **not** the server protocol schema and must not replace its existing
SDK input. A later serving build will have its own combined source identity.

### Preserved failures and omissions

The first compile failed on two unqualified build-identity constants in the new
test harness. The fix uses `crate::BUILD_PROFILE`/`crate::BUILD_OPT_LEVEL`;
`generated/core-queries/action-tests-01.log` and its original source-hash manifest
retain that failure. The full new/adjacent focused tests then passed.

`attempt02/core-clippy.log` retains the failed all-target core Clippy check:
`architecture_exploration.rs` has `filter_map_bool_then`, `engine/events.rs` has
`items_after_test_module`, and `engine/shared_history_work_tests.rs` has
`large_enum_variant`. These files are unchanged from baseline. Production library
and schema-example Clippy subsequently passed without allowances or suppressions;
all-target Clippy is **not** claimed passing. No unrelated source was rewritten.

Root Node dependencies remain absent; the doctor failure is retained, with no
implicit bootstrap. Runtime verification did not require them. No clocks exist
in the deterministic core dependency surface; the core purity tests plus boundary
inspection establish no core clock effects. Actual host clock/quota behavior is
still a separate transport acceptance gate. Full core/sim/compiler/contracts/server
matrices, root Node suites, schema-SDK integration, release, dense-board performance,
retention and browser/gameplay acceptance were deliberately omitted.
