# Game client contracts, delivery and acceptance

`game-client` owns generated-protocol decision/session adapters, a qualified
captured preview and a standalone React/Vite harness. `game-ui` owns shared
permitted display with scoped styles and host React. `multiverse` owns product
account UX, navigation, Home, deck editor and Play setup; `deck-builder` retains
the existing auth/Actor/SQLite backend. Rust host/core own admission, legality,
visibility and durable execution. Product sign-in does not grant a Rust seat.
Read [application launch gates](multiverse-app.md) and [client setup](../packages/game-client/README.md).

## Delivered scope and remaining gates

The production App joins through an operator-issued single-seat file and enables
only explicit `mulligan` and `bottom_opening` commands. It journals and reconciles
unknown outcomes. Other decisions explain missing live controls/context. Nine
request shapes have local draft adapters; they do not establish live or whole-profile
coverage. `fixtures/decision-controls/coverage.json` inventories 28 requests and
33 responses with separate reachability, control, context and test evidence.
Review new variants explicitly; schema presence is never admission.

Action transport exists; the pure action-query lifecycle controller is opt-in
and remains uncomposed in the production App. Discovery labels are unavailable.
Complete games, all reachable decisions, human usability, arbitrary builder-deck
admission, automated launch and account-to-seat provisioning remain unfinished.
The product preview is a captured sample, never the selected deck or a real game.
Do not use legacy solo playtester briefs/components as guidance.

## Package, privacy and presentation boundaries

- `app` composes routing/lifetime; components never fetch or access storage.
- `session` owns SDK effects, authorization, exclusive ownership and recovery.
  Pure controllers inject effects; no browser/React/SDK runtime dependency.
- `decisions` owns local human drafts: syntax, supplied candidates, ordering and
  context binding. It computes no eligibility, costs, payment or combat rules.
- `presentation` maps permitted projections and producer-owned allowlisted public
  metadata to display. Never inspect executable IR, snapshots or private fixtures.
- `features` composes public `@multiverse/game-ui` display. No duplicate table.
- `preview` is router-free and local-only; it imports no auth/session/SDK runtime.
  Its public component types hide generated inputs; implementation parity remains
  checked in the client. `verify-preview` is Node-only provenance verification.

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

## Action discovery transport

[Core action beginnings](game-client-core-queries.md) defines only priority/mana
initiation completeness. Rust DTOs/routes embed core outcomes; HTTP authenticates
the viewer and forwards to the exclusive match worker. It never computes rules.
`POST /v1/matches/{match_id}/actions/query` accepts `start {token,scope,page_size,work}`,
`continue {cursor,work}` or `cancel {cursor}` by `operation`; no URL query/viewer/
unknown/duplicate fields. `complete`, `incomplete`, `stale`, nested `unavailable`
and `cancelled` retain exact meanings. Partial/limited is never a complete empty menu.
An emitted beginning passes the native initiation validator; submission must still
validate and later targets/payment can fail. Every current ActionLabel is unavailable.

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
The browser production proxy currently blocks action queries/cancellation; do not
infer App integration from native endpoint/SDK availability.

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
operations, 100ms inter-page wait. Limits may only decrease. At most two total retries
for HTTP429/explicit quota, with one-second wait and full charged budgets. Other
transport/protocol failures retain uncertainty. At most one extra cancellation
attempt per retired intent, four seconds, no work/budget replenishment.

Future menu composition must retain original CommandContext/query context/scope,
associate only the returned query generation, verify current snapshot phase and
binding, then use normal validation/journaling. Never fetch a fresh command context
at callback time. Missing labels/context stay blocked; this controller constructs
or submits no PlayerResponse.

## Native delivery and operator procedure

`GET /client-config.json` is Rust-generated public no-store origin configuration.
Authenticated presentation descriptors pin catalog/schema/producer/artifact hashes;
public `/presentation/<sha256>.json` serves immutable prevalidated allowlisted bytes.
SDK verifies bounded strict JSON, digest, catalog and face identity. A 404 is
missing metadata; corruption stops. Producer schema lives in
`packages/game-server/tools/presentation.schema.json`; request handlers only serve
startup-verified bytes. Operator pins `artifacts.json`, not arbitrary remote JSON.

Native `serve-client` requires numeric loopback, exact Host, absent or matching
Origin and non-cross-site Fetch Metadata; no CORS. Vite enforces the local boundary
before forwarding to explicit `MTG_CLIENT_UPSTREAM`. Current proxy admits reviewed
read routes, opening submissions and execution status, not arbitrary upstream/
filesystem/query/cancel/operator paths. Bootstrap cannot change this mapping.

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

Qualification authority is `packages/game-client/sdk-qualification.json` plus
fixture provenance; historical captures are unchanged. Current native/SDK pin is
`ebfa6dde266139c45220959c4179ddedf6a38433`, preserving compiler fix `dc12222` and
reviewed PR22/23. Protocol schema:
`a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`;
bootstrap schema:
`0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
This is separate from engine HEAD, original `2aaf`/`e86` captures and the later
PR24 product extraction. Never switch SDK builds between qualification checks.

[Source-qualified opening and product evidence](game-client-evidence.md) retains
exact producer/build identities, actual checks, proof locations, failures and
omissions. Read it before reusing acceptance; it is not required contract onboarding.
