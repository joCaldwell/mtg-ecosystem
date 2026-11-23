# Game client contracts, delivery and acceptance

`packages/game/web` owns generated-protocol decision/session adapters, a qualified
captured preview and a standalone harness; `game-ui` owns shared permitted display;
`multiverse` owns product UX and Play setup; `deck-builder` retains the
auth/Actor/SQLite backend. Rust host/core own admission, legality, visibility and
durable execution. Product sign-in does not grant a Rust seat.
Read [application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch) and [client setup](../packages/game/README.md).

## Delivered scope and remaining gates

The standalone App imports an operator-issued single-seat file. Product
`@multiverse/game/session` uses the same router-free table with an Actor-authorized
File. Non-query decisions use local drafts, with producer context required for
source-dependent choices. The [decision inventory](../packages/game/fixtures/decision-controls/coverage.json)
pins request and response schema shapes, reviewed control mappings and capture
provenance. Its read-only check verifies the request cases in the web decision
switches. Priority and mana windows use bounded [action discovery](#core-action-discovery).

One exact foundations release passed [hosted completion](game-client-evidence.md#hosted-completion).
Launch requires deck admission, private provisioning and three owned automated
seats; unsupported automation pauses. The owner-scoped record confirms pauses and
results after shutdown, and a retained table may be read-only while the host
resolves any pending command.

The product preview uses a captured sample and local choices; the solo
playtester is outside this client.

## Package, privacy and presentation boundaries

[The package manual](../packages/game/AGENTS.md#boundaries) owns layering:
session effects are separate from pure local drafts and permitted presentation;
shared display comes from game-ui. The router-free preview imports no session/auth/
SDK runtime, and its Node-only verifier preserves historical fixture qualification.

Core-observed current characteristics override printed reference values. Metadata
is offline-produced from exact archived source with catalog/printing/face/source
hashes; compiler source-to-ability labels remain a separate producer gate. No
Oracle heuristics, card-name dispatch, image API lookup or turn-time source fetch.
Display uses exact stored Scryfall printings for the captured public preview and
public battlefield, command and stack cards. Images omit referrer information.
Private live cards and unavailable or failed printings use source-backed text and
observed characteristics. Invalid metadata
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
Reads use bounded backoff/retry; 429 responses may extend the wait using an
authorization-local `Retry-After` hint, capped at 60 seconds and aborted on stop.
Only a successful stream read after cooldown renews the rate-limit retry budget.
Hintless collisions stay bounded. Failed or partial bodies remain uncertain.
Recovery verifies manifest and metadata, retaining completed stages across hinted
429s within the same authorization.

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
`Priority` and `Mana` initiation; other requests return `unsupported_window`. The
host authorizes the viewer; core has no HTTP/auth/clock/database dependency.
`ActionScope` is `Window` or one permitted `Object { object: ObjectHandle }`.
Emitted beginnings pass the existing response validator; submission validates
again. Cast is not affordability, and initiation completeness covers no later
target/payment/X choice or strategy ranking.

`ActionQueryResult::into_parts()` returns the permitted outcome plus an optional
opaque `ActionContinuation` that the host keeps behind its authenticated cursor
for `resume_actions`. Dropping it discards scratch, never a command or durable
record; restart restarts discovery.

| Native outcome | Meaning |
| --- | --- |
| `complete { page }` | Scope exhausted; combine all preceding pages |
| `incomplete { page, required_minimum: 1 }` | Capacity stopped discovery; continuation exists |
| `stale` | Viewer/token/executor/state binding no longer matches; no private reason |
| `unavailable { reason }` | Invalid boundary, viewer, page size or scope, or invariant failure; no partial menu |

Pages never repeat entries; a full page is still incomplete. Native pages hold
1–64 entries; budgets count logical work, not CPU time.

Context is `priority` or `mana { cost, source }`. `ActionLabel` is `unavailable`
or `available { text, source? }`: core owns wording from offline compiler
bindings, and activations without a bound source label stay unavailable. Labels
grant no affordability, target, payment or submission permission.

Optional `Observation.decision_context.source` supplies kind, Oracle identity,
optional object handle/ability index and exact source text from compiler
`SourceLabels`. Missing fields remain unavailable: never infer mode/X meaning,
bounds or legality from text/IR/card identity. Other viewers receive no decision
context. Metadata never admits new mechanics or qualifies live controls.

Discovery traverses only the viewer's published handles; it never mints handles,
advances rules/history/cursors or submits receipts. Continuations bind viewer and
private executor/state identity, so fork/restore invalidates them even if the
public token matches. No private identity affects permitted output. Qualification is in
[evidence](game-client-evidence.md#core-action-discovery-evidence).

## Action discovery transport

HTTP authenticates and forwards to the exclusive match worker without computing
rules. `POST /v1/matches/{match_id}/actions/query` accepts `start`,
`continue` or `cancel` by `operation`, with no URL query, viewer or unknown fields.
Outcomes keep the native meanings plus `cancelled`; partial is never a complete
empty menu.

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

Quota exhaustion never changes rules/clock status. Unknown, foreign, expired or
replayed cursors are stale without a private reason. Abort controls one fetch, not
server cancellation; no durable continuation exists. Discovery changes no game
state or seat budget, with no timing-side-channel guarantee. SDK `queryActions`
makes one validated POST with no pagination/retry.

### Session-owned query controller

The sole read owner builds immutable `ActionQueryContext` only after ownership,
manifest, viewer and identity checks; it stores no credential. Replacing owner,
token, generation or seat invalidates synchronously; cleanup uses the captured
previous SDK, and late completions issue no I/O after the Web Lock is released.
Handles, scope and beginnings stay in memory, never in URLs, logs or storage.
Local ceilings only tighten transport bounds; retries never replenish budget.

ReadSession captures the original CommandContext, query context and scope before
discovery, and journals a beginning only with its issued draft, complete snapshot,
current generation and exact scope. Pass requires a priority window; other
beginnings require available labels matching source kind/object/ability. Never
fetch a fresh command context at callback time; the query controller constructs or
submits no PlayerResponse.

## Native delivery and operator procedure

`GET /client-config.json` is Rust-generated public no-store origin configuration.
Authenticated presentation descriptors pin catalog/schema/producer/artifact hashes;
public `/presentation/<sha256>.json` serves immutable prevalidated allowlisted bytes.
SDK verifies bounded strict JSON, digest, catalog and face identity. A 404 is
missing metadata; corruption stops. Producer schema lives in
`packages/game-server/tools/presentation.schema.json`; request handlers only serve
startup-verified bytes. Operator pins `artifacts.json`, not arbitrary remote JSON.
Product lifecycle instead fetches one authenticated public manifest per native
session and caches the SDK's Node-only `./presentation-producer` output in memory.
Its scoped artifact GET omits the native bearer only behind Actor ownership and
active-identity checks before and after I/O; other v1 operations retain exact
human-seat authorization. No browser source parsing follows.

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

A native owner builds exact server and `provision_client` under its target's Cargo lock,
then exports `schema`, `bootstrap-schema`, `client-config-schema`, `native-info`.
Client/SDK commands never invoke Cargo; the client README owns SDK verification
and client build. From root:

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
prints no capabilities; existing matches use `export-seat`. Keep operator files
private, import exactly one seat at `/join`, and reimport after refresh. Preserve failed evidence, stop owned hosts and verify released ports.
This does not deploy an online lobby, grant accounts seats or qualify a full game.

## Reproducibility and current acceptance

Use pinned environment and local package locks; [client README](../packages/game/README.md)
contains exact commands and the distinct scopes of `check` and browser tests. Authenticated tests disable traces, HAR,
video and automatic screenshots; receipts contain public identities/counts only.
Keep Chromium/Firefox versions, exact source/schema/build hashes, failed attempts
and explicit omissions. No full native matrix follows a docs/client-only edit.

[`sdk-qualification.json`](../packages/game/sdk-qualification.json) owns current
native build and schema pins for SDK generation and local decision checks. The
R4 client reads observation 56. Product launch requires a hosted browser proof
for the same consumer and native identity. The earlier Native21 proof belongs
to its [retained qualification](../packages/game/fixtures/native21-hosted-qualification.json).

[Client evidence](game-client-evidence.md) retains exact identities, checks, proof
locations, failures and omissions; read it before reusing acceptance.
