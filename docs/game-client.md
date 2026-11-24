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
File. The table renders the acting seat’s decision and submits indices from its
candidate table. The decision inventory records the generated schema shapes and
the source of the opening capture.

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
SDK runtime, and its Node-only verifier checks the capture and producer hashes.

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

## Decision controls

### Core action discovery

Protocol 2 embeds a decision in observation 57 for the acting seat. Priority
beginnings and payment sources arrive in the same observation as their token.
Candidate indices are local to that decision. A seat’s object handles can
identify visible cards for presentation. A selected answer carries only indices
or exact amounts. Candidate labels in the parallel decision context provide
descriptions, including source text when the viewer may see it.

| Shape | Control and answer |
| --- | --- |
| Pick | Choose from each slot’s allowed candidate indices. Enforce its count, repeat policy and joint rules across slots. |
| Order | Select and order the allowed number of distinct listed items. |
| Distribute | Assign exact decimal quantities to the supplied buckets, satisfying their bounds, total and gates. |
| Number | Enter an exact decimal quantity within the supplied bounds. |
| Pay | Activate an offered source or finish the production step. At the spending step, allocate the exact price from the available pool. |

The controller captures the original seat, match, revision, incarnation and
token when it opens a draft. Confirmation uses that captured context through
the durable command journal. Replacing the decision invalidates its indices.
The authenticated host validates the complete answer before issuing a receipt.

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
read routes, submissions, execution status and queued-command cancellation, not arbitrary upstream/filesystem/operator paths. Bootstrap cannot
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
client reads observation 57 over protocol 2. Product launch requires a hosted browser proof
for the same consumer and native identity. The earlier Native21 proof belongs
to its [retained qualification](../packages/game/fixtures/native21-hosted-qualification.json).

[Client evidence](game-client-evidence.md) retains exact identities, checks, proof
locations, failures and omissions; read it before reusing acceptance.
