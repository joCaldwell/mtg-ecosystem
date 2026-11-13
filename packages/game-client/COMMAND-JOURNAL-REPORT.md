# Durable command journal checkpoint

Implementation source: `b6de2dd00bbde58043ffac082f5204b3f82aca4c`, based on the
clean T3-bound `origin/main` checkout at
`144fe059ab2707877109d7e9dbe8751f204681dd`. This is the standalone command session
slice, not full M2 or playable Commander. The app still uses
`browserReadSession()` and has no command controls or live send wiring.

The SDK owner supplied original checkpoint `d2fee399`, cherry-picked unchanged
with attribution as `305b90bd`. Its three SDK files provide `prepareCommand`,
`parsePreparedCommand` and `submitExact`; this lane authored no SDK wire encoder,
SDK policy/retries, Rust, native schemas or qualification-pin changes. Integrate
this prerequisite once when combining lanes. Other changes are session files,
distinct command/journal tests and one package-local test script. No dependency
or lockfile changes, root install, Cargo, deployment, merge or cleanup occurred.
The original shared checkout and existing worktrees/artifacts were left alone.

## Contract and ownership

`ReadSession` remains the sole read loop and processed cursor owner. Optional
`Authorization.commands` injects the pure `CommandLifecycle`; no second poller,
clock or cursor exists. Authorization, the existing exclusive Web Lock,
manifest identity, metadata catalog and actual resume viewer/build/catalog checks
all precede journal access. One seat-only lookup precedes current-build matching.

IndexedDB database `multiverse-command-journal`, version 1, has one `commands`
store keyed by the JSON tuple `[server origin, match, seat]`, excluding build and
catalog from the key. A read/write transaction checks the key and total count and
adds one slot atomically. The command is not sent until transaction **complete**,
using strict durability. Request success alone never authorizes a send. Concurrent
tabs cannot insert two unresolved commands even without cooperating Web Locks.
Existing Web Locks still precede every session journal/network operation, with
no forced takeover or fallback ownership. Standard references:
[IndexedDB transaction scheduling](https://w3c.github.io/IndexedDB/#transaction-scheduling)
and [durability](https://w3c.github.io/IndexedDB/#dom-idbtransaction-durability).

Persisted shape is exactly `{version, binding, exactBody}`. Binding contains only
origin/match/seat and engine/server/catalog/protocol identities. The detached,
immutable SDK-decoded envelope is reconstructed in memory. No capability, full
observation, diagnostic or redundant envelope is stored. Strict recovery rejects
unknown fields/versions, malformed identity values and invalid/duplicate/oversize
wire JSON through the SDK parser. Explicit default-port origins such as `:80`
retain their original identity. Missing current seat records do not permit deletion
or replacement of another seat/build's records.

The SDK prepares and validates the initial envelope, retaining original command ID,
expected revision and decision token. The session persists `exactBody` before
calling SDK `submitExact(exactBody)`. Recovery uses `parsePreparedCommand` and
always sends the persisted string; no retry regenerates an ID, revision, token,
response or JSON serialization. Both injected-fetch and actual browser-to-HTTP
transport-boundary tests compare exact bytes, including retained whitespace.

Accepted submission, pending execution, absent execution row, changed view/token,
abort, timeout, local stop, refresh and incompatible build are not terminal.
Missing execution status authorizes only replay of the same persisted body.
Authoritative server dispositions map as follows, from current
`game-server/src/protocol.rs`, `worker.rs` and queued broker cancellation:

| Server result | Journal behavior |
| --- | --- |
| Submission accepted / execution pending | Retain unresolved slot |
| Execution consumed_by_rules | Resolve as consumed |
| Execution discarded_by_departure | Resolve as departure-discarded |
| Execution cancelled / definitive rejected reason cancelled | Resolve as cancelled |
| Other definitive submission/execution rejection | Resolve as rejected; retain public rejection enum in memory |
| Missing status / transport or decoding failure | Retain uncertainty; no replacement |

Cancellation requests concern a server-queued command. An accepted cancellation
response can be the immutable result of an already accepted command and does not
clear it. Missing/not-queued/error responses retain uncertainty. Local draft
cancellation belongs to controls; this implements no universal Undo or concession.

Terminal journal deletion compares the complete original binding and exact body
within its own transaction. Before accepting another explicit intent, the read
owner must obtain a coherent resume started after resolution. A failed refresh
keeps this gate. Controller incarnation advances after resolution even if revision
and token are unchanged, invalidating the old draft. A fresh explicit human callback
is necessary; the old response is never automatically resubmitted as a new command.

Every async boundary and effect initiation is fenced by the owning authorization
generation. Stop/StrictMode/seat replacement preserves durable uncertainty and
cannot let late completions publish into a replacement seat. Synchronous subscriber
cleanup during persisting/sending/rejection is also fenced before subsequent I/O.

## Bounds and failure behavior

- SDK command limit: 65,536 UTF-8 bytes. Local binding fields are at most 512
  characters, with strict match/seat/digest validation. At most 64 unresolved
  records per browser database: at most 4 MiB of command bytes plus bounded
  bindings/record metadata and browser-managed IndexedDB overhead. Terminal
  resolution alone frees slots; no eviction, expiry or cleanup of uncertain work.
- Open and transaction each have a 3-second bound. Blocked/timeouts fail closed;
  late successful opens close their connection. Version changes close connections;
  unsupported database versions fail rather than resetting data.
- Unavailable/denied storage, blocked opens, quota/count exhaustion, corruption,
  conflict and transaction failure produce bounded `CommandSnapshot.failure` enums.
  A failed scan or persistence blocks command creation/send. Read-only observation
  access can remain available. Failed terminal deletion retains uncertainty.
- Four automatic status/replay opportunities per command authorization, driven
  only after successful reads by the existing loop. Successful reads do not renew
  that command budget. Pending/unknown remains unresolved after exhaustion; an
  explicit retry callback checks status/replays the original body once. Refresh
  reauthorizes and scans; it cannot manufacture a replacement. Transport failures
  retain unresolved state and can reconcile through this bounded read owner.
- Existing read-side four-retry/backoff and interrupted-response-body recovery are
  preserved. SDK sends once and contains neither journals nor retries. Browser
  command effects retain the existing 30-second headers/body deadline.

## Controls/integration interface

Pure types are in `src/session/command.ts`; UI needs type imports only:

```ts
interface DecisionIntent {
  kind: 'decision';
  source: 'human';
  context: CommandContext;
  response: PlayerResponse; // generated SDK type
}
// CommandContext: binding: JournalBinding, incarnation: number,
//                 revision: string, token: string
```

`ReadSession.commandContext()` returns the current immutable context or null.
`submitIntent(intent)`, `retryCommand()` and `cancelQueuedCommand()` are the only
command actions. Snapshot `command` exposes phase, commandId, definitive disposition,
public rejection reason and bounded failure enum; it contains no persisted body or
credential. Phases are disabled/scanning/ready/persisting/unresolved/sending/
awaiting-refresh/blocked. A duplicate/stale/unbound intent has no effects.

The controls owner emits its own local `DecisionCompletion` callback. Future app
composition must retain the **original complete CommandContext**, compare the
local origin/match/seat/incarnation/revision/token and then adapt that explicit
human completion. Never stamp an old response with a fresh context after rejection,
refresh or a replacement decision. The full JournalBinding includes builds,
catalog and schema; local controls do not need SDK runtime imports or I/O.

`browserCommandSession(protocolSchemaHash)` is an explicit, uncomposed opt-in
factory. The argument must be the protocol schema digest of the actually qualified
SDK build; it is not obtained from a user draft or arbitrary server text. Existing
`browserReadSession()` never creates or reads a journal and cannot send commands.
An integration owner must deliberately compose the opt-in factory only after its
SDK/native/pin/serving gates are qualified; the current restricted development proxy
still admits read routes only. No app/feature/decision code was changed here.

## Exact qualification and setup

All lane SDK/client checks used one SDK build, with these unchanged baseline inputs:

| Identity | Value |
| --- | --- |
| Native source for baseline exports | `936566b76e9819c4503c9065590be2a66037d26a` |
| Protocol | `7fe738bb5a4aecaedb23a9a6405691c6497c6b117b38464aacde6601f152204c` |
| Bootstrap | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| SDK generation manifest | `6d4ccd06a8cb00bd3133b1d7363e009525159dcec6047cf1ec79eb79e411e58d` |
| SDK build manifest | `f98b5868c43bd2f0850dd65fb0f3da3767681a7571f703499aa4226ce1c3668e` |
| SDK browser.js | `bbeda831398bd807272319422f0c4d657e7958535387b7e1d0d72b3c68c4bc2a` |
| Client lockfile (unchanged) | `1f837860ed1ab73700fbf18193eb1deaa1e2fadee31f520a1d808ae9fa38a45e` |

Inputs are retained at the action owner's worktree beneath
`generated/action-transport/baseline/{protocol,bootstrap}.schema.json`.
Original compiled source closure remains
`/home/josh/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/blocking-target-merged-native-source-manifest.json`.
The retained baseline executable `blocking-target-mtg-server-03` has SHA-256
`bc2289df5b978ea76d038b23452b6f328e05b9aae630344d3fc8d8a8c5155240`,
engine `edf075b61f6497fcc51ca20c7d3107cef0b3afea35323d9026d2defc983b197c`,
server `384609e1f0ffc0e3a25abd147d172f0b67bdfdede3fbee22ee763ba4ee7722f4`.
This lane inspected its identity; it did not build or provision that executable.

From repository root, with supplied absolute export paths and already installed
Playwright Chromium/Firefox binaries:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_SCHEMA=/absolute/baseline/protocol.schema.json \
MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/baseline/bootstrap.schema.json \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run test:journal:browser --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run test:browser --workspaces=false
```

Pinned Node 26.11.1, local Playwright 1.65.0, Chromium 158.0.8127.18 and Firefox
159.0. SDK verify passed strict types/browser packaging and 64 tests. Client check
passed fixture qualification, strict types, import/effect boundaries, **60 Node
cases** and production build. This includes all original 38 Node cases and the
22 distinct command/journal cases. The separate real browser journal run passed
**16 cases** (seven subcases plus parent per engine). Existing production fixture
browser suite passed **10 cases**, including no API requests or persisted data on
the unchanged read-only inspection route. No tests skipped or retried in final runs.

Journal browser tests serve transpiled session source modules and the exact built
SDK on an isolated loopback test origin; they test actual IndexedDB and Web Locks,
not a storage shim or the uncomposed Vite command factory. Timing/authorization
cases use injected deterministic effects. Synthetic envelopes/statuses are labeled
in tests; they establish no card facts, rules legality or full match coverage.

Ignored evidence root: `packages/game-client/generated/command-journal/`.
`qualification-final.json` binds runtime source, exact SDK/client manifests/bundle/lock
hashes and all retained log hashes; the earlier `qualification.json` is preserved. Passed receipts include `sdk-exact.log`,
`client-check-checkpoint.log`, `browser-checkpoint.log` and
`fixture-browser-checkpoint.log`. The production build retains existing chunk-size
and React Router directive warnings. Package install reported the SDK's existing
one moderate advisory; dependencies were not rewritten.

## Separately qualified real native path

The action-transport owner explicitly supplied a real host command probe at
`/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-action-ee918eaddd68f820/generated/action-transport/host01/report.json`,
SHA-256 `184d1153d6830b898100d35bb04a3227062f757153a7ae33bf2853051c7dcd9f`.
Four source-backed permitted foundations mulligan keep commands were actually
submitted and consumed. For the first, its SDK fetch wrapper discarded a completed
real HTTP 200 after commit; there was one SDK send, a transport failure, a real
`consumed_by_rules` status and an exact duplicate accepted result. It is not an
unmodified network-fault run or a browser-journal integration test.

Frozen `attempt02/producer-manifest-final.json` SHA-256
`8a802cb0ea26f53cef88415fb50b3c81a390d8206e0f107c77ecfcab46510aa9`
qualifies native source `1e6fbefffb908cdf7dd741c4baa97f6467c4c85d`, SDK source
`76b5f3a1e4cfe8e32189fd197b3f1edf05670f80`, native source fingerprint
`ba6379af2980ffc855635c9662c3989af1a11b93ff8e635fda1d778362fd7692`,
binary `fb53ef1b029dc6f3c7abeb21791347cff07ef3f1958b484dcb138e2018c6d521`,
engine `0e8abd9759e99c6be84492e859396a90fcdc4254b88d125e294114f2ed50730d`,
server `99d06fefdcff1569acd690b5c4f1f7d78ecc34452399f3530b39cbc45acf3777`,
protocol `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`
and unchanged bootstrap. Private registry/bootstrap files remain only with the
producer; none were copied into this report or the journal.

This lane compared the complete command, submission, execution, resume, stream
and manifest schema components: all are byte-identical after canonical key sorting
between its baseline and that newer producer. `producer-receipt.json` retains the
comparison hashes and the original manifest checkpoint receipt; that checkpoint
is frozen as `producer-manifest-checkpoint.json`, hash
`0092c3fe4e4e70aa7e11682cb024fd1994d04b7e689e9b17977b0759e271bcbf`.
`producer-receipt-final.json` references the final immutable manifest without
rewriting the earlier receipt. No newer schema was installed to pass lane checks.

The real native producer probe is paired with the actual browser journal fault
suite as separate, explicitly qualified evidence. End-to-end execution of this
opt-in browser journal factory against the **combined current native/SDK/controls**
remains an integration gate; the peer probe does not establish it.

## Preserved findings and omissions

Meaningful initial failures remain in this lane's ignored evidence:

- `types-first.log`: browser `page.evaluate` result needed an explicit return type;
  fixed the test typing without weakening strictness.
- `browser-first.log`: Chromium's lock release after page close was asynchronous;
  the test now waits for the actual lock inventory before reacquiring. Firefox
  already passed. Product ownership behavior was unchanged.
- `browser-second.log`: destroying the HTTP socket before headers did not reliably
  expose response loss to browser fetch. The test now flushes headers/partial JSON
  then drops the body, exercising the preserved SDK TransportError correction.
- Self-review added post-rejection refresh-failure and synchronous-subscriber-stop
  regressions before the runtime checkpoint. Failed refresh cannot lose its gate;
  subscription cleanup cannot initiate effects after releasing ownership.

No engine/native compilation, full runtime/performance/retention/profile/game
matrices, natural game, bots, live UI send composition, cross-device fencing,
Safari, physical power-loss/storage-eviction proof or human usability pass ran.
No full M2/playability or combined current-native journal acceptance is claimed.
The integration owner must combine transport/pin updates and controls, retain one
qualified SDK build, supply its exact protocol digest to the opt-in factory,
qualify command serving/proxy routes, run the actual combined browser-to-native
journal path and retain its own source-qualified report before enabling live sends.

## Parent integration qualification

Parent combined reviewed action transport with main `0b4a8757`, native source
`e86df7e73f5ac896a4b2488779efb46ef28240d4`, then merged that source into this
branch at `41874eeabafae90d1660ab477cfd446c2de6bd7a`. The prepared-command
prerequisite is preserved once. The separate SDK qualification now deliberately
uses current `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`
exports. Baseline historical source/fixture/provenance bytes above remain history.

With one current SDK build: generation, strict types and 66 SDK tests passed;
client types/boundaries/all 60 tests/build and all 16 Chromium/Firefox IndexedDB
journal cases passed. These current receipts are under this worktree's root
`generated/m2-integration/`; old standalone receipts were preserved.

The controls branch combines this exact journal source with its unchanged
controls implementation. Its combined checks passed 67 client tests, 22 fixture
browser cases and 16 journal browser cases. A separate test-only same-origin
proxy served the actual browser command factory/session source and production SDK
against the fresh native `serve-client` binary. In both Chromium and Firefox,
the pure control adapter produced a response to an actual permitted mulligan
request from its retained original CommandContext. An explicit test button sent
that response through the real journal factory. The proxy interrupted the body
only after a real native HTTP 200; the exact persisted body hash matched the
native POST, local stop retained its slot, and navigation/reimport reconciled
`consumed_by_rules` through native execution status and refreshed the view. No
replacement or replay was sent, and the old intent was ignored. The slot cleared
only after authoritative disposition. This is automated integration evidence,
not deployed App wiring or human play.

That receipt and reproducible harness live in the controls worktree's root
`generated/m2-integration/native-journal-02/report.json` and `native-journal.mjs`.
The initial harness checked for an opening before native progress had settled;
its bounded-readiness failure remains in `native-journal-01.log`. The corrected
harness waits for the actual committed request. All owned host/browser processes
are stopped. Fresh native identities and focused build checks are recorded in
`docs/game-client-action-transport.md`'s parent qualification section. Product
read-only proxy/app behavior and source-label/context gaps remain unchanged.
