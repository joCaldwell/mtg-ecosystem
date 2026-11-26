# Game client contracts, delivery and acceptance

`packages/game/web` owns generated-protocol decision/session adapters, a qualified
captured preview and a standalone harness; `game-ui` owns shared permitted display;
`multiverse` owns product UX and Play setup; `deck-builder` keeps the
auth/Actor/SQLite backend. Rust host/core own admission, legality, visibility and
durable execution. Product sign-in does not grant a Rust seat. See
[application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch)
and [client setup](../packages/game/README.md).

## Delivered scope and remaining gates

The standalone App imports an operator-issued single-seat file; product
`@multiverse/game/session` uses the same router-free table with an Actor-authorized
File. The table renders the acting seat's decision and submits indices from its
candidate table.

One exact foundations release passed [hosted completion](#hosted-completion).
Launch requires deck admission, private provisioning and three owned automated
seats; unsupported automation pauses. The owner-scoped record confirms pauses and
results after shutdown; a retained table may be read-only while the host resolves
a pending command. The product preview uses a captured sample and local choices.

## Package, privacy and presentation boundaries

[The package manual](../packages/game/AGENTS.md#boundaries) owns layering. The
router-free preview imports no session/auth/SDK runtime; its Node-only verifier
checks capture and producer hashes.

Core-observed current characteristics override printed reference values. Metadata
is offline-produced from exact archived source with catalog/printing/face/source
hashes; compiler source-to-ability labels are a separate producer gate. No Oracle
heuristics, card-name dispatch, image API lookup or turn-time source fetch.
Exact stored Scryfall printings illustrate the captured preview and public
battlefield, command and stack cards, without referrer. Private live cards and
unavailable printings use source-backed text and observed characteristics. Invalid
metadata hash/schema is a blocking protocol error, never fallback success. Never
fabricate source labels, X meaning, mode labels, payment intent or clocks from wire
syntax. No service worker/private cache, URL secrets, telemetry of private
projections, trusted snapshots, seeds, receipts or other seats' history.

## Authorization, reads and ownership

Import exactly one generated-schema-validated bootstrap through the file picker. It
must match the configured canonical numeric loopback origin, resume viewer, match,
protocol, engine/server builds and catalog. Capability stays only in memory;
refresh requires reimport. Never derive identity from URL labels, let an imported
file select a proxy target, or persist bearer tokens.

Acquire the exclusive Web Lock before every session network/journal operation; no
forced takeover, secret broadcasting or fallback ownership. The single read owner
controls cursor and recovery and keeps the last Observation/token across
lifecycle-only pages. A changed generation/seat/
identity invalidates all asynchronous completions, drafts and effects before I/O.
Reads use bounded backoff; a 429 may extend the wait by an authorization-local
`Retry-After` hint capped at 60 seconds and aborted on stop. Only a successful
stream read after cooldown renews the rate-limit retry budget. Failed or partial
bodies stay uncertain. Recovery verifies manifest and metadata, keeping completed
stages across hinted 429s within one authorization.

## Durable command journal

Capture the **original complete CommandContext** when a draft starts: binding
(origin/match/seat/engine/server/catalog/protocol), incarnation, revision and token.
Human completion carries that context; never stamp an old selection with a fresh
context after refresh, rejection or decision replacement. Duplicate/stale/unbound
intents have no effects. UI draft cancellation is not server cancellation, Undo,
concession or a new command.

SDK `prepareCommand` creates a detached frozen validated envelope/body pair;
`parsePreparedCommand` recovers strict original bytes; `submitExact` sends once.
Persist `{version,binding,exactBody}` (original serialization) in IndexedDB; only
transaction **complete** with strict durability authorizes send. No credential, full
observation or redundant envelope is stored.

Database `multiverse-command-journal` version 1 has one `commands` store keyed by
JSON tuple `[server origin, match, seat]`, deliberately excluding build/catalog:
scan that seat before current-build matching so incompatible unresolved work blocks
replacement. Atomic insertion checks key and total count across tabs. Limits: 64
unresolved records, 65,536 UTF-8 bytes per command (4 MiB body budget plus bounded
overhead), 512-character binding fields, three-second open/transaction timeout.
Denied/blocked/quota/corrupt/unsupported storage fails closed and never resets or
evicts unresolved work. Late opens close.

| Authoritative disposition | Journal behavior |
| --- | --- |
| Submission accepted / execution pending | Retain unresolved slot |
| Execution consumed_by_rules | Resolve consumed |
| Execution discarded_by_departure | Resolve departure-discarded |
| Execution cancelled / definitive cancelled rejection | Resolve cancelled |
| Other definitive rejection | Resolve rejected with public enum |
| Missing status / abort / timeout / transport or decode failure | Retain uncertainty; only exact original replay is permitted |

Cancellation concerns a queued server command; an accepted cancellation response
may repeat earlier acceptance and does not by itself clear uncertainty. Terminal
deletion compares the entire original binding/body transactionally. Before new
intent, obtain a coherent resume started after resolution; a failed refresh keeps
the gate. Advance controller incarnation even if token/revision are equal. Every
completion/effect is fenced, including synchronous subscriber cleanup during
persistence/send/rejection and stop/StrictMode/seat replacement.

Each command authorization gets four automatic status/replay opportunities, driven
only by successful reads (which do not renew the budget); afterwards explicit retry
checks status/replays the same body once. Refresh scans rather than manufacturing
replacement. Browser command effects have a 30-second headers/body deadline. Local
stop, missing rows, changed view or incompatible build are not terminal and never
erase ambiguity. The SDK itself has no journal/retry policy.

## Decision controls

### Core action discovery

Protocol 2 embeds a decision in observation 80 for the acting seat; priority
beginnings and payment sources arrive in the same observation as their token.
Candidate indices are local to that decision; a seat's object handles can identify
visible cards. An answer carries only indices or exact amounts. Candidate labels
in the parallel decision context describe candidates, including source text the
viewer may see.

| Shape | Control and answer |
| --- | --- |
| Pick | Choose from each slot's allowed candidate indices. Enforce its count, repeat policy and joint rules across slots. |
| Order | Select and order the allowed number of distinct listed items. |
| Distribute | Assign exact decimal quantities to the supplied buckets, satisfying their bounds, total and gates. |
| Number | Enter an exact decimal quantity within the supplied bounds. |
| Pay | Activate an offered source or finish the production step. At the spending step, allocate the exact price from the available pool. |

Confirmation goes through the durable command journal with the captured context;
replacing the decision invalidates its indices. The host validates the complete answer.

## Native delivery and operator procedure

`GET /client-config.json` is Rust-generated public no-store origin configuration.
Authenticated presentation descriptors pin catalog/schema/producer/artifact hashes;
public `/presentation/<sha256>.json` serves immutable prevalidated bytes (schema:
`packages/game-server/tools/presentation.schema.json`). The SDK verifies bounded
strict JSON, digest, catalog and face identity; 404 is missing metadata, corruption
stops. The operator pins `artifacts.json`. Product lifecycle instead caches the
SDK's Node-only `./presentation-producer` output per native session; its scoped
artifact GET omits the native bearer only behind Actor ownership and identity
checks before and after I/O.

Native `serve-client` requires numeric loopback, exact Host, absent or matching
Origin and non-cross-site Fetch Metadata; no CORS. Vite enforces the same boundary
before forwarding to explicit `MTG_CLIENT_UPSTREAM`. The proxy admits reviewed read
routes, submissions, execution status and queued-command cancellation only;
bootstrap cannot change this mapping.

Public responses enforce nosniff/no-referrer/frame denial and restrictive CSP:
self scripts/styles/fonts/connections, self plus stored `cards.scryfall.io` images;
no unsafe-eval/inline, base, objects, frames, forms or workers. Export allowlist:
index and single-level assets JS/CSS/WOFF2/PNG/WebP/JPEG only; no symlinks,
nonregular files, source maps, SVG or secrets. Limits: 128 files, 16 MiB/file,
64 MiB total, metadata 4 MiB, descriptor/config 4 KiB. Bundle/registry/presentation
roots must not overlap. Unknown SPA paths 404; reject encoded/dot/backslash/absolute
paths and static artifact queries.

A native owner builds exact server and `provision_client` under its target's Cargo
lock, then exports `schema`, `bootstrap-schema`, `client-config-schema`,
`native-info`. Client/SDK commands never invoke Cargo. From root:

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

Use actual exported filenames/digests and a **new 0700 private parent** for the
registry and 0600 single-seat files. The provisioner starts no bots, refuses
existing paths and prints no capabilities; existing matches use `export-seat`.
Stop owned hosts and verify released ports. This qualifies no lobby, account seat
or full game.

## Reproducibility and current acceptance

The [client README](../packages/game/README.md) owns commands. Authenticated tests
disable traces, HAR, video and automatic screenshots; receipts hold public
identities/counts, browser versions, exact source/schema/build hashes, failed
attempts and omissions.

[`sdk-qualification.json`](../packages/game/sdk-qualification.json) owns current
native build and schema pins. Product launch requires a hosted browser proof for
the same consumer and native identity. Evidence binds exact producer, source and
schema, not moving HEAD; later hosts, producers or consumers need fresh proof.
Read the records below before reusing acceptance.

### Active baselines

| Baseline | Exact proof root |
| --- | --- |
| Observation 21 native producer `2305e2f8` | [Engine evidence](game-engine-evidence.md#combined-client-native-producer-observation-21) |
| Native21 hosted qualification | [Retained record](../packages/game/fixtures/native21-hosted-qualification.json) |
| Metadata-delivery SDK bridge (SDK 69 extends 66; observation 21 builds and schema unchanged) | `~/.local/share/mtg-ecosystem/engine-evidence/product-presentation-sdk-20261115/` |
| Application boundary extraction `de785a97` (relocation bridge and Native21 bootstrap/proxy/retry/lease witness) | `~/.local/share/mtg-ecosystem/engine-evidence/game-application-boundary-20261116/` |
| Core action discovery, native source `d1d039539d63aa17cd411c654e22086ccf3c9322` | `generated/core-queries/attempt02/`; [immutable record](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/game-client-core-queries.md#executed-checks-and-retained-evidence) owns commands, hashes and failures; its schema is not SDK input |

### Hosted completion

Chromium `774cad38` finished a foundations game in 38.9 minutes under 240 Actor
requests/minute with zero 429s, 935 consumed/reconciled commands and three joined
bot leases. The visible owner win matches native ledger outcomes for all four
seats; the browser did not directly observe the native outcome. Only the tested
release identity is approved. Evidence:
`product-shared-budget-completion.potzlXV6/completion-evidence.json`
SHA-256 `ec06be154b07eef380499fba4319a06fe179694d233436657ae4549d2438f814`.

### Historical records

Earlier producer-context, opening/recovery, observation 16 lifecycle, shared
display, live-discovery, frontend and hosted-prefix records keep their identities,
failures and proof roots in the
[immutable client evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/13704bf5781362a4fcf51848dc8a2f3e398fd87b/docs/game-client-evidence.md).
The [immutable transport/delivery reports](https://github.com/joCaldwell/mtg-ecosystem/tree/1c3eca4c37b90de02decca316fefab5c8035140e/docs)
and [package reports](https://github.com/joCaldwell/mtg-ecosystem/tree/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client)
keep earlier prerequisites. Raw evidence stays preserved.
