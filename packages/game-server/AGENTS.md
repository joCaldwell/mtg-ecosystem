# Local Rust hosting and generated clients

For current engine architecture, setup and evidence boundaries, start with
[the engine map](../../docs/game-engine-current.md) and
[bootstrap/doctor](../../docs/game-engine-development.md). Release figures below
are historical and source-qualified; later IR25 candidate work is separate.

Engine/server follow-ups use `docs/game-engine-verification-workflow.md`: cheap
focused verification is the default; full runtime/release/profile checks are
explicit opt-ins. The default engine check compiles server targets but does not
run server tests. Add focused host/SDK tests for host/API changes. Commit and push
coherent focused checkpoints without waiting for a full matrix; see that workflow
for exact cadence and coverage omissions.

D1b-3 hosting admission treats each profile's `hosted` flag as a request,
validated against the explicit reviewed IR capability set at build time and admission. The current hosted
profiles remain foundations and attachments. Registry tests iterate the data
registry and its embedded catalogs; adding an unhosted profile needs no server
selector or test-table edit. See `docs/game-engine-profiles.md`.

D1b-2 gives the host a separate `SERVER_BUILD_ID`, binding its own Rust source and
manifest/build script to engine `BUILD_ID`. Registry metadata, worker bootstrap
and the process identity handshake pin both. Public manifests publish both;
recovery rejects changed or missing host identity before opening durable state.
Engine `BUILD_ID` remains engine-code-only.


This package implements milestone P8 for `commander-foundations-v1`. Read
`docs/game-engine-plan.md` P8 and `docs/game-engine-spec.md` §§5–7 first.
Protocol scaffolding does not establish a playable or durable server.

P8.2 adds exactly `commander-attachments-v1` under the frozen
`docs/game-engine-server-attachments.md` contract. Its four acceptance items
passed; measured identities, evidence and limitations are recorded in
`docs/game-engine-progress.md`. The historical release figures below retain
their original foundations source/build qualification. All hosting guards and
public foundations schema bytes remain unchanged.

Release verification passed 526 Rust debug tests plus 40 Node checks,
526 release tests, and 40 SDK tests with strict TypeScript. Current-build search
passed sixty primary jobs and all independent replays: p95 2,662,119µs, maximum
2,690,433µs, kernel whole-process peak 394,698,752 bytes, with 4,188 useful trials
and 3,012 unstarted trials excluded. The package README binds the exact build.
Fresh native/server/SDK offline retention and independent artifact audits passed.
Original passed phases keep their exact freeze/log qualification across the user
pause and two host-only retention fixes; all native and SDK sources stayed exact.
Fourteen focused host guards cover npm 12 cache opt-in and the repository-relative
SDK/native-fixture layout. Keep the required catalog fixture pinned when moving
the SDK tests. Final proof locators are in `docs/game-engine-server.md`.

D2c-a's production server/simulator builds and SDK verification passed against
engine snapshot/bundle formats 19/20: 47 SDK tests, strict TypeScript and 33
response variants. `StepBeginningTriggerV1` remains unreviewed for hosting and
search; no hosted profile was added. Public projection/context/sidecar versions
remain 16/16/15. The exact current identities and functional verification are in
`docs/game-engine-cards-as-data-d2c-a.md`; historical P8 performance figures above
retain their original build qualification.

D2c-b production server/simulator builds and SDK verification passed against
IR v22 with unchanged snapshot/bundle 19/20 and public 16/16/15: 47 SDK tests,
strict TypeScript and 33 response variants. `EventPlayerRecipientV1` and
`SourceStatusConditionV1` remain outside reviewed hosting/search capability sets;
no hosted profile was added. See `docs/game-engine-cards-as-data-d2c-b.md` for the
exact engine/server identities and source closure. This is functional evidence,
not a new quiet performance claim. The D2c-b report/progress ledger records the
completed parent review; this historical increment does not accept later IR25 work.

Rust links the deterministic core directly. Rust DTOs and embedded core public
projection/decision types are the external protocol authority. Generate Draft7
schemas with explicit inbound/outbound contracts; generate TypeScript from those
schemas. Do not maintain parallel hand-written response or observation types.
Core quantities remain exact decimal strings. Transport counters are canonical
u64 decimal strings; published core numeric fields must fit JavaScript's exact
integer range. A resource pause is never a Magic loss or draw.

From repository root:

- `cargo test --locked -p mtg-server`
- `cargo clippy --locked -p mtg-server --all-targets -- -D warnings`
- `cargo run --locked -p mtg-server -- schema`

The verification driver gives test/build children an explicit `0022` creation
mask, the conventional environment expected by permission-fixture tests. It
keeps its own private `0077` mask and records the child setting in the frozen
verification metadata. Do not change the user's global umask. Application
registry/worker directories and credential/ledger files still enforce explicit
`0700` and `0600` modes regardless of that test environment.

The SDK's local `package.json` documents its generation/type/runtime checks.
Keep generated schemas/types, databases, credentials and execution artifacts
ignored. Pin dependency lockfiles. Native retention includes this crate's Rust
sources and manifest because it is a Cargo workspace member.

The local/loopback service provides: authenticated per-seat projections,
match-scoped command IDs, immutable idempotent submission outcomes, durable
acceptance before acknowledgment, bounded queues/pages, explicit cancellation,
one authoritative executor per match, and recovery to exact recorded boundaries.
Do not expose host state, seeds, snapshots, receipts, raw IDs, internal traces,
query sidecars, other seats' histories or global private rule counters. Bearer
capabilities belong only in authorization headers, never URLs/logs. Derive the
viewer from authentication. A public viewer revision changes only with permitted
information. Clock/resource/fault statuses stay separate from rules outcomes.

Verify crash/retry/reconnect/private-stream behavior and isolation before claiming
hosting complete. No deployment, public authentication service or lobby is in
this milestone. Preserve unrelated parser work in the main checkout.
