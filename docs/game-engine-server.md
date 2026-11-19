# Local Rust hosting contract

Foundations and attachments are the reviewed hosted profiles. Profile data requests
hosting; explicit reviewed capabilities and exact pod/build/catalog validation
admit it. Other headless pools are not implicitly hosted. Search is a separate
native consumer. [Evidence](game-engine-evidence.md) owns source-qualified P8 proof;
[client delivery](game-client.md) owns browser assets, bootstrap and opening scope.
Deployment/lobby/matchmaking and account-to-seat provisioning remain unfinished.

## Hosted profiles

`commander-foundations-v1` and `commander-attachments-v1` request hosting through
strict discovered profile data. Reviewed capability sets, whole-pod construction
and exact catalogs/builds admit them; other headless profiles remain rejected.
This selects admission, never profile-specific rules. Public attachments, grants,
counters/tokens are permitted projections; hands, libraries and search choices
remain viewer-private. SDK completion policies use only their seat observation
and public manifest/catalog; they are not browser controls or strategic proof.

Historical P8.2 attachment acceptance required one real four-client default pod,
unchanged 25-minute watchdog, natural typed win/draw at all clients, zero retries/
CoreConcede, final state/input/turn/memory records and completed-game recovery
within 120 seconds with durable tables/head unchanged except generation. Worker
limits stayed 4 GiB, swap zero, tasks 32. Registry/recovery/strict decoding/search
privacy and generated variants had separate checks. [Engine evidence](game-engine-evidence.md)
retains original proof locators; this is not current-HEAD acceptance. Focused host/
SDK checks follow [development](game-engine-development.md#choose-focused-checks).
Resource/time failures never authorize weaker limits or broader admission.

## Read-only product admission

`mtg-server admit-client` reads at most 65,536 bytes of strict duplicate-aware JSON
from stdin and emits one JSON result. Version 1 accepts only `operation: "admit"`,
`profile`, `engine_build_id`, `server_build_id`, `catalog_sha256` and `deck` with
`commander` plus unique `{oracle_id,count}` rows. The commander is included among
the hundred cards. Source/catalog lookup uses the embedded reviewed profile;
caller-selected paths, seeds, names and source hashes grant no admission.

The command verifies both actual executable identities, exact catalog, reviewed
Hosting capabilities and complete four-seat pod admission, using the selected
profile's other three default decks. Success returns `status: "admitted"`, version,
profile/build/catalog identities and canonical normalized deck; rejection returns
`status: "rejected"`, version, reason (`identity`, `profile`, `deck`) and bounded
public detail. Malformed input fails without reflecting input bytes. A configured
Git/source hash is not an authoritative native field. Admission creates no game,
credential or consumer and makes no launch/recovery/supervision claim.

## Product native lifecycle

`mtg-server lifecycle ROOT` consumes one bounded strict version-1 binding line on
stdin and retains that pipe as its lifetime lease. The binding includes game ID,
operation generation, exact profile/build/catalog pins, immutable Oracle deck and
its product digest, and canonical numeric loopback human origin. It persists the
binding and randomly seeded four-deck config before registration. Existing bindings,
all four decks and native registry metadata must agree on recovery; an ambiguous
partial registration fails closed rather than creating a replacement match.
Each game owns one private offline registry and bounded local server, with the
existing systemd worker resource limits. Before spawn, the exact generated worker
unit is durably recorded; stop/join verifies that unit inactive and its resource
group empty, including uncertain startup and parent death. Recovery quiesces that
unit before replacing it. This lifecycle uses explicit adapter recovery, without
hidden broker replacement units. No shared hot registration is implemented.

The lifetime reader watches EOF during startup and hosting. Stop cancels startup
or pauses permitted running execution, shuts the host and joins the worker owner.
Only a lifecycle-issued operator pause authorizes automatic lifetime recovery;
clock/resource faults never become rules results or receive refilled limits.
`lifecycle-stop ROOT` advances the durable generation before waiting for the
exclusive owner lease; old ensure generations fail even after caller uncertainty.
`lifecycle-join ROOT` waits for ownership after an already-written fence. A later
explicit recovery can own the new generation of the same immutable match. EOF
shutdown does not advance the generation. Registry, bindings, single-seat files,
operator capabilities and journals remain host-private (0700/0600); none are served
as assets. A persisted human decision token anchors a stable wait timestamp;
status exports only that timestamp and native typed outcome.

`lifecycle-profile PROFILE` reports exact executable pins and the reviewed public
pod without provisioning. The product runs three separately leased SDK processes,
each with only one seat capability and its own durable exact-command journal.
Their current public foundations pass/draw policy is finite; unsupported decisions
stop explicitly. Attachments retain separate admission and no automated-launch
qualification. [Product launch](../packages/multiverse/web/ARCHITECTURE.md#game-launch)
owns Actor authorization, scoped proxy and remaining browser gates.

## Protocol authority and private streams

Rust DTOs embed the actual public core Observation, PlayerRequest and
PlayerResponse types. Draft7 schemas use deserialize settings for commands and
serialize settings for responses. Shared Rust constants define routes. The
isolated TypeScript SDK generates declarations from those schemas, including
all variants and tuples; there is no separate handwritten rules protocol.

Transport counters are canonical bounded u64 decimal strings. Core quantities
remain exact signed decimal strings, and numeric core fields are restricted to
JavaScript's exact integer range. Both schema and publication checks enforce
these bounds. Unsupported capacity pauses hosting without inventing a rules
loss, concession or draw.

Commands are at most 65,536 bytes. Strict raw JSON validation rejects duplicate
keys, invalid Unicode/scalars, excessive nesting, aliases and imprecise numeric
spellings before typed decoding. There is no caller-selected seat. Bearer seat
capabilities travel only in Authorization headers. The SDK bounds response
reads and validates raw JSON and the pinned schema without coercion or field
removal. Fetch omits cookies, rejects redirects and never retries commands
implicitly.

Each authenticated seat has its own revisioned stream. Pages contain at most
32 entries in cursor order; `next` is the last included cursor, or the supplied
cursor for an empty page. Future cursors reject. Long polls wait at most
20 seconds, with one admitted poll per seat. The command's expected revision is
the latest own-stream cursor, including lifecycle entries. A client retains its
latest Observation/token across lifecycle-only pages.

Public streams contain permitted projections, host statuses and actual rules
outcomes. The public manifest binds profile, rules, catalog,
starting player and all four open decklists. It omits the real seed, private profile
fingerprint, raw State/IDs, snapshots, query receipts and other seats' histories.
Transport lifecycle and per-owner command execution are separate from core
observations. Accepted submission does not imply rules consumption: execution
can remain Pending, become ConsumedByRules or DiscardedByDeparture, or represent
a durable Cancelled/Rejected command.

## Durable ownership and retries

One worker owns one private match directory, exclusive OS lock and SQLite
writer connection. After verified Ready, bounded broker Resume reads use a separate
strict read-only connection and fresh committed snapshot, independent of rules IPC.
Generation/bootstrap, private-file identity and pinned process liveness fence each
read; fatal replies, stop and recovery revoke it. Only new verified Ready reopens it. Directories are mode 0700 and private files 0600. SQLite uses WAL,
synchronous FULL, foreign keys, a bounded database and generation fencing.
Private parents must pre-exist durably; creation syncs the parent directory.
The selected local model trusts the owning OS user and excludes concurrent
same-UID filesystem tampering.

Each transaction binds the exact private operation record, immutable command
result, command execution updates, per-seat outbox and new durable head. Only
successful commit permits live Game installation and acknowledgment. Generation
and expected head checks reject stale writers. Private DTOs use duplicate-aware
raw decoding and exact typed round trips; unknown nested fields cannot disappear
through a permissive tagged-enum decoder.

Idempotency is scoped to match, seat and command ID. Lookup precedes stale-view
validation. Exact body retries return the original immutable result; changing the
body under that ID conflicts. An uncertain RPC is not automatically resent. A
client can retry its original ID/body to discover a durable result. Cancellation
only removes a known unsent queued call and writes its durable tombstone; it
cannot undo an accepted core input.

Registration binds match ID and the actual prepared engine/catalog/private
profile before Ready, including after restart. Copying another valid worker tree
cannot satisfy that identity. These private pins never become public identifiers.
One global OS lock fences the registry, serving and offline create/rotate/revoke
administration. There are at most two registered matches. Staged worker creation
closes, syncs and atomically renames before publication. OS-random capabilities
are issued once through protected output; persisted credential files contain
purpose/match-separated verifier hashes and compare them in constant time.
Rotation/revocation require stopping the supervisor and take effect at restart.

## Exact recovery and history costs

Private checkpoint0 and bootstrap are bounded artifacts. Worker bootstrap and
records use version 3. Recovery proves their actual build/catalog/profile/match
identity and full State hash, then re-executes every exact recorded operation.
Advance budget/return, processed input facts, sequence, boundaries, before-hash
chain, host clocks/readiness, private publication content/cursors, command
results and execution dispositions must all agree. Global tail checks reject
orphan records, commands, execution rows and outbox entries. Generic replay to a
next decision cannot replace exact-operation recovery, including faults that
change a frame without advancing a rule boundary.

The current optimization stages only a bounded private replay cursor. Three
host threads compute actual complete after-State hashes for at most six
outstanding records. SQLite, decoding and sequential rule execution remain on
the owning thread. Every record receives a full hash proof, including repeated
unchanged states; each batch is drained before more records are staged. No
usable Worker or Ready escapes before all hashes, joins and ledger proofs pass.
Thread startup failures, panic, channel failure or any mismatch fail closed and
join all started threads. The original source-qualified P8 verification included production/adversarial,
actual hosting, completed-game recovery, workspace/search and offline retention
checks. These are historical results, not current-HEAD acceptance; exact sources,
builds, proof locations and preserved failures are in the
[original P8 evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/game-engine-server.md#verification-status-and-retained-failures).
This refactor does not
relax the 120-second startup limit.

Immutable remembered views memoize their complete canonical JSON bytes. Cold
initialization uses the P7 row encoder: it retains every serialized view field,
reuses an object row only after full typed equality, and preserves row order and
duplicates. Its local row cache covers only the last cold view for one viewer
and digest. Generic serialization remains the independent exact-byte oracle.
Memos share across forks, do not serialize or affect equality, and invalidate
before shared or unique mutation. Every history byte still enters each State
SHA; no entry is dropped or summarized. This saves repeated serialization but
adds memory proportional to retained serialized history. Kernel measurements
must include memos, simultaneous Game forks, digest skeletons and host buffers.
Later full snapshots still materialize JSON: this release retains the complete
ledger from bounded checkpoint0 and makes no streaming-checkpoint claim.

Commander-history cast-count and damage maps have field-specific canonical
u8-key decoding so nested tagged publications decode the same JSON keys through
SQLite and framed IPC. Duplicate/aliased keys reject. Scalar PlayerId decoding,
map field types, serialization and public schemas retain their original form.

## Hosting limits and clocks

The HTTP1 loopback supervisor uses the pinned Rust dependencies. No HTTP
executor runs Game. A separate systemd-user transient service owns each match;
startup verifies the actual resource-group identity and kernel limits before
loading its catalog or Game.

| Limit | Selected value |
| --- | --- |
| Registered matches | 2 |
| Per-worker kernel memory / swap / tasks | 4 GiB / zero / 32 |
| Queued calls per match | 16 |
| Accepted connections / admitted handlers | 64 / 64 |
| Concurrent polls | 1 per seat |
| Body read / handler / idle connection | 5 / 25 / 30 seconds |
| Worker Ready and RPC deadlines | 120 seconds |
| Worker stdio frame | At most 3 MiB |
| Automatic core advance quantum | 64 |
| External calls between automatic advances | At most 4 |
| Recovery attempts per actor lifetime | At most 3 |
| Initial per-seat idle clock | 30 minutes |

Unsent queue admission, exact cancellation and connection/poll capacity are
bounded independently. Clock checks run even during continuous polling.
The one-second clock-check interval starts after automatic worker work completes,
so a slow Tick cannot continually outrank queued requests. Running execution keeps
its four-external-call advance limit.
Finished/faulted actors await notification rather than spinning. Cleanup after
cancellation, timeout, correlation failure and shutdown kills/reaps the exact
worker resource group. A replacement may start only after the old group is
verified empty. Other matches have independent queues and processes.

Clocks debit monotonic elapsed time only while idle with an offered ready
request. Validation, rule execution, querying and commit work are excluded.
Readiness anchors after durable commit. Recorded elapsed inputs replay exactly;
unrecorded crash/downtime freezes. Expiry is a coherent host resource pause and
never a Magic outcome. Storage/query/publication limits have the same separation.


## Source-qualified verification and recovery

The original P8 completed functional, crash/retry/isolation and offline retained
reconstruction proofs for its exact source. Later attachments, schema and transport
increments retain separate acceptance. See [evidence](game-engine-evidence.md),
[hosted profiles](#hosted-profiles), [client qualification](game-client.md#reproducibility-and-current-acceptance)
and [SDK commands](../packages/game-server/sdk/README.md). No older test count is
current-HEAD acceptance. Full historical proof tables are linked from evidence.

Keep registry/ledger/credentials private (0700 directories, 0600 files). Preserve
original executable, schemas, catalog, accepted input journal and both build IDs
for exact recovery; replacement code cannot silently open old durable state.
Export only permitted projections and sanitized reports. Run fresh offline
reconstruction only for a requested release/provenance claim, through bounded
[retention entrypoints](game-engine-storage.md#retention-defaults), never implicit
SDK generation. Keep failed attempts and original wrapper exit statuses.
