# Game client contracts, delivery and acceptance

`game-client` owns generated-protocol decision/session adapters, a qualified
captured preview and a standalone React/Vite harness. `game-ui` owns shared
permitted display with scoped styles and host React. `multiverse` owns product
account UX, navigation, Home, deck editor and Play setup; `deck-builder` retains
the existing auth/Actor/SQLite backend. Rust host/core own admission, legality,
visibility and durable execution. Product sign-in does not grant a Rust seat.
Read [application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch) and [client setup](../packages/game-client/README.md).

## Delivered scope and remaining gates

The production App joins through an operator-issued single-seat file. Public
`@multiverse/game-client/session` composes the same router-free table from an
Actor-authorized human File. Every non-query request has a local draft adapter;
source-dependent choices require complete producer context. Explicit confirmation
retains the original command context, journals exact bytes and reconciles unknown
outcomes. Local controls do not establish whole-profile coverage.
`fixtures/decision-controls/coverage.json` owns request/response coverage with
separate reachability, control, context and test evidence.
Review new variants explicitly; schema presence is never admission.

The production App composes session-owned action discovery in priority/mana
windows, with window/object scope, bounded pagination, status and explicit stop.
Pass priority is self-explanatory. Named land/cast, ability/mana and FinishMana
beginnings require their exact permitted labels/source and scope. Missing meaning
stays blocked. FinishMana's flag controls reversal only if payment fails.
Complete games, whole-profile usability and integrated product launch remain
qualification gates. Exact deck admission, private provisioning/recovery and
three separately owned automated seats are implemented; default browser launch
stays disabled pending integrated proof. Unsupported automation pauses explicitly.
The owner-scoped durable record supplies confirmed pauses/results after native
shutdown. Command controls can become read-only while the last permitted table
remains visible; command dispositions retain their separate native-status authority.
The product preview is a captured sample, never the selected deck or a real game.
Do not use legacy solo playtester briefs/components as guidance.

## Package, privacy and presentation boundaries

[The package manual](../packages/game-client/AGENTS.md#boundaries) owns layering:
session effects are separate from pure local drafts and permitted presentation;
shared display comes from game-ui. The router-free preview imports no session/auth/
SDK runtime, and its Node-only verifier preserves historical fixture qualification.

Core-observed current characteristics override printed reference values. Metadata
is offline-produced from exact archived source with catalog/printing/face/source
hashes; compiler source-to-ability labels remain a separate producer gate. No
Oracle heuristics, card-name dispatch, image API lookup or turn-time source fetch.
Missing metadata/art shows identity/current facts/text fallback. Invalid metadata
hash/schema is a blocking protocol error, never fallback success. Do not fabricate
source labels, X meaning, mode labels, payment intent or clocks from wire syntax.
No service worker/private cache, URL secrets, telemetry of private projections,
trusted snapshots, seeds, receipts or other seats' history.

## Authorization, reads and ownership

Import exactly one generated-schema-validated bootstrap through the file picker.
It must match the operator-configured canonical numeric loopback origin, actual
resume viewer, match, protocol, engine/server builds and catalog. Capability stays
only in memory; refresh requires reimport. Do not derive identity from URL labels,
trust an imported file to select a proxy target, or persist bearer tokens.

Acquire the exclusive Web Lock before every session network/journal operation.
No forced takeover, secret broadcasting or fallback ownership. The single read
owner controls processed cursor and recovery; no duplicate poller/clock. Keep the
last Observation/token across lifecycle-only pages. A changed generation/seat/
identity invalidates all asynchronous completions, drafts and effects before I/O.
Reads use bounded backoff/retry; a failed/partial response body is uncertainty,
not a successful response. Reauthorize/verify manifests before recovery.

## Durable command journal

Capture the **original complete CommandContext** when a draft starts: binding
(origin/match/seat/engine/server/catalog/protocol), incarnation, revision and token.
An explicit human completion carries that original context. Never stamp an old
selection with a fresh context after refresh, rejection or decision replacement.
Duplicate/stale/unbound intents have no effects. UI draft cancellation is not
server cancellation, Undo, concession or a new command.

SDK `prepareCommand` creates a detached frozen validated envelope/body pair.
`parsePreparedCommand` recovers strict original bytes; `submitExact` sends once.
Persist `{version,binding,exactBody}` in IndexedDB before sending, retaining original
ID/revision/token and serialization (including whitespace/member order). Only
transaction **complete** with strict durability authorizes send; request success
does not. No credential, full observation or redundant envelope is stored.

Database `multiverse-command-journal` version 1 has one `commands` store keyed by
JSON tuple `[server origin, match, seat]`, deliberately excluding build/catalog.
Scan that seat before current-build matching so incompatible unresolved work blocks
replacement. Atomic insertion checks key and total count across tabs. At most
64 unresolved records, each command at most 65,536 UTF-8 bytes (4 MiB body budget
plus bounded metadata/browser overhead); binding fields at most 512 characters.
Open/transaction timeout is three seconds. Denied/blocked/quota/corrupt/unsupported
storage fails closed, never resets or evicts unresolved work. Late opens close.

| Authoritative disposition | Journal behavior |
| --- | --- |
| Submission accepted / execution pending | Retain unresolved slot |
| Execution consumed_by_rules | Resolve consumed |
| Execution discarded_by_departure | Resolve departure-discarded |
| Execution cancelled / definitive cancelled rejection | Resolve cancelled |
| Other definitive rejection | Resolve rejected with public enum |
| Missing status / abort / timeout / transport or decode failure | Retain uncertainty; only exact original replay is permitted |

Cancellation concerns a queued server command. An accepted cancellation response
may repeat immutable earlier acceptance and does not by itself clear uncertainty.
Terminal deletion compares the entire original binding/body transactionally.
Before accepting new intent, get a coherent resume started after resolution;
a failed refresh keeps the gate. Advance controller incarnation even if token/
revision remain equal. Every completion/effect is fenced, including synchronous
subscriber cleanup during persistence/send/rejection and stop/StrictMode/seat replacement.

Four automatic status/replay opportunities per command authorization are driven
only by successful reads; successful reads do not renew that budget. Afterwards
explicit retry checks status/replays the same body once. Refresh scans rather than
manufacturing replacement. Browser command effects have a 30-second headers/body
deadline. Local stop, missing rows, changed view or incompatible build are not
terminal and never erase ambiguity. SDK itself has no journal/retry policy.

## Core action discovery

`PlayerSession::action_beginnings(token, scope, page_size, budget)` discovers only
`Priority` and `Mana` initiation, including nested permanent-mana payment. Other
requests return `unsupported_window`. Setup, pending inputs and concessions retain
existing query-boundary rejections. The host authorizes the viewer; core has no
HTTP/auth/clock/database dependency.

`ActionScope` is `Window` or one permitted `Object { object: ObjectHandle }`;
object scope omits global beginnings. Priority enumerates Pass, then objects in
lexical handle order: PlayLand, Cast, ActivateMana and ascending compiled activated-
ability indices. Mana enumerates FinishMana(false), FinishMana(true), then each
permitted ActivateMana. Every emitted beginning passes the existing budgeted
response validator; rejected-response text/receipts are discarded. Submission must
validate again. Cast is not affordability, FinishMana retains payment/reversal
semantics, and initiation completeness is not every later target/payment choice.
No strategy ranking or complete target/payment/X combinations are supplied.

`ActionQueryResult::outcome()` borrows its permitted outcome; `into_parts()` returns
that plus optional opaque `ActionContinuation`. Only the outcome serializes. The
host stores the continuation behind its authenticated cursor and calls
`resume_actions`; it cannot inspect, clone, serialize or manufacture continuation
state through the public API. Dropping/cancelling it discards scratch, never a
command, receipt or durable recovery record. Restart restarts discovery.

| Native outcome | Meaning |
| --- | --- |
| `complete { page }` | Scope exhausted; combine all preceding incremental pages |
| `incomplete { page, required_minimum: 1 }` | Page/work capacity stopped discovery; continuation exists |
| `stale` | Viewer/token/executor/state binding no longer matches; no private reason |
| `unavailable { reason }` | Unsupported/invalid boundary, viewer, page size or object scope, or invariant failure; no trusted partial menu |

Pages echo token/scope and never repeat entries. Exactly filling a page is still
incomplete even on its last candidate; only a resumed `complete` establishes
exhaustion, possibly with an empty final page. Native page size is 1–64 entries,
not a byte/time/total-cardinality bound. Zero budget initializes/enumerates nothing
and returns incomplete with null context; an echoed token is not validation.
One initialization unit checks token/scope and builds context. Invalid viewer/
page size and existing setup/concession guards may reject before initialization.
Each syntax candidate costs one step, its existing validator resumes one unit at
a time, and a final unit detects exhaustion. Continuation retains at most one
candidate/validator, object cursor and context, never accumulated emitted entries.
Logical work is not a CPU-instruction or constant-time guarantee. Unlimited budget
is trusted native-only use; transport ceilings are independently stricter below.

Context is `priority` or `mana { cost, source }`. Mana source uses the nested
mana-ability source or the ordinary current stack source's existing permitted handle.
`ActionLabel` retains `unavailable` and adds `available { text, source? }`: core owns
action wording, and optional source descriptions use offline compiler bindings.
Cast and nonintrinsic activations without a bound source label remain unavailable.
Intrinsic land mana uses the runtime's CR 305.6 production query and an explicit
core-authored source description naming the produced color.
Labels grant no affordability, target, payment or submission permission.

`Observation.decision_context` is optional for historical projections. Its optional
`source` supplies kind (`spell`, `activated`, `triggered`, `mana_ability`, `effect`),
Oracle identity, optional existing object handle/ability index and complete source
text. Compiler `SourceLabels` retains exact Oracle text, verified normalized source
blocks (self references may be `~`) and lowerer-owned activated/triggered/mana/static
indices. Standalone choose-one modes retain exact ordered option text. Schema 1,
mechanical capabilities and reviewed hosted/search sets stay unchanged; catalog
and build identities change. Metadata never admits new mechanics.

Current announcement/payment/target and resolution choices receive source context.
A resolving instruction supplies its exact source block or selected mode option; prevention
allocation and damage replacement candidate labels use the actual effect origin.
Trigger-order candidates carry optional ability labels. Missing fields remain
unavailable: do not infer mode/X meaning, optional instructions, numeric bounds or
legality from text/IR/card identity. Setup/combat/rule choices use their supplied
candidates and quantities; a different viewer receives no decision context.
Native source-backed metadata does not qualify live controls or complete games.

Discovery traverses only the viewer's existing published handle map and rechecks
visibility/incarnation; it never scans private IDs for order, mints handles,
republishes views, advances rules/history/cursors or submits receipts. Continuations
bind viewer and private executor/state identity; fork/restore/semantic changes
invalidate them even if a public token matches. No state digest/private identity
enters permitted output, and private object counts cannot affect public work
transcripts. Host scheduling/quotas/serialization remain outside core. Exact native
source/schema qualification and preserved failures are in [evidence](game-client-evidence.md#core-action-discovery-evidence).

## Action discovery transport

Rust DTOs/routes embed the native outcomes above. HTTP authenticates the viewer
and forwards to the exclusive match worker; it never computes rules.
`POST /v1/matches/{match_id}/actions/query` accepts `start {token,scope,page_size,work}`,
`continue {cursor,work}` or `cancel {cursor}` by `operation`; no URL query/viewer/
unknown/duplicate fields. `complete`, `incomplete`, `stale`, nested `unavailable`
and `cancelled` retain exact meanings. Partial/limited is never a complete empty menu.
An emitted beginning passes the native initiation validator; submission must still
validate and later targets/payment can fail. Action labels and decision descriptions follow the producer contract above.

| Transport resource | Bound |
| --- | --- |
| Request / result | 2,048 / 65,536 bytes; token/handle ASCII graphic strings 1–128 bytes |
| Page / work per call | 1–32 entries / 1–1,000 logical units |
| HTTP concurrency | One action query per seat; existing 64 global/16 broker bounds |
| Worker quota per seat | Fixed one-second window, 16 charged start/continue calls and 8,000 requested units; reserve full allowance |
| Scratch | One continuation per seat, four per match |
| Whole query | 32,000 reserved units, 128 calls, absolute 30-second TTL |
| Cursor | 256 random bits, lowercase hex, rotated/consumed once on continuation |
| Deadlines | Five-second body read, 25-second HTTP request; existing IPC isolation |

Quota exhaustion never changes rules/clock status. Cancel own live cursor is quota
exempt. Unknown/foreign/expired/replayed/recovered cursors are stale without private
reason. Replacement executor clears scratch even if public token stays equal;
quotas survive normal commits. Abort controls one fetch/body, not confirmed server
cancellation; inaccessible scratch expires. No durable query continuation exists.
Discovery changes no Game/head/publications/ready flags or remaining seat budget;
worker processing excludes busy time, but independent clock ticks continue.
No universal completion, constant-time or timing-side-channel guarantee follows.

SDK `queryActions` makes one validated POST, with no pagination/retry. Session binds
viewer, exact context/generation, scope, token and continuation history independently.
The browser production proxy admits the exact POST action-query route (including
cursor cancellation) and queued-command cancel route. Other routes remain blocked.

### Session-owned query controller

The sole read owner constructs immutable `ActionQueryContext` only after ownership,
manifest, actual viewer and identity checks. It carries opaque authorization/
generation identities, ReadBinding, qualified schema digest and permitted token.
Inject current context, a closure over the original authorized SDK, monotonic time,
abortable operation/wait and sanitized error classification. No credential is stored.
Synchronize synchronously before owner/token/generation/seat replacement; stop/await
cleanup before releasing ownership. Cleanup must use the captured previous SDK.

Explicit start retains original intent/scope and returns that intent's query
generation, even after supersession. Repeated same-context/scope starts are idempotent;
old-generation cancellation is ignored. Late completions cannot relabel themselves
with the latest generation. Immutable snapshots omit token/cursor/credential;
handles/scope/beginnings remain private in-memory facts, never URL/log/storage data.

Local ceilings: page 16, work 250/call, 32 charged attempts, 8,000 units, 256 aggregate
entries, 131,072 aggregate UTF-8 response bytes, absolute 20-second chain, four-second
operations, 100 ms inter-page wait. Limits may only decrease. At most two total retries
for HTTP 429/explicit quota, with one-second wait and full charged budgets. Other
transport/protocol failures retain uncertainty. At most one extra cancellation
attempt per retired intent, four seconds, no work/budget replenishment.

ReadSession captures original CommandContext/query context/scope before discovery,
associates only that returned query generation, and requires its issued draft,
complete snapshot, current generation and exact scope before normal journaling.
Pass requires a priority window and window scope. Other beginnings require
producer-owned available labels and matching source kind/object/ability; FinishMana
requires the supplied payment source and preserves its conditional reversal flag.
Never fetch a fresh command context at callback time. The pure query controller
constructs or submits no PlayerResponse. Stop invalidates synchronously and awaits
captured-SDK cancellation/in-flight deadlines before releasing the Web Lock; late
completions cannot issue I/O after release or replace a newer import.

## Native delivery and operator procedure

`GET /client-config.json` is Rust-generated public no-store origin configuration.
Authenticated presentation descriptors pin catalog/schema/producer/artifact hashes;
public `/presentation/<sha256>.json` serves immutable prevalidated allowlisted bytes.
SDK verifies bounded strict JSON, digest, catalog and face identity. A 404 is
missing metadata; corruption stops. Producer schema lives in
`packages/game-server/tools/presentation.schema.json`; request handlers only serve
startup-verified bytes. Operator pins `artifacts.json`, not arbitrary remote JSON. Product lifecycle instead obtains one authenticated
public manifest per native session and uses the SDK's public Node-only
`./presentation-producer` bridge to that same producer. It caches validated bytes
in memory. Its exact scoped artifact GET omits native bearer only behind existing
Actor ownership/active-identity checks before and after I/O; other v1 operations
retain exact human-seat authorization. No browser source parsing follows.

Native `serve-client` requires numeric loopback, exact Host, absent or matching
Origin and non-cross-site Fetch Metadata; no CORS. Vite enforces the local boundary
before forwarding to explicit `MTG_CLIENT_UPSTREAM`. Current proxy admits reviewed
read routes, submissions/execution status and exact action-query/queued-command
cancel routes, not arbitrary upstream/filesystem/operator paths. Bootstrap cannot
change this mapping.

Public responses enforce nosniff/no-referrer/frame denial and restrictive CSP:
self scripts/styles/fonts/connections; self plus stored `cards.scryfall.io` images;
no unsafe-eval/inline, base, objects, frames, forms or workers. Export allowlist:
index and single-level assets JS/CSS/WOFF2/PNG/WebP/JPEG only, no symlinks/nonregular
files/source maps/SVG/secrets. Limit 128 files, 16 MiB/file, 64 MiB total;
metadata 4 MiB and descriptor/config 4 KiB. Bundle/registry/presentation roots must
not overlap. Unknown SPA paths return 404; source route allowlists are authority.
Reject encoded/dot/backslash/absolute paths and static artifact queries.

A native owner builds/retains exact server and `provision_client` under the Cargo
lock, then exports `schema`, `bootstrap-schema`, `client-config-schema`, `native-info`.
Client/SDK commands never invoke Cargo. With qualified exports, follow client README
for explicit SDK verification and client build. From root:

```sh
sh scripts/engine-env.sh node packages/game-server/tools/client-artifacts.ts \
  packages/game-engine/tests/fixtures/expected-match-ir.json \
  /absolute/protocol.schema.json /absolute/new-presentation
sh scripts/engine-env.sh node packages/game-server/tools/export-client.ts \
  /absolute/client/dist /absolute/new-public-bundle \
  index.html assets/EXACT.js assets/EXACT.css
/absolute/provision_client /absolute/NEW_REGISTRY /absolute/NEW_PRIVATE_EXPORT_DIR http://127.0.0.1:18741
/absolute/mtg-server serve-client /absolute/NEW_REGISTRY 127.0.0.1:18741 \
  /absolute/new-public-bundle BUNDLE_SHA256 \
  /absolute/new-presentation ARTIFACT_MANIFEST_SHA256
```

Use actual exported filenames/digests and a **new 0700 private parent** for registry
and 0600 single-seat files. Provisioner starts no bots, refuses existing paths and
prints no capabilities. For existing provisioned matches use `export-seat`. Keep
original operator files private, import exactly one seat at `/join`, and reimport
after refresh. Preserve failed evidence, stop owned hosts and verify released ports.
This does not deploy an online lobby, grant accounts seats or qualify a full game.

## Reproducibility and current acceptance

Use pinned environment and local package locks; [client README](../packages/game-client/README.md)
contains exact commands. `check` verifies types, boundaries, qualified fixtures,
pure tests and production build. Browser fixture, IndexedDB, CSP and real native
App/proxy tests have distinct scopes. Authenticated tests disable traces, HAR,
video and automatic screenshots; receipts contain public identities/counts only.
Keep Chromium/Firefox versions, exact source/schema/build hashes, failed attempts
and explicit omissions. No full native matrix follows a docs/client-only edit.

Current native/SDK qualification is owned by
[`sdk-qualification.json`](../packages/game-client/sdk-qualification.json), separate
from historical capture provenance, current engine HEAD and later product changes.
Use one qualified SDK throughout checks; never replace it with historical schemas
to make fixtures pass.

[Source-qualified opening and product evidence](game-client-evidence.md) retains
exact producer/build identities, actual checks, proof locations, failures and
omissions. Read it before reusing acceptance; it is not required contract onboarding.
