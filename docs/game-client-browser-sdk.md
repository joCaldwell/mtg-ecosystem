# Browser SDK implementation evidence

Scope: `packages/game-server/sdk`, from base
`691496f6410c188d0e26f3a6f46d35f5ee159243`. This is an M0 package/validator
checkpoint, not acceptance of all M0–M4 work. Native semantics, host orchestration,
client session policy and presentation remain with their respective owners.

## Implemented package contract

The isolated package exports browser ESM and precise declarations through
`@multiverse/game-server-sdk` and `/browser`. Runtime exports are `GameClient`,
`HttpError`, `TransportError`, `ProtocolError`, `counter`, `PROTOCOL_VERSION`,
`parseSeatBootstrap`.
Generated `Observation`, `PlayerRequest`, `SeatResume`, `ResumeState`,
`SeatBootstrap` and other DTO types are reexported;
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

## Current native dependency and verified slice

The second SDK checkpoint depends on the native contract at
`5ff4fd94b6dace1cdee0f0223305e054c42af8f3` ([native PR #14](https://github.com/joCaldwell/mtg-ecosystem/pull/14)).
It does not duplicate or modify that lane's Rust changes. Frontend integration
must consume the native dependency and both SDK checkpoints (first `f2fb129`).
Native report-only follow-ups do not change the schema/binary identity.

Native artifacts are retained at
`/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-integr-f43e8db4a0817154/generated/client-native/`.
Its `checkpoint.json` pins source/build/schema/fixture evidence. This SDK consumes:

- Protocol schema SHA-256:
  `519ce989ecc98c87e4aba0a14c1c7aa04fffa1aa552c7f2905cdb42f290b93ae`.
- Separate bootstrap schema SHA-256:
  `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
- Engine build:
  `5a911d22f157820122bb332ed206af9ad6756ec303d8bdba6792be0fe3ac60ac`.
- Server build:
  `4b84b60f982a0104eb2c1347a66f0aab5faf849fb26e7f19eabcef7118009d38`.

The generated sixth route implements `resume({signal?})`, capped at 131072 bytes
or the caller's smaller limit. It correlates match identity and available-view
viewer identity. `parseSeatBootstrap(text)` checks the native separate schema,
4096-byte bound, strict JSON, and canonical numeric loopback origins. These are
wire checks; the session still owns bootstrap selection, upstream allowlisting,
identity binding and recovery policy. No action/context DTO was added.

Reproduce from repository root with the pinned Node launcher, using the exact
native artifact paths or equivalent retained copies:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_SCHEMA=/absolute/client-native/protocol.schema.json MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/client-native/bootstrap.schema.json sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run test:browser --workspaces=false
```

`verify` includes explicit generation and browser build; frontend-only builds use
`build:browser` with the same two inputs. Executable mode alternatively uses
`MTG_SERVER_BIN` and runs only `schema` / `bootstrap-schema`, never Cargo.

Current checks passed: isolated `npm ci`; strict TypeScript including published
package imports; all **54 Node tests**, including **19522** independent runtime
Ajv/standalone comparisons across all 33 response/28 request wire variants;
**12 browser fixture checks each** in Chromium 158.0.8127.18 and Firefox 159.0.
Resume bounds are inclusive; malformed/bootstrap secrets, explicit default-port
origins, mismatched viewers and pending/available shape negatives are tested.
The final injected-native-fetch case verifies the same receiver requirement for
both default and explicitly supplied fetch. Runtime bundle size is 1202477 bytes
(uncompressed; no compression or latency acceptance claim).
Final bundle SHA-256:
`567373c22ac8480fadaef86c6b24239960140bcfba19312472a413a37cdd358b`.
Receipts: SDK `generated/verification.{json,log}` and
`generated/browser-proof-1794530552560.json`; exact code and build inputs are
hashed there. Final documentation edits do not change measured code.

Real-host read proof passed in **four isolated contexts**: seat 0 and seat 1 in
each browser, not four independent seats. An explicit same-origin proxy allowed
only the configured `http://127.0.0.1:18741` upstream and generated read routes.
Each context imported its individual private file through a picker, read manifest,
resume, stream after `next`, and repeated resume: 16 authenticated HTTP 200 reads.
Viewer/build/catalog matched; resume remained the committed opening boundary;
no-store and CSP held. No game command was submitted. Browser output contains
only public build/catalog identity, viewer/cursor and bounded counters; it omits
credentials, hands, decision tokens and raw observations. Native owner then
stopped the host; this lane started no native process.

Real-host receipt: `generated/browser-host-proof-1794530471739.json`. Its exact
bundle hash is `d432718e4092462791dbb797b82ccb102c53ead460129ac377b1f25c0809c0d1`,
retained with exact hashed runtime/build inputs in `generated/native-read-proof/`.
It precedes the final one-line extension of global fetch binding to explicitly
injected fetch. The default-fetch path used by the host proof is unchanged;
the final 54-test/12-check proof covers that extension. Do not attribute the old
bundle hash to the final source. The original 53-test receipts are retained as
`generated/native-verification-53.{json,log}`.

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

The browser package and bounded read-only native path are verified as qualified
above. This does not complete every M0 gate: action/context contracts, static
presentation serving and later session/journal integration remain separate.
The synthetic fixture alone does not prove hosting, and the real read proof does
not establish durable recovery or playability.

Action beginnings/decision context and compiler-backed source labels are separate
unfrozen contracts. No SDK action/query replacements were invented. Full engine
matrix, hosted profile expansion, human games, four-browser isolation, session
recovery and deployment were not performed. Safari is untested.
