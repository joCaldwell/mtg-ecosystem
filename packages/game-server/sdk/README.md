# Game protocol TypeScript SDK

Generated transport for the Rust protocol. Rust public DTOs and
[`protocol::ROUTES`](../src/protocol.rs) own exact schemas, methods and paths;
[`src/client.ts`](src/client.ts) owns exported types/methods. The package root and
`/browser` export production ESM/declarations from `dist/`, never source/examples.
The SDK implements action queries; production App composition remains a separate
[client gate](../../../docs/game-client.md#delivered-scope-and-remaining-gates).

## Build and verify

Use pinned Node through `sh scripts/engine-env.sh` from repository root. This
nested package is outside root workspaces and owns its exact lock/local install.
Never overwrite shared root dependencies. The native owner builds/retains a
compatible executable separately; no SDK command invokes Cargo.

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_BIN=/absolute/mtg-server \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
```

Alternatively supply both `MTG_SERVER_SCHEMA` and `MTG_SERVER_BOOTSTRAP_SCHEMA`
with source-qualified exported files. Executable mode calls `schema` and
`bootstrap-schema`; its default is `../../../target/debug/mtg-server` relative to
this package. `MTG_SERVER_OUTPUT` is an isolated generator-test override, while
verification uses normal ignored `generated/` so all checks consume one output.

`build:browser` explicitly generates declarations, route data and standalone Ajv
validators, then bundles static helpers. There is no runtime schema compilation,
external import, Node global, JSON import, React, policy or private fixture in the
ESM. Preserve CSP without unsafe-eval. Installation has no generation hook;
browser generation/build is explicit. No SDK command invokes Cargo or implicitly
downloads browsers.
The generator rejects external references, future protocol envelopes, non-Draft7
tuples, unconstrained types and malformed/external/traversal route descriptors;
only complete output publishes, with failed temporary output removed.

`verify` repeats browser build, strict types, exhaustive response/tuple consumers
and transport/generator tests, then writes `generated/verification.{json,log}`.
`generated/generation.json` pins schemas/declarations/validators, generator inputs,
options and lock; `dist/build.json` pins input allowlist, exports and distribution
bytes. Verification records commands, actual tools/counts and source/log hashes.
Keep native provenance separately: matching schemas do not prove saved-state
compatibility. Changed native exports or SDK source need explicit regeneration.

## Read a seat

```ts
import { GameClient } from '@multiverse/game-server-sdk';
const client = new GameClient({ baseURL, matchId, capability });
const manifest = await client.manifest();
const resumed = await client.resume();
const page = await client.poll({ after: '0', limit: 32, waitMs: 20_000 });
```

Inject `fetch` only when a host/test adapter deliberately supplies it. Native
fetch is bound to its receiver, omits cookies, rejects redirects and allows HTTPS
or loopback HTTP without URL credentials. Capabilities stay in private fields and
Authorization headers, never errors/URLs/logs. The SDK receives permitted data,
not raw State, snapshots, receipts, private profile digests or other seats' history.

`parseSeatBootstrap(text)` validates strict Rust-schema JSON, a 4096-byte bound and
canonical numeric loopback origin, including explicit port 80. It returns sensitive
in-memory data; the session must explicitly import it, verify configured upstream,
actual viewer and build/catalog identity. It selects no proxy and persists nothing.

`manifest()` correlates match identity and supplies catalog, four open decklists
and public build/profile metadata. `resume()` is one authenticated read bounded at
131,072 bytes (or a smaller configured limit), correlating match/available viewer.
Pending/available resume state, nullable last lifecycle and actual rules outcome
are distinct. Session authorization and engine admission remain outside transport.

Poll cursors are canonical u64 decimal strings. Pages strictly advance, with `next`
equal to the last included cursor or the supplied cursor when empty. Commands use
the latest own-stream cursor, including lifecycle entries; retain the latest
Observation/token across pages without a replacement view. `events()` advances
only yielded entries: persist the last processed cursor and reconnect explicitly.

## Submit and recover

`prepareCommand(envelope)` returns a detached, deeply frozen validated envelope
and `exactBody`. `parsePreparedCommand(exactBody)` validates recovered bytes without
reserializing; `submitExact(exactBody)` validates and sends those exact bytes once,
checking returned command ID. `submit(envelope)` prepares then submits. The SDK
has no persistence, journal, pagination retry or automatic command retry policy.
An interrupted submission may already be durable; callers may retry only the
original body/ID under the host's idempotency contract. Transport abort does not
cancel an accepted command. Follow [client journaling](../../../docs/game-client.md#durable-command-journal)
for IndexedDB durability, ownership and uncertainty resolution.

`execution(commandId)` distinguishes pending, consumed_by_rules,
discarded_by_departure, cancelled and typed rejection. `cancel(commandId)` is a
bodyless POST for a still-queued command, never an undo. Both require a canonical
32-character lowercase hexadecimal ID and correlate the response. Immutable
submission acceptance can coexist with a later departure/cancellation disposition;
inspect execution status rather than assuming an accepted cancellation consumed
rules input. Typed win/draw and resource-paused/faulted host status stay distinct.

Responses are bounded during reads: 8 MiB default, configurable to 32 MiB, with
stricter method bounds. Early rejection cancels the stream. Strict raw JSON rejects
duplicate decoded keys, nesting beyond 128, invalid/lone Unicode scalars and
noncanonical structural numerics, including fractions, exponents and -0. Schema
validation never coerces, defaults or strips fields. Core quantities remain exact
canonical decimal strings; numeric fields outside JavaScript's exact range reject.
Protocol validation is not gameplay legality or seat authorization.

## Origin, presentation and discovery

`readClientConfig(baseURL)` and `parseClientConfig(text)` validate the public Rust
origin configuration, 4096-byte limit and canonical numeric loopback upstream.
The session compares it with bootstrap and explicit proxy settings; imported
credentials cannot select upstreams.

`presentation()` reads an authenticated descriptor then immutable same-origin
JSON without authorization, returning `{descriptor, artifact}`. Descriptor limit
is 4096 bytes; artifact 4 MiB or the caller's smaller bound. Producer/schema/hash,
catalog, face ordering and identity failures are `ProtocolError`; missing data
stays `HttpError(404)` for session display fallback. No retry or local inference.
Producer contract is [`presentation.schema.json`](../tools/presentation.schema.json).

`queryActions(request, {signal})` performs one bounded start/continue/cancel POST
using generated request/response types. Caller owns token/viewer/scope/cursor
binding, continuation, expiry and retries. Incomplete never means an empty legal
menu; cursors can stale without a changed public token. Labels/missing contexts
remain unavailable. Native, transport and session limits are separately defined in
[the discovery contract](../../../docs/game-client.md#core-action-discovery).

## Browser proofs

After explicit build, use the locked browsers; downloads are separate setup:

```sh
sh scripts/engine-env.sh node packages/game-server/sdk/node_modules/playwright/cli.js install chromium firefox
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run test:browser --workspaces=false
```

The synthetic loopback harness exercises production ESM under restrictive CSP
with no inline scripts/unsafe-eval, proving validators, exact numbers/cursors,
raw JSON failures, bounded streams, native fetch/abort, Web Locks and IndexedDB.
It closes owned browsers/server and retains versions/hashes in create-new
`generated/browser-proof-*.json`. This is not real Rust hosting/seat isolation.

Read-only real-host proof requires an independently running compatible host kept
at opening. Pass only individual private seat files, never issued registry files:

```sh
SDK_NATIVE_COMMIT=<full-source-SHA> SDK_NATIVE_INFO=/absolute/native-info.json \
SDK_NATIVE_BIN=/absolute/mtg-server SDK_BROWSER_HOST_ORIGIN=http://127.0.0.1:18741 \
sh scripts/engine-env.sh node packages/game-server/sdk/scripts/test-browser-host.mjs /private/seat0.json /private/seat1.json
```

It uses explicit picker import and an allowlisted same-origin test proxy, sends no
commands and prints no credentials/observations. Manifest/resume/stream identities,
isolated Chromium/Firefox contexts and no-store behavior are checked in scrubbed
`generated/browser-host-proof-*.json`; no completed-game/crash-recovery claim follows.
[Client evidence](../../../docs/game-client-evidence.md) owns source-qualified results.

## Finite completion consumers

Run one process per seat with bounded private stdin, never capability argv/env/logs:

```sh
sh scripts/engine-env.sh node packages/game-server/sdk/src/examples/foundations-consumer.mjs < /private/seat-0-bootstrap.json
```

The stdin shape is `{baseURL, matchId, capability, viewer, outputPath?}`; viewer is
0–3. Optional output is create-new mode 0600; stdout is scrubbed identity/count/
outcome or explicit nonterminal-stop data. These are finite completion examples,
not general bots or browser controls:

| Consumer / pure policy | Supported purpose |
| --- | --- |
| `foundations-consumer.mjs` | Keep/pass, empty combat, offered discard and mandatory Commander returns; natural decking witness, no lands/casts/activations |
| `commander-consumer.mjs` | Admitted all-Isamaru pod's land/commander/payment/attack sequence; no optional hand spells |
| `attachments-consumer.mjs` | Admitted attachments pod, public-state mana planning/casts/equip/Auras/combat and offered private search handles; one policy instance per seat/game |

Consumers pin/recheck public manifest/catalog, drain pages and preserve observation
across lifecycle entries. Unsupported requests stop. Shared retry loop retains the
original envelope/ID on unknown transport outcomes, 429/503 only: at most 24 attempts
and 8 minutes per operation within 25 minutes overall, at most 5,000 accepted
inputs/seat (20,000 total). Stale revision stops instead of minting a new ID.
Accepted counts mean immutable acknowledgments, not resolution. In-memory plans,
envelopes/cursors have no process-crash resume guarantee. Server recovery with
living consumers uses exact retries/stream reads. Resource/deadline/input stops
never become rules outcomes. Synthetic multi-process tests and historical admitted
four-client host proofs retain separate [host qualification](../../../docs/game-engine-server.md#hosted-profiles).
