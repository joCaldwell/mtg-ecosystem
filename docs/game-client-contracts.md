# Proposed browser integration contracts

Status: planning only. Companion to [client plan](game-client-plan.md).
The [responsibility boundaries](game-client-boundaries.md) govern all contracts below.
Names below describe contracts, not implemented routes or final wire spellings.
Rust DTO/schema review must freeze gameplay fields and bounds before implementation;
the offline presentation producer owns its separate artifact schema.

## 1. Existing surface and ownership

[protocol.rs](../packages/game-server/src/protocol.rs) currently exports five
operations: manifest, submit, stream, execution status and cancellation.
[PlayerSession](../packages/game-engine/crates/core/src/engine/player_session.rs)
offers observe, decision, check_response, resume and search_context; it does not
enumerate legal actions. [visibility.rs](../packages/game-engine/crates/core/src/engine/visibility.rs)
defines the actual request/response vocabulary.

Semantic reads execute through the owning worker's core query API. Resume and
command-status reads use committed host records; immutable manifest/presentation
reads serve validated pinned artifacts. HTTP handlers do not run Game or implement
rules. Authenticate seat-scoped reads; never accept a caller-selected viewer.
Rust DTOs generate gameplay schemas, routes and TypeScript. Queries preserve the
engine's purity, budget, handle-lifetime and privacy invariants. SDK wire validation
does not establish legality. The session, not SDK or components, owns retry/journal policy.

## 2. Browser SDK packaging

Add a documented browser entrypoint and reproducible build/export contract to the
existing isolated SDK. Preserve its own lockfile and independent generation tests.
Generated outputs stay ignored build artifacts. The client build uses explicitly
generated artifacts with their source/schema digest; it must not invoke Cargo
implicitly or import SDK tests, Node consumers or private source fixtures.

Prove ESM imports, JSON import handling, abort behavior, streaming bounds and exact
numbers in an actual production browser bundle. SDK validation currently calls
Ajv.compile at module load. Prefer build-generated standalone ESM validators to
support a CSP without unsafe-eval. Preserve custom numeric formats, strict raw JSON
decoding and schema parity; this is a prerequisite proof, not an established ability.
If generation fails, resolve the validator design before implementation progresses;
do not weaken runtime validation or CSP to make the demo pass.

Evidence: [SDK package](../packages/game-server/sdk/package.json),
[validation](../packages/game-server/sdk/src/validation.ts),
[SDK manual](../packages/game-server/sdk/AGENTS.md).
Official validator reference: <https://ajv.js.org/standalone.html>.

## 3. Local joining and browser authorization

Chosen local-alpha default: a local operator tool exports exactly one seat's
bootstrap file, selected from an already provisioned match. The browser imports
that file through an explicit file picker. It contains the loopback server origin,
match ID, one seat capability and expected seat identity. Validate its schema and
origin before constructing the SDK. Verify actual viewer identity on observation.
The operator credential and other seats' credentials must never enter the browser.
The existing full issued credential file is NOT a browser bootstrap.

Capabilities remain in memory and authorization headers. They are absent from URL,
history, browser storage, diagnostics and command journal. Refresh or browser
restart requires reimport of the same single-seat file. This is the documented
local-alpha reauthorization experience, not seamless account-based resume.

Serve static client assets and API under one loopback origin. Vite development
uses a same-origin proxy to the existing host. In development the browser SDK base
is the Vite origin; a configured, allowlisted proxy maps it to the bootstrap's
upstream origin. Verify match/build/catalog identity through that mapping. Never
use the bootstrap origin to silently bypass the proxy or accept arbitrary upstreams.
Specify Host/Origin checks for the
browser-facing surface and a restricted CSP; avoid wildcard cross-origin access.
Do not change the SDK's credentials:omit behavior to suggest cookies already work.
Public cookie sessions, CSRF policy and account-to-seat mapping are later contracts.

One browser profile/origin may have one active controller per match/seat. Acquire
a Web Lock before polling or sending; a second tab explains that the seat is open
and waits for the owner to close/release it. Initial release has no forced takeover.
Never broadcast credentials or hand contents. If the required lock API is unavailable,
fail the connection clearly in the supported-browser gate. Locks do not fence other
devices/browser profiles: duplicate imported credentials there are unsupported in
local alpha. A 429 cannot identify which controller is primary or prove a competing
controller exists; stop automatic retry after a bounded conflict window and present
possible contention/rate limiting without claiming ownership or forcing takeover.
Cross-device
takeover requires server-issued ownership generations in a later increment.

## 4. Coherent resume read

Add a bounded authenticated read returning the latest committed publication state:

- Match, authenticated viewer, engine build, server build, catalog and protocol identity.
- Last committed permitted observation, including its decision token when present.
- Current own-stream cursor and permitted lifecycle/outcome.

Read these atomically from one durable publication boundary. Do not observe Game
again merely to generate fresh handles; do not expose raw state, private operation
sequence, query receipts, work sidecars or hidden state hashes. A new stream poll
starts after that returned cursor. History is separate and paginated. Current
observation survives lifecycle-only entries. If no observation is ready, return an
explicit pending status instead of mixing data from different boundaries.

This endpoint is new work. An early read-only prototype may replay from zero, but
full-alpha refresh acceptance requires bounded resume. Final response-size limits
must be compatible with host limits and the SDK; over-budget is explicit, never a
truncated observation. Add strict schema and authorization tests.

## 5. Action beginnings and decision context

Introduce a core query for available action beginnings, scoped to current viewer,
decision token and permitted objects. Support priority and mana windows as required
by hosted profiles. Return action kind, existing object/ability handle or index,
source-backed label reference and a response template for initiating the action.
Examples of kinds are pass, land play, cast, activate and mana activation.

The core query must use shared rules predicates/validation rather than a second rule
table. Its semantics are 'this initiation is allowed'; targets, costs, payment and
other later choices may remain. Do not enumerate every X value, payment partition
or target combination. Avoid claiming that candidate membership guarantees a whole
joint selection is legal.

Core owns eligibility, decision context and semantic completeness/staleness. Server
owns HTTP envelopes, request quotas, continuation lookup/expiry and scheduling;
it must not derive eligibility from the catalog. Use bounded object/ability scope
or pages, with explicit complete/incomplete/stale
results. A continuation is opaque, viewer/token bound and expires on relevant
state change; trusted core Work stays private. No query changes game state,
public history or cursors. Exhaustion is a query status and cannot become a rules
loss or a state-changing resource pause merely through repeated discovery calls.
Freeze per-seat quotas, maximum page/work and cancellation semantics in M0.

Test pairs of states differing only in opponent-hidden information: action results,
reasons, handles, cursor behavior and public resource responses must not expose
the difference. Generic unavailability is preferable to a hidden-state explanation.
Submission invokes core validation against current state after host admission checks.

Core adds permitted source/ability and semantic context where requests lack explanations
(for example optional effects and mode indices). Labels come from exact source
metadata through offline compiler/source identity mappings; UI formats the prompt
without interpreting IR. Core does not own localized copy or artwork. Context is
additive and generated.
Only expose a source when permitted by the viewer's observation.

A bounded proposed-response preflight may be added if interaction testing requires
it. It is advisory and pure; it never returns a receipt usable as a privileged
submission. This endpoint is not mandatory merely to avoid ordinary stale rejection.

## 6. Pending commands and recovery

Use a versioned IndexedDB journal with at most one unresolved command per
server-origin/match/seat/build/catalog binding. Persist the exact validated envelope
before the first send, including command ID, original expected revision and decision
token. An IndexedDB failure blocks sending and gives a recoverable explanation.
The journal contains private choices; no full observation, capability or shared
telemetry. Remove records only after definitive rejection/cancellation,
departure-discard or confirmed consumption. Accepted/pending acknowledgment keeps
the record. Unresolved records remain until authoritative reconciliation. A user
may stop the local session, but cannot clear ambiguity and resume sending a
replacement action; preserve the unresolved binding/tombstone. Never silently
expire ambiguity. Before opening a new build/catalog journal, scan unresolved
records by origin/match/seat so a changed identity cannot bypass this guard.

Command sequence:

1. Acquire controller ownership and validate match identity.
2. Process known stream backlog; verify the current decision and selected handles.
3. Build one envelope with the latest processed own cursor and fresh command ID.
4. Persist the exact envelope, then send. Disable duplicate submissions for it.
5. Record the immutable submission result. Accepted does not mean consumed or resolved.
6. Observe execution disposition and subsequent observations. A consumed cast may
   lead to another required choice rather than a resolved spell.

On rejoin, recover journal only after matching identity and authorization. Read
coherent current state, then reconcile the outstanding command using execution
status and/or an exact-envelope retry. A missing execution row is not proof the
original cannot still arrive. Unknown transport outcomes retain the same ID/body;
never substitute a newer revision. Repeated bounded retry failures show a resumable
offline state, not a new gameplay command. Respect 429/503 backoff and server hints.

A definitive stale/invalid rejection resolves that original attempt. Refresh the
view and revalidate intent; require confirmation before constructing a new command.
A changed decision discards the old draft. Do not auto-replay old choices against
new tokens. Switching seat clears in-memory private data; retain unresolved journal
under its old binding so it can be reconciled only after that seat is reauthorized.
On incompatible build/catalog, stop and preserve the journal for recovery tooling.

Escape cancels a local draft. Transport abort does not undo a command. Explicit
queued-command cancellation follows the existing server contract. `FinishMana`
and its `undo_mana` behavior require a dedicated source-reviewed interaction; they
are not a universal Undo button. Concession is separately confirmed and never a
test shortcut for natural-game acceptance.

## 7. Presentation metadata

Offline TypeScript source-data tooling derives an allowlisted presentation artifact
from the pinned archived source; compiler tooling supplies semantic ability mappings.
The server verifies artifact identity/shape and serves it; the client formats it.
Neither server request handlers nor client components parse Oracle text/IR for
rules or active-effect explanations. The artifact contains:
Oracle identity, printing/face identity, names, Oracle text, mana/type text, source
ability labels, stored image URLs and provenance digest. Link it to catalog identity.
Validate one-to-many face mapping. Never construct Scryfall API image lookups or
depend on live card fetching during play. Missing artwork shows text and identity.

Core-observed characteristics delivered unchanged by the server override printed
values for current game display.
Do not infer rules from printed text or images. Tokens use engine-supplied identity
and characteristics with a fallback when no reviewed artwork exists. Static public
catalog metadata may be cached by digest; private observations and API responses
must not enter a service worker/offline cache. No service worker is needed initially.

## 8. Compatibility, timing and deployment boundaries

Pin client release to tested protocol/schema digest, observation version, hosted
profile capabilities and request coverage. Accept compatibility intentionally;
matching the latest compiler IR number is insufficient. Bind an active session
and journal to its exact match engine/server/catalog identities. Different builds
cannot silently resume saved host state. Retain the original executable/data when
upgrading development binaries; no rolling live-match migration is promised.

The server publishes lifecycle statuses, not a general authoritative clock API.
Initially show status/wait duration without a fabricated game-clock countdown.
Add a specific clock projection before showing authoritative remaining time.

Local host still relies on Linux systemd-user resource groups, offline registration
and a two-match registry. A static frontend deployment alone cannot host this game.
Online hosting needs separate provisioning, durable storage, process supervision,
auth, shutdown/drain, backups and capacity evidence. No hosting platform is selected
by this plan.
