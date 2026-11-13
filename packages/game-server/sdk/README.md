# Game protocol TypeScript SDK

This is the generated client for the Rust protocol in `../src/protocol.rs`.
The schemas and declarations are generated from `mtg-server schema`; they are
ignored artifacts. The HTTP client has schema, mock-transport and real local
server checks. Complete admitted four-player integration is tracked separately
in [the server contract](../../../docs/game-engine-server.md). Production
authentication and deployment are outside this local release.

## Browser package

Build explicitly before adding `file:../game-server/sdk` to a consumer's own
isolated dependencies. No install hook, check or build invokes Cargo. The Rust
owner supplies the compatible executable or exact exported schema file:

```sh
MTG_SERVER_SCHEMA=/absolute/source-qualified/protocol.schema.json MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/source-qualified/bootstrap.schema.json npm --prefix packages/game-server/sdk run build:browser --workspaces=false
```

Both `@multiverse/game-server-sdk` and `@multiverse/game-server-sdk/browser` export
`GameClient`, `HttpError`, `TransportError`, `ProtocolError`, `counter` and
`PROTOCOL_VERSION`, `parseSeatBootstrap`, `parseClientConfig` and `readClientConfig`. Exact generated type exports include `Observation`,
`PlayerRequest`, `PlayerResponse`, `CommandEnvelope`, `StreamPage`, `PublicManifest`,
`SeatResume`, `ResumeState`, `SeatBootstrap`, `ClientConfig`,
`PresentationDescriptor`, and producer-owned `PresentationArtifact`
and the existing types listed in `src/client.ts`; `ClientOptions` and `PollOptions`
are transport configuration. They resolve to `dist/browser.js` and
`dist/browser.d.ts`. Source/tests/examples are not package exports.

```ts
import { GameClient, type Observation, type PlayerRequest } from '@multiverse/game-server-sdk';
const sdk = new GameClient({ baseURL: location.origin, matchId, capability, fetch: fixtureFetch });
// Omit fetch to use real browser HTTP. Fixtures inject typeof globalThis.fetch
// returning native Response/ReadableStream; no retry or gameplay policy is added.
```

The build generates standalone Ajv validators and bundles their static helpers,
including all four custom numeric formats. Ajv compilation runs only at build
time. The browser ESM has no external imports, Node globals, runtime JSON imports,
React, example policies or private fixtures. Strict raw JSON decoding, response
correlation, byte limits and decimal exactness remain in the shared transport.
The browser default fetch is bound to its global receiver. No automatic retries
are introduced. The eight generated routes include native `get_resume`, `get_presentation`,
and public `get_client_config`, pinned to the source-qualified exports. Action/context operations are not implemented.

`generated/generation.json` pins schema bytes, declarations, standalone validators,
generator inputs/options and lockfile. `dist/build.json` pins generated input,
runtime input allowlist, exports and all distribution files. Generated artifacts
remain ignored. A changed source/export requires explicit regeneration. Preserve
the native source/build provenance alongside these digests; a matching schema
alone does not prove that a host binary is compatible with saved match state.

```sh
# Explicit one-time download of the locked automation browsers; never part of a check.
node packages/game-server/sdk/node_modules/playwright/cli.js install chromium firefox
# After build:browser; no Cargo, no implicit download, no external API.
npm --prefix packages/game-server/sdk run test:browser --workspaces=false
```

The small loopback harness serves production ESM with `default-src 'none'`,
`script-src 'self'`, `connect-src 'self'`, no inline scripts or `unsafe-eval`.
Its external module proves CSP enforcement with a deliberately blocked dynamic
function, then exercises validators, exact quantities/cursors, raw JSON negatives,
bounded/cancelled streams, native fetch/abort, Web Locks and IndexedDB in Chromium
and Firefox. The runner closes browsers/server and records exact versions and
hashes in a new `generated/browser-proof-*.json`. Fixture HTTP is explicitly
synthetic; it does not establish real Rust hosting, seat isolation or human play.
See [the browser SDK report](../../../docs/game-client-browser-sdk.md) for source
qualification, actual evidence and remaining native integration gates.

`parseSeatBootstrap(text)` validates the separately exported Rust `SeatBootstrap`
schema plus the 4096-byte file bound, strict raw JSON and canonical numeric
loopback origin (including explicit port 80). It returns sensitive data for
in-memory session use only. The caller must explicitly import the file, verify
the allowlisted proxy mapping and compare viewer/build/catalog identity; this
function does not select an upstream or persist credentials.

`resume({ signal? })` performs one authenticated read with a 131072-byte bound
(or the caller's smaller configured bound). It validates and correlates match
identity and checks available observation viewer against the envelope viewer.
`state.status` distinguishes pending from available; `lifecycle` is the last
published lifecycle and can be null, while `outcome` is the last rules outcome.
The session binds viewer/build/catalog to its bootstrap/manifest and polls after
the exact `next` cursor. No handle regeneration, replay or automatic retry occurs.

The separate read-only real-host proof requires a native owner to start a
matching host first. It uses an allowlisted same-origin proxy and explicit file
picker imports. Only individual seat files may be passed, never the issued file
or registry. It sends no commands and prints no credentials or observations:

```sh
SDK_NATIVE_COMMIT=<full-native-source-SHA> SDK_NATIVE_INFO=/absolute/native-info.json SDK_NATIVE_BIN=/absolute/mtg-server SDK_BROWSER_HOST_ORIGIN=http://127.0.0.1:18741 node packages/game-server/sdk/scripts/test-browser-host.mjs /private/seat0.json /private/seat1.json
```

The host must remain at the opening boundary during this proof. The harness
checks manifest/resume/stream identity, stable repeated resume and native no-store
responses in isolated Chromium/Firefox contexts. It writes a scrubbed
`generated/browser-host-proof-*.json`; this read path is not a completed-game or
crash-recovery test. The harness proxy is test tooling, not production hosting.

The nested package owns its lockfile and dependencies. It deliberately stays
outside the root npm workspace. From the repository root, install without changing the shared root `node_modules` symlink:

```sh
npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
# Reuse the parent's built mtg-server executable; no implicit Cargo build.
MTG_SERVER_BIN=/absolute/path/to/mtg-server npm --prefix packages/game-server/sdk run check --workspaces=false
```

The default executable is `../../../target/debug/mtg-server` relative to this
package. `MTG_SERVER_SCHEMA` and `MTG_SERVER_BOOTSTRAP_SCHEMA` together accept
explicit Rust exports for verification; `MTG_SERVER_OUTPUT` selects an isolated
generated output directory. Executable mode calls `schema` and `bootstrap-schema`.
The generator rejects external references, future protocol envelopes,
non-Draft7 tuple dialects and unconstrained TypeScript types. It publishes only
complete reviewed declarations, records SHA-256 fingerprints, and removes
failed temporary output. Tests verify deterministic regeneration and failures.

`json-schema-to-typescript` is pinned at16.0.0, TypeScript at5.9.3, and Ajv
at8.20.0 in the standalone lockfile. DTOs come from the generated types; the
compile check exhaustively consumes all33 `PlayerResponse` variants and checks
fixed-arity damage/allocation/combat tuples. Runtime validation uses the exact
Rust schemas without coercion, defaults or removing extra properties. Endpoint
paths and methods come from the same Rust `protocol::ROUTES` export, through a
fingerprinted generated route artifact. Unknown operations, external URLs,
traversal paths and malformed route descriptors fail generation.

```ts
import { GameClient, PROTOCOL_VERSION, type CommandEnvelope } from './src/client.ts';

const client = new GameClient({
  baseURL: 'http://127.0.0.1:3000',
  matchId: '0123456789abcdef0123456789abcdef',
  capability: process.env.SEAT_CAPABILITY!,
});
const page = await client.poll({ after: '0', limit: 32, waitMs: 20_000 });
const entry = page.entries.findLast(e => e.publication.kind === 'view_replaced');
if (entry?.publication.kind === 'view_replaced') {
  const decision = entry.publication.observation.decision;
  if (decision?.request === 'priority') {
    const command: CommandEnvelope = {
      protocol_version: PROTOCOL_VERSION,
      command_id: crypto.randomUUID().replaceAll('-', ''),
      expected_revision: page.next,
      command: { kind: 'decision', token: decision.token, response: { response: 'pass' } },
    };
    const submitted = await client.submit(command);
  }
}
```

This example passes only when the seat has an ordinary priority request. The
SDK never receives raw state, snapshots, recovery sidecars or validation
receipts. Capabilities are private object fields and appear only in the
Authorization header. The client permits HTTPS or loopback HTTP, rejects URL
credentials, disables redirects and cookies, and redacts transport failures.

Requests are not automatically retried. An interrupted submission may already
have been accepted: the caller must retry the exact same envelope/id only when
the host's durable idempotency contract is available. Poll cursors remain exact
canonical u64 decimal strings. A page must advance strictly and `next` must
match its final included entry (or the input cursor on an empty page). Commands
use this latest own-stream cursor as `expected_revision`, including lifecycle
entries; retain the latest permitted observation and its token when a page has
no replacement view. The engine token still determines whether a decision is
current. The optional `events` iterator does not acknowledge progress invisibly; persist
only the cursor of an entry your consumer processed. On failure, reconnect
explicitly from that cursor. Transport abort does not promise cancellation of
an already accepted game command.

`manifest()` reads the typed compiled catalog, four open decklists and public
match/build/profile metadata. Its match ID must equal the configured client
match. No actual seed, library order or private profile digest is transported.
The SDK validates the wire contract; engine admission and catalog identity
verification remain the server's responsibility.

`execution(commandId)` reads the durable execution disposition for this seat's
command: `pending`, `consumed_by_rules`, `discarded_by_departure`, `cancelled`,
or `rejected` with its typed reason. These are separate from host lifecycle
statuses such as `waiting`. Both it and `cancel(commandId)` require a canonical
32-character lowercase hexadecimal ID and verify the returned ID exactly.
`cancel` sends a bodyless POST. It can cancel a still-queued command; it does
not undo accepted rules input. An already accepted command retains its immutable
`accepted` submission result; inspect `execution` to distinguish eventual
consumption from departure discard. Neither method retries automatically.

Response bodies are bounded while reading (8MiB default, configurable up to
32MiB); early rejections cancel their streams. Large exact game quantities
remain strings. Core numeric fields outside JavaScript's exact integer range
are rejected rather than rounded. Before schema validation, a bounded raw JSON
scanner rejects duplicate keys after escape decoding, nesting beyond128, escaped
lone surrogates, and noncanonical numeric lexemes. Structural numbers must use
canonical safe integer spelling: fractions, exponents and `-0` reject even when
`JSON.parse` would round them to an apparently valid integer. Large exact
quantities remain unrestricted canonical decimal strings as specified by Rust.
Typed `win`/`draw` publications are separate from host lifecycle statuses such
as `resource_paused` and `faulted`. Protocol validation does not replace the
engine's legality check or server session authorization.

For retained scoped evidence, run `npm --prefix packages/game-server/sdk run
verify --workspaces=false` with the same `MTG_SERVER_BIN`. It repeats generation,
strict compilation and all test files, then writes `generated/verification.json`
and `generated/verification.log`. The manifest hashes every SDK source and its
lockfile, exact schema/declarations, generator version/options and complete log;
it records actual tool versions,33 exhaustive response variants, tuple checks
and the executed test count. This evidence is scoped to the SDK checks above.
See [AGENTS.md](AGENTS.md) for the nested workflow.

The finite pass/draw consumer is executable with Node26:

```sh
node packages/game-server/sdk/src/examples/foundations-consumer.mjs < /private/path/seat-0-bootstrap.json
```

Launch one separate process per seat. Each reads a bounded private stdin JSON
object with `baseURL`, `matchId`, `capability`, `viewer` (0–3), and optional
`outputPath`. Do not put capabilities in arguments, environment variables or
logs. The optional output is a new mode0600 file; existing files are preserved.
Stdout contains one scrubbed report with counts, cursor, build/catalog identity
and either a typed rules outcome or an explicit nonterminal stop reason.

The consumer fetches the actual public manifest, checks its named profile and
canonical catalog SHA-256, pins the complete public manifest, and rechecks it
before reporting completion. It receives observations and sends responses only
through `GameClient`. Its separately exported, strictly typed policy keeps the
opening hand, passes priority, declares empty attacks/blocks, discards the first
offered excess cards, and accepts a mandatory Commander return if offered.
Other decision families fail explicitly. It never plays lands, casts, activates
abilities or fabricates concessions. This policy exercises natural draw failure;
it is not a general game-playing agent.
Accepted-input counts mean confirmed immutable submission acknowledgments;
they do not claim that a spell resolved or replace the server's consumption
receipts.

Before responding it drains `has_more`, retains its latest own observation
across lifecycle-only pages, and uses the latest own-stream cursor. One offered
token receives one deterministic seat/counter command ID. This example adds an
explicit retry policy above the SDK: only unknown transport outcomes, 429 and
503 retry, preserving the exact envelope/ID, with at most24 attempts and8minutes
per operation inside the25-minute overall bound. A stale revision stops rather
than silently creating a new ID. Each seat stops before a5,001st accepted input,
so four processes accept at most20,000 total. Resource pauses, faults, deadline
and input limits are never reported as Magic outcomes.

There is no saved client checkpoint or process-crash resume claim: pending
envelopes and cursors live only in that process. Server recovery while clients
remain alive is handled by exact retry and subsequent stream reads. Focused
tests include four actual Node processes against a synthetic HTTP fixture;
they establish process/transport/privacy behavior, not an admitted Rust-hosted
game or the P8 four-consumer exit gate.

A separate finite Commander-development consumer uses the same transport loop:

```sh
node packages/game-server/sdk/src/examples/commander-consumer.mjs < /private/path/seat-0-bootstrap.json
```

Its pure `commanderDevelopmentPolicy` receives only the acting seat's Observation
and the public manifest/catalog. It supports the admitted all-Isamaru pod: keep
the opening hand, play an offered own Plains, fund and explicitly pay for its
Commander including public per-caster tax, then attack the next living offered
defender with an eligible Commander. It declares no blockers and handles mandatory
bottom, discard and Commander-return choices. It does not cast optional hand
spells. Unsupported decision families stop explicitly. Quantities and tax use
BigInt internally and exact decimal strings on the wire. It is completion test
machinery and makes no playing-strength claim. The private bootstrap, retry,
input and time limits above apply unchanged.

Four independent Node processes using this policy completed the admitted
all-Isamaru pod and agreed on Win(P2), after65 turns and2,389 accepted inputs.
The server's independent journal and recovery checks establish the corresponding
engine/host evidence; the SDK's schema/mock checks alone do not. Exact release
identities, resource limits and retained reports are in
[the server contract](../../../docs/game-engine-server.md). This finite policy
makes no claim about arbitrary Commander decks or strategic strength.

The attachments completion consumer uses the same bootstrap and unchanged limits:

```sh
node packages/game-server/sdk/src/examples/attachments-consumer.mjs < /private/path/seat-0-bootstrap.json
```

Create one `attachmentsDevelopmentPolicy()` instance per seat/game when using
its exported callback directly. It accepts only `commander-attachments-v1` and
uses that seat's observation plus the pinned public manifest/catalog. It plays
lands, casts affordable commanders and spells, equips and targets Auras, plans
mana from public lands/rocks/dorks (including nested Signet payments), and handles
all 23 generated request families. Blocks use the offered legality hints and
public granted keywords; trample and payments retain exact decimal quantities.
Search choices use only the private handles offered to that seat. This is a
finite completion example, with no strategic-strength claim. Its in-memory mana
plan is private to the consumer; restarting a consumer is not supported.


## Local presentation and origin configuration

`readClientConfig(baseURL, {signal?, fetch?})` reads the public no-store Rust
configuration. `parseClientConfig(text)` validates an explicit captured config.
Both enforce a 4096-byte limit and canonical numeric loopback upstream. The
session owns comparing this upstream with a bootstrap and any explicit Vite mapping;
it must not derive proxy destinations from an imported file.

`GameClient.presentation({signal?})` reads the authenticated Rust descriptor,
then the same-origin immutable JSON without authorization. The result is
`{descriptor, artifact}`. The descriptor is capped at 4096 bytes; artifact at 4 MiB
or the caller's smaller response limit. Hash, producer/schema, shape, catalog,
face ordering and identity checks fail closed with `ProtocolError`. Missing data
remains `HttpError(404)` for session-owned display fallback. No retry or local
rules inference is performed. Producer types and standalone validators are generated
from `../tools/presentation.schema.json`; its producer source and exact schema
hashes are recorded beside the Rust export. Do not mix schemas/builds from a
concurrent engine source; see `docs/game-client-local-serving.md` at repository root.

Action transport adds `GameClient.queryActions(request, { signal })`: one bounded
start/continue/cancel POST, using generated `ActionQueryRequest` and
`ActionQueryResponse`. The caller owns pagination, cursor/token/viewer binding,
expiry, retries and recovery. An incomplete result is never an empty legal menu;
a stale cursor can occur without a changed public decision token. Labels and
missing source contexts stay explicitly unavailable.

For durable session journals, `prepareCommand(envelope)` returns a detached,
deeply frozen `{ envelope, exactBody }`. Persist `exactBody` before sending.
`parsePreparedCommand(exactBody)` strictly validates recovered bytes without
reserializing them. `submitExact(exactBody, { signal })` validates and sends that
exact string once, checking the returned command ID. The SDK does not persist,
retry or settle an unknown command outcome. See the source-qualified
[action transport report](../../../docs/game-client-action-transport.md).
