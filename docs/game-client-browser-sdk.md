# Browser SDK implementation evidence

Scope: `packages/game-server/sdk`, from base
`691496f6410c188d0e26f3a6f46d35f5ee159243`. This is an M0 package/validator
checkpoint, not acceptance of all M0–M4 work. Native semantics, host orchestration,
client session policy and presentation remain with their respective owners.

## Implemented package contract

The isolated package exports browser ESM and precise declarations through
`@multiverse/game-server-sdk` and `/browser`. Runtime exports are `GameClient`,
`HttpError`, `TransportError`, `ProtocolError`, `counter`, `PROTOCOL_VERSION`.
Generated `Observation`, `PlayerRequest` and other existing DTO types are reexported;
related shapes can be addressed through indexed access. `ClientOptions.fetch`
accepts native fetch-compatible fixture/transport injection. No retries, journals,
legality decisions or strategy are introduced.

Generation emits standalone Ajv validators with identical strict options and
uint8/uint32/uint64/uint formats. The raw JSON scanner and transport bounds remain
shared. Production runtime has no Ajv compiler, Node globals, external imports,
JSON import attributes, private fixtures or example consumers. Browser fetch's
required global receiver is now bound explicitly. Node's permissive receiver had
hidden this browser failure.

Build contract and commands are in the SDK README. All dependencies remain in
its exact standalone lockfile; root dependencies were not installed or changed.
No Cargo command ran in this lane. Playwright 1.65.0 and esbuild 0.28.2 are pinned.
Generated schema, validators, declarations and bundle manifests carry SHA-256
source/artifact inventories. No generated output is committed.

## First historical-schema experiment

Input: retained Rust export from D2cd `394301d`,
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/d2cd-lifetime-394301d-20261104T030216Z/sdk-generated/protocol.schema.json`.
Copied into SDK `generated/inputs/d2cd-394301d.schema.json`.
SHA-256 `b7def42ae371eb079517904e5c1a7eba02fdfe58c88ba15120f97ddede631b55`.
This is historical-source evidence, not a claim that the current host exports
identical schemas or supports this source's saved state.

Executed using Node 26.11.1 through `sh scripts/engine-env.sh`:

- `npm --prefix packages/game-server/sdk run build:browser --workspaces=false`
  with `MTG_SERVER_SCHEMA` pointing at that export: passed; 784127-byte ESM.
  Bundle SHA-256 `a05bc1d9cb6e10ec180eb937608d2da8a3bad30097961a11a38924b4356ade9d`.
- `node --test packages/game-server/sdk/tests/{client,execution,strict-json,generate,package,validator-parity}.test.ts`:
  24 passed. Standalone parity: 19062 structural/adversarial comparisons, all 33
  response and 28 request wire variants. These are wire witnesses, not legal
  gameplay fixtures or a hosted coverage claim.
- `npm --prefix packages/game-server/sdk run test:browser --workspaces=false`:
  11 checks each on Chromium 158.0.8127.18 and Firefox 159.0, passed. Actual
  production module loads under restrictive CSP; dynamic evaluation is explicitly
  blocked. Native browser HTTP, abort, bounds/cancellation, exact strings,
  malformed raw JSON, custom formats, no retries, Web Locks and IndexedDB pass.
- Full `verify`: stopped at strict TypeScript because this historical schema
  lacks current `Characteristics.snow`. Expected source mismatch preserved in
  `generated/historical-schema-typecheck-failure.log`; no declaration fallback.

Other receipts under SDK `generated/`: `historical-browser-build.log`,
`historical-focused-tests-final.log`, `historical-browser-tests-final.log`,
`browser-proof-1794529921335.json`. The browser report pins exact build/harness
sources. Earlier successful experiment `browser-proof-1794529765580.json` is
retained; the first browser run failed on unbound Window.fetch before its fix.

## Remaining dependency gates

The native lane has supplied a new resume/bootstrap export and a matching host;
exact dependency commit and current-source SDK adaptation are the next checkpoint.
This first checkpoint does not implement those DTOs, consume a real host in a
browser, or pass the current complete SDK suite. The synthetic HTTP fixture does
not prove authentication/seat isolation, durable recovery or playability.

Action beginnings/decision context and compiler-backed source labels are separate
unfrozen contracts. No SDK action/query replacements were invented. Full engine
matrix, hosted profile expansion, human games, four-browser isolation, session
recovery and deployment were not performed. Safari is untested.
