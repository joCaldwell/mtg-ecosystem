# Local Rust hosting contract

[Evidence](game-engine-evidence.md) owns source-qualified proof;
[client delivery](game-client.md) owns browser assets, bootstrap and opening scope.
Deployment, lobby/matchmaking and account-to-seat provisioning remain unfinished.
Focused host/SDK checks follow [development](game-engine-development.md#choose-focused-checks).

## Hosted profiles

`commander-foundations-v1` is the reviewed hosted profile: its data requests
hosting, and admission checks reviewed capabilities, the whole pod, catalog and
build identity, never selecting profile-specific rules. Other headless pools are
not implicitly hosted; search is a separate native consumer. Public attachments,
grants and counters/tokens are permitted projections; hands, libraries and search
choices stay viewer-private. SDK completion policies use only their seat
observation and public manifest/catalog and are neither browser controls nor
strategic proof.

Capacity, clock, storage, query and publication limits pause hosting coherently.
They never become a loss, concession, draw or other Magic outcome, and never
authorize weaker [limits](#hosting-limits-and-clocks) or broader admission.

## Read-only product admission

`mtg-server admit-client` reads at most 65,536 bytes of strict duplicate-aware JSON
from stdin and emits one JSON result. Version 1 accepts only `operation: "admit"`,
`profile`, `engine_build_id`, `server_build_id`, `catalog_sha256` and `deck`
(`commander` plus unique `{oracle_id,count}` rows; the commander counts toward the
hundred). Source/catalog lookup uses the embedded reviewed profile; caller-selected
paths, seeds, names and source hashes grant nothing.

It verifies both actual executable identities, the exact catalog, reviewed Hosting
capabilities and four-seat admission with the profile's other three default decks.
Success returns `status: "admitted"`, version, identities and the canonical
normalized deck; rejection returns `status: "rejected"`, version, reason
(`identity`, `profile`, `deck`) and bounded public detail. Malformed input fails
without echoing bytes. Admission creates no game, credential or consumer.

## Product native lifecycle

`mtg-server lifecycle ROOT` consumes one bounded strict version-1 binding line on
stdin and keeps that pipe as its lifetime lease. The binding holds game ID,
operation generation, exact profile/build/catalog pins, the immutable Oracle deck
and its product digest, and the canonical numeric loopback human origin. The
binding and randomly seeded four-deck config persist before registration. On
recovery, bindings, all four decks and native registry metadata must agree; an
ambiguous partial registration fails closed rather than creating a replacement.

Each game owns one private offline registry and bounded local server under the
standard worker limits. The generated worker unit is durably recorded before spawn;
stop/join verifies it inactive with an empty resource group (including after
uncertain startup or parent death), and explicit recovery quiesces it before
replacement. There are no hidden broker replacement units or shared hot
registration.

EOF or stop cancels startup or pauses permitted running execution, shuts the host
and joins the worker owner. Only a lifecycle-issued operator pause authorizes
automatic lifetime recovery; clock/resource faults never receive refilled limits.
`lifecycle-stop ROOT` advances the durable generation before waiting for the
exclusive owner lease, so stale ensure generations fail even after caller
uncertainty; `lifecycle-join ROOT` waits after an already-written fence; EOF
shutdown does not advance the generation. A later explicit recovery may own the new
generation of the same immutable match. Registry, bindings, single-seat files,
operator capabilities and journals stay host-private (0700/0600) and are never
served as assets. A persisted human decision token anchors a stable wait
timestamp; status exports only that timestamp and the native typed outcome.

`lifecycle-profile PROFILE` reports exact executable pins and the reviewed public
pod without provisioning. The product runs three separately leased SDK processes,
each with one seat capability and its own durable exact-command journal; their
finite foundations pass/draw policy stops explicitly on unsupported decisions.
Attachments keep separate admission and no automated-launch qualification.
[Product launch](../packages/multiverse/web/ARCHITECTURE.md#game-launch) owns Actor
authorization, scoped proxy and remaining browser gates.

## Protocol authority and private streams

[`protocol::ROUTES`](../packages/game-server/src/protocol.rs) defines the protocol
version and routes. The acting seat receives a `Decision<Viewer>` inside its
observation, with ordered candidates and all legal selection constraints. A
decision command supplies the token and a core `Answer` whose indices refer to that
token's candidate table. A validated input retains its selections across queued
departures. Priority and payment need one authenticated observation read. The host
exports Draft7 schemas from Rust for SDK declarations and standalone validators.

Transport counters are canonical bounded u64 decimal strings; core quantities are
exact signed decimal strings; numeric core fields fit JavaScript's exact integer
range. Schema and publication checks both enforce these bounds.

Commands are at most 65,536 bytes. Strict raw JSON validation rejects duplicate
keys, invalid Unicode/scalars, excessive nesting, aliases and imprecise numeric
spellings before typed decoding. There is no caller-selected seat; bearer seat
capabilities travel only in Authorization headers. SDK transport rules are in its
[manual](../packages/game-server/sdk/AGENTS.md).

Each authenticated seat has its own revisioned stream. Pages hold at most 32
entries in cursor order; `next` is the last included cursor, or the supplied cursor
for an empty page. Future cursors reject. Long polls wait at most 20 seconds, one
admitted poll per seat. A command's expected revision is the latest own-stream
cursor, including lifecycle entries; clients keep their latest Observation/token
across lifecycle-only pages.

Public streams contain permitted projections, host statuses and actual rules
outcomes. The public manifest binds profile, rules, catalog, starting player and all
four open decklists, omitting the real seed, private profile fingerprint, raw
State/IDs, snapshots, query receipts and other seats' histories. Command execution
is separate from core observations, and acceptance does not imply consumption:
execution stays Pending, becomes ConsumedByRules or DiscardedByDeparture, or is a
durable Cancelled/Rejected command.

## Durable ownership and retries

One worker owns one private match directory, exclusive OS lock and SQLite writer
(WAL, synchronous FULL, foreign keys, bounded size, generation fencing). After
verified Ready, broker Resume reads use a separate read-only connection and fresh
committed snapshot, fenced by generation/bootstrap, private-file identity and
process liveness; fatal replies, stop and recovery revoke them until a new Ready.
Directories are 0700 and private files 0600; private parents must pre-exist
durably and creation syncs the parent. The trust model is the owning OS user,
excluding concurrent same-UID filesystem tampering.

Each transaction binds the exact private operation record, immutable command
result, execution updates, per-seat outbox and new durable head; only a successful
commit permits live Game installation and acknowledgment. Generation and expected
head checks reject stale writers. Private DTOs use duplicate-aware raw decoding and
exact typed round trips, so unknown nested fields cannot vanish through a
permissive tagged-enum decoder.

Idempotency is scoped to match, seat and command ID, and lookup precedes stale-view
validation. Exact body retries return the original immutable result; a changed body
under that ID conflicts. Uncertain RPCs are never resent automatically; a client
may retry its original ID/body to discover the durable result. Cancellation only
removes a known unsent queued call and writes a durable tombstone; it cannot undo
an accepted core input.

Registration binds match ID and the actual prepared engine/catalog/private profile
before Ready, including after restart; copying another valid worker tree cannot
satisfy it, and these pins never become public identifiers. One global OS lock
fences the registry, serving and offline create/rotate/revoke administration.
Staged worker creation closes, syncs and atomically renames before publication.
OS-random capabilities are issued once through protected output; credential files
hold purpose/match-separated verifier hashes compared in constant time.
Rotation/revocation require stopping the supervisor and apply at restart.

## Exact recovery and history costs

Private checkpoint0 and bootstrap are bounded artifacts; worker bootstrap and
records use version 3. Recovery proves actual build/catalog/profile/match identity
and the full State hash, then re-executes every recorded operation. Advance
budget/return, processed input facts, sequence, boundaries, before-hash chain, host
clocks/readiness, private publication content/cursors, command results and
execution dispositions must all agree; global tail checks reject orphan records,
commands, execution rows and outbox entries. Generic replay to a next decision
never replaces exact-operation recovery.

Parallel after-State hashing still proves every record's full hash; no Worker or
Ready escapes before all hashes, joins and ledger proofs pass, and any thread
failure or mismatch fails closed. Remembered views memoize canonical JSON bytes
(unserialized, invalidated before mutation, never affecting equality); every
history byte still enters each State SHA. Memory measurements must include memos,
simultaneous forks, digest skeletons and host buffers. Full snapshots materialize
JSON; there is no streaming-checkpoint claim.

Preserve the original executable, schemas, catalog, accepted input journal and both
build IDs for exact recovery; replacement code cannot silently open old durable
state. Export only permitted projections and sanitized reports; keep failed
attempts and original wrapper exit statuses. No older test count is current-HEAD
acceptance ([evidence](game-engine-evidence.md),
[client qualification](game-client.md#reproducibility-and-current-acceptance)).

## Hosting limits and clocks

The HTTP1 loopback supervisor uses pinned Rust dependencies; no HTTP executor runs
Game. A separate systemd-user transient service owns each match, and startup
verifies its actual resource-group identity and kernel limits before loading the
catalog or Game.

| Limit | Selected value |
| --- | --- |
| Registered matches | 2 |
| Per-worker kernel memory / swap / tasks | 4 GiB / zero / 32 |
| Queued calls per match | 16 |
| Accepted connections / admitted handlers | 64 / 64 |
| Concurrent polls | 1 per seat |
| Body read / handler / idle connection | 5 / 25 / 30 seconds |
| Worker Ready, recovery and RPC deadlines | 120 seconds |
| Worker stdio frame | At most 3 MiB |
| Automatic core advance quantum | 64 |
| External calls between automatic advances | At most 4 |
| Recovery attempts per actor lifetime | At most 3 |
| Initial per-seat idle clock | 30 minutes |

Queue admission, exact cancellation and connection/poll capacity are bounded
independently, and matches have independent queues and processes. Clock checks run
even during continuous polling; the one-second interval starts after automatic
worker work, so a slow Tick cannot starve queued requests. Finished/faulted actors
await notification rather than spinning. Cleanup after cancellation, timeout,
correlation failure and shutdown kills/reaps the exact worker resource group, and a
replacement starts only after it is verified empty.

Clocks debit monotonic elapsed time only while idle with an offered ready request;
validation, execution, querying and commit are excluded, and readiness anchors
after durable commit. Recorded elapsed inputs replay exactly; unrecorded
crash/downtime freezes. Expiry is a resource pause, not a Magic outcome.
