# Game protocol TypeScript SDK

Generated transport for the Rust protocol. Rust public DTOs and
[`protocol::ROUTES`](../src/protocol.rs) own exact schemas, methods and paths;
[`src/client.ts`](src/client.ts) owns exported types/methods. The package root and
`/browser` export production ESM/declarations from `dist/`, never source/examples.
Production App composition remains a separate
[client gate](../../../docs/game-client.md#delivered-scope-and-remaining-gates).

## Build and verify

Use pinned Node through `sh scripts/engine-env.sh` from repository root. This
nested package is outside root workspaces and owns its exact lock/local install.
The native owner builds a compatible executable separately; no SDK command invokes
Cargo or implicitly downloads browsers.

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_BIN=/absolute/mtg-server \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
```

Alternatively supply both `MTG_SERVER_SCHEMA` and `MTG_SERVER_BOOTSTRAP_SCHEMA`
with source-qualified exported files. Executable mode calls `schema` and
`bootstrap-schema`; its default is `../../../target/debug/mtg-server` relative to
this package. `MTG_SERVER_OUTPUT` is an isolated generator-test override; `verify`
uses the ignored `generated/` so all checks consume one output.

`build:browser` generates declarations, route data and standalone Ajv validators
with no runtime schema compilation, Node globals or unsafe-eval, preserving CSP.
The generator rejects external references, unconstrained types and malformed route
descriptors, publishing only complete output. `verify` repeats the build, strict
types and transport/generator tests and writes `generated/verification.{json,log}`;
`generated/generation.json` and `dist/build.json` pin inputs and bytes. Matching
schemas do not prove saved-state compatibility; changed native exports or SDK
source need explicit regeneration.

## Use

```ts
import { GameClient } from '@multiverse/game-server-sdk';
const client = new GameClient({ baseURL, matchId, capability });
const manifest = await client.manifest();
const resumed = await client.resume();
const page = await client.poll({ after: '0', limit: 32, waitMs: 20_000 });
```

| Area | Methods |
| --- | --- |
| Configuration | `parseSeatBootstrap`, `readClientConfig`, `parseClientConfig` |
| Reads | `manifest`, `resume`, `poll`, `events`, `presentation` |
| Commands | `prepareCommand`, `parsePreparedCommand`, `submitExact`, `submit`, `execution`, `cancel` |

Capabilities stay in private fields and Authorization headers, never errors, URLs
or logs. Responses are bounded strict JSON validated without coercion; protocol
validation is not gameplay legality or seat authorization. The SDK has no
persistence, journal or retry policy. [Server routes](../../../docs/game-engine-server.md)
own limits and cursor semantics; [the client contract](../../../docs/game-client.md)
owns [journaling](../../../docs/game-client.md#durable-command-journal),
presentation and [inline decision candidates](../../../docs/game-client.md#core-action-discovery).
The Node-only `./presentation-producer` export checks pinned source identities via
[`presentation.schema.json`](../tools/presentation.schema.json).

## Browser proofs

After explicit build, use the locked browsers; downloads are separate setup:

```sh
sh scripts/engine-env.sh node packages/game-server/sdk/node_modules/playwright/cli.js install chromium firefox
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run test:browser --workspaces=false
```

The synthetic loopback harness exercises production ESM under restrictive CSP with
no inline scripts or unsafe-eval, retaining versions/hashes in create-new
`generated/browser-proof-*.json`. It does not prove Rust hosting or seat isolation.

Read-only real-host proof requires an independently running compatible host kept
at opening. Pass only individual private seat files, never issued registry files:

```sh
SDK_NATIVE_COMMIT=<full-source-SHA> SDK_NATIVE_INFO=/absolute/native-info.json \
SDK_NATIVE_BIN=/absolute/mtg-server SDK_BROWSER_HOST_ORIGIN=http://127.0.0.1:18741 \
sh scripts/engine-env.sh node packages/game-server/sdk/scripts/test-browser-host.mjs /private/seat0.json /private/seat1.json
```

It uses picker import and an allowlisted same-origin test proxy, sends no commands
and prints no credentials or observations; scrubbed results go to
`generated/browser-host-proof-*.json`. No completed-game or crash-recovery claim
follows. [Client evidence](../../../docs/game-client-evidence.md) owns source-qualified results.

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

Unsupported requests stop. The shared retry loop retains the original envelope/ID
on unknown transport outcomes and 429/503 only: at most 24 attempts and 8 minutes
per operation within 25 minutes overall, and at most 5,000 accepted inputs per seat
(20,000 total). Stale revision stops instead of minting a new ID. Accepted counts
are acknowledgments, not resolution; there is no process-crash resume guarantee,
and resource/deadline/input stops never become rules outcomes. Multi-process tests
and admitted four-client proofs keep separate
[host qualification](../../../docs/game-engine-server.md#hosted-profiles).
