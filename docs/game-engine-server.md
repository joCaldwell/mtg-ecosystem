# Local foundations server

Status: **P8 verified for the finite `commander-foundations-v1` profile**. The prerequisite `commander-foundations-v1` P5 profile is
released in `d9afc77`. This server hosts that exact finite profile over loopback
HTTP. Search is a separately verified native consumer and is not exposed here.
Deployment, lobbies, ratings, matchmaking and spectator policy are later work.

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
connection. Directories are mode 0700 and private files 0600. SQLite uses WAL,
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
join all started threads. Production and independent adversarial checks passed,
as did the actual hosting and completed-game recovery proofs below. Full workspace tests and current-build search acceptance passed;
offline reconstruction and independent artifact audits also passed.
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

Axum 0.8.9 and Tokio 1.53.3 provide the HTTP1 loopback supervisor. No HTTP
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
Finished/faulted actors await notification rather than spinning. Cleanup after
cancellation, timeout, correlation failure and shutdown kills/reaps the exact
worker resource group. A replacement may start only after the old group is
verified empty. Other matches have independent queues and processes.

Clocks debit monotonic elapsed time only while idle with an offered ready
request. Validation, rule execution, querying and commit work are excluded.
Readiness anchors after durable commit. Recorded elapsed inputs replay exactly;
unrecorded crash/downtime freezes. Expiry is a coherent host resource pause and
never a Magic outcome. Storage/query/publication limits have the same separation.

## Verification status and retained failures

All previous attempts remain preserved with their source, executable, SDK and
harness identities. A failed attempt never substitutes for a rules outcome.

| Candidate / attempt | Evidence |
| --- | --- |
| Original 188-file release, pass/draw clients | Failed the unchanged 25-minute watchdog: 2,675 consumed commands, public turn 79, no outcome |
| Original release, Commander clients | Failed after 17 consumed inputs; exposed nested Commander-history key decoding |
| Fixed 189-file release, four Commander clients | Completed naturally: 65 turns, 2,389 accepted inputs, typed Win(P2) at all four independent Node SDK processes, zero retries and no CoreConcede |
| Fixed release, copied completed recovery | Failed normal 120-second Ready deadline; original bytes and all seven copied durable tables/head remained exact, generation alone advanced |
| Memoized 190-file release, direct host diagnostic | Failed its separate 300-second total harness guard at 2,382/2,389 inputs; no complete-game or cold-recovery result claimed |
| Same 190-file release, copied partial prefix | Failed fixed 120-second diagnostic recovery; all seven tables/head and original bytes remained exact, generation alone advanced |
| Parallel recovery, before cold-encoder correction | Direct cold recovery passed in 95.8 seconds; a concurrent search diagnostic exposed roughly threefold cold-hash cost |
| Current 192-file typed-cold candidate | Actual four-client game completed in 325.489 seconds; normal completed-game Ready passed in 101.713 seconds; 107 smoke and 55 backpressure checks passed; quiet search, offline reconstruction and independent artifact audits passed |

The current immutable candidate is Build
`7e77708cad1f80dfa7ad3be82d198f3ad6eae7f62b9c127879a335206bbc8da5`,
server SHA256 `0becfed3cff7cf0882c634e3a4124dd201ce914c54c6516d3f6bb2fbdf26ec28`,
with 192 native files / 52,558,543 bytes. All 92 server tests, 20 focused cold-memo
checks, 40 SDK tests, strict TypeScript, Clippy and formatting passed; the public
schema retains its exact bytes. Independent source/feature review binds the
unchanged deterministic core dependency boundary.

Four actual independent Node SDK clients completed the admitted all-Isamaru
pod in 325.489 seconds, within the unchanged 25-minute guard. All four reported
Win(P2) after 65 turns and 2,389 accepted/consumed inputs, with zero retries and
no CoreConcede. The public damage totals were [40,22,38,22] and life [0,18,2,18],
exactly 40 minus those totals. The recorded play includes 65 land plays, four
Commander casts, four mana activations/spends and 61 declared attackers.
Final head 4785 / core boundary 6849 has full State SHA256
`0dacaf403068c8dd452e4b253e42f902525b2a03bf6f3c6db6bb5942a3b7b728`.
Worker peak was 1,049,440,256 bytes under the actual 4 GiB/swap-zero/tasks32
kernel limits, with zero OOM kills. Independent copied-worker review verified
command/outbox/head integrity; all original bytes and owned resource groups
were checked after closure.

A fresh direct host diagnostic independently produced the same 65-turn,
2,389-input natural Win(P2) and full Core State hash. A separate cold process recovered it in
99.3794 seconds at 927,735,808 bytes with four observed threads. All seven table
fingerprints, durable head and host state matched; only generation advanced.
Including all remaining setup/final-audit overhead gives a conservative bound of
102.1462 seconds. Direct diagnostics remain separate from normal startup.

Normal startup then passed against a new private copy of the closed actual HTTP
game: Ready in 101.712544 seconds under the unchanged 120-second deadline,
914,870,272 bytes under actual 4 GiB/swap-zero/tasks32 limits and zero OOM kills.
All four HTTP seats reported Win(P2). All seven database tables, full durable
head and Core State matched; only generation advanced from 2 to 3. Independent
review hash-checked the original registry without opening its SQLite database,
and confirmed supervisor exit and empty owned groups. Normal recovery report
SHA256: `c0dc6e9e5421c49e08b247b64c11e186e2391008fa70ea30f899dae0e83c4f83`;
independent review:
`6441e67444262296940e37de79bab06c9fe06e59baae5dd2f3ce07e73f947737`.

All twelve current-build search diagnostics met the minimum useful trials,
5-second hard limit and memory limit. Independent review matched all 651 useful
trials to the P7 reference by candidate, counts, outcomes and complete State hash.
One concurrent root took 3.144373 seconds; the current build's separate quiet sixty-job
p95 gate subsequently passed at 2,662,119 microseconds. Build-bound assignment identifiers are expected to
differ across builds and are not substitutes for gameplay comparisons.

Current-build quiet search, all-sixty semantic replay, full workspace
debug/release, fresh offline reconstruction and independent retained-artifact
audits passed. The complete private proof archive is retained outside the worktree. The finite gameplay catalog/policy and public schema remain
pinned throughout.

### Final verification and source qualification

The five passed phases are development checks, separate workspace release tests,
SDK verification, engine offline retention and server/SDK offline retention.
Development/release passed 526 Rust tests each; the development gate also passed
40 Node checks. SDK verification passed 40 tests and strict TypeScript, including
fresh offline reconstruction at its new retained location.

The first attempt failed a permission fixture because the private driver's
`0077` mask reached test children and turned requested `0755` into `0700`.
Verification children now explicitly use `0022`; the parent keeps `0077` and
application paths still enforce `0700` / `0600`. All 33 original driver guards
and independent review passed. The subsequent user-requested overnight pause
interrupted retention; the cancelled status and completed logs remain immutable.

Engine offline retention then passed. Server retention exposed two host-tool
issues: npm 12 rejects direct tarball cache requests by default, and flat SDK
copying omitted a repository-relative test catalog. The final two-file host
correction permits only each validated registry tarball cache command, while
client installation/verification remain offline with scripts disabled. It
reconstructs `client-source/packages/game-server/sdk` beside exactly one native
fixture, `packages/game-engine/tests/fixtures/expected-match-ir.json`, whose SHA256
must agree with the actual retained native source authority before/after use.
Fourteen focused host guards passed, including real npm behavior and relocation,
corruption, mismatch and extra-fixture negatives.

The final companion pins the exact original four passed phase objects, logs and
old source freezes; it executes only server retention under the new two-host-file
freeze. All 192 native files, all 23 SDK files and the nine other host inputs remain
unchanged. All eight predecessor evidence roles pass. The final source freeze is
`de9a6dd3350421cdb3a19df795fb7a246b3c5387f4e0f300ce2f93b8fa047c4a`;
final five-phase status is
`da1c4f116af4c7a6c2fd505f8dcbd3f5a16bebefa0391098df875b6aa3aebbc6`.
This reuse preserves the earlier host-source qualification; those commands are
not represented as executions of the final host scripts.

## Offline reconstruction and retained proofs

`npm run retain:engine` rebuilt with an empty Cargo home/fresh target and frozen
offline mode, then verified full replay in 1,199.535326 seconds. Independent audit
checked all 192 native source files, 151 external packages / 10,704 vendor files,
eleven catalogs, 42 configurations, 540 trace executions, 727 snapshots, 17,627
recovery boundaries and nineteen legal-match journals. Historical initial State
remains `58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.

`npm run retain:server` independently rebuilt the server with another empty Cargo
home, passed 92 server tests and compared exact compiled NativeInfo/SQLite options.
It reconstructed the isolated SDK with 20 SHA-512-locked public tarballs and
53 byte-pinned cache files, performed a fresh offline install, generated schemas
and declarations, and passed strict TypeScript and all 40 SDK tests. Its completed
phase took 198.188076 seconds. A fresh run produces ten exact generated artifacts;
the older eleven-file development directory also held an ancillary test log.
The auditor now verifies the exact ten-file output set. The failed audit and its
source remain retained rather than being relabeled passed.

The native audit also recovered two disappeared temporary historical report copies
from unchanged retained P7 fixtures. Their exact canonical bytes match hashes
recorded before the reboot. This is historical byte recovery, not fresh execution.
All original failed attempts and their source/executable/harness identities remain
preserved. Statistical profiling warnings and the unavailable historical
preoptimization profiling chronology remain explicit in the search report.

External store:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-11fdee9d9b50bb8d-2Y0CLl/`.

| Artifact | SHA256 |
| --- | --- |
| Native bundle, 2,087,587,224 bytes | `11fdee9d9b50bb8d2e4fb345dfc6bbc5f05a04a16dcfd919cf48e5976c7f9ce4` |
| Offline simulator executable | `731d7690b3367f9eca28bf3af3620ad3caf21461d616f9fa80ea76a56de46ed8` |
| Offline server executable | `26f34b45729f33705fa10107319e7240d5a97f1e032cc164cbbd24a15b2a7f2c` |
| Server/SDK `server-b6cUNM/evidence.json` | `87a7a5ac13d9e20d8448d6eb10c4ee84f6d18b69978afce44466abf03bb5afb0` |
| Independent native audit | `d1fddb2e4e818de9252757efc63f1d25f288874fa94f8a2dc95af123f97bb4ca` |
| Independent server audit | `39546b1f84c1890eaac013f136c6c355826a6d8a6f97259e0987b86bd2082fa8` |
| Final private proof archive manifest | `a11f203b1cff1334c57e13655a7f93549c138e77dd00c69596238085d600952a` |

`server-proof-archive/files/` retains every closed integration, recovery, quiet
measurement, independent replay, profiling, source, failed-attempt and final audit
proof: 5,522 files / 3,335,347,670 bytes. The complete source/destination byte maps
were rechecked before and after copying; directories/files/executables have
private modes and no active-prefix omissions. Native vendor/rebuild data and the
server/SDK reconstruction remain beside that archive. Measured executables retain
their original performance claims; rebuilt hashes differ while native Build agrees.

This closes scoped P8 hosting. Broader dense/history, retained-branch, fixed-work
scaling and trusted same-strategy omniscient diagnostic measurements remain work
in the engine plan. No general Magic, strategic-strength, deployment or public
service claim follows from this release.
