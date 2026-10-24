# Game protocol TypeScript SDK

This is the generated client for the Rust protocol in `../src/protocol.rs`.
The schemas and declarations are generated from `mtg-server schema`; they are
ignored artifacts. The HTTP client has schema, mock-transport and real local
server checks. Complete admitted four-player integration is tracked separately
in [the server contract](../../../docs/game-engine-server.md). Production
authentication and deployment are outside this local release.

The nested package owns its lockfile and dependencies. It deliberately stays
outside the root npm workspace. From the repository root, install without changing the shared root `node_modules` symlink:

```sh
npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
# Reuse the parent's built mtg-server executable; no implicit Cargo build.
MTG_SERVER_BIN=/absolute/path/to/mtg-server npm --prefix packages/game-server/sdk run check --workspaces=false
```

The default executable is `../../../target/debug/mtg-server` relative to this
package. `MTG_SERVER_SCHEMA` accepts an explicit Rust-exported schema file for
verification; `MTG_SERVER_OUTPUT` selects an isolated generated output directory.
The generator rejects external references, future protocol envelopes,
non-Draft7 tuple dialects and unconstrained TypeScript types. It publishes only
complete reviewed declarations, records SHA-256 fingerprints, and removes
failed temporary output. Tests verify deterministic regeneration and failures.

`json-schema-to-typescript` is pinned at16.0.0, TypeScript at5.9.3, and Ajv
at8.20.0 in the standalone lockfile. DTOs come from the generated types; the
compile check exhaustively consumes all29 `PlayerResponse` variants and checks
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
it records actual tool versions,24 exhaustive response variants, tuple checks
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
