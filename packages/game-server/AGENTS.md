# Local Rust host and generated clients

Read [the current engine map](../../docs/game-engine-current.md),
[hosting contract](../../docs/game-engine-server.md), [client delivery](../../docs/game-client.md)
and [focused verification](../../docs/game-engine-development.md#choose-focused-checks).
Rust DTOs/public core types own the protocol. Generated schemas/SDK follow those
types; no handwritten response/observation replacements. Core rules remain deterministic.

- Host owns auth-to-seat, HTTP/OS/SQLite, queues, clocks and resource groups.
  Derive viewer from verified authorization; product sign-in alone grants no seat.
- Reviewed hosted profiles are foundations/attachments; search-enabled foundations
  is separate. Capabilities, exact catalogs/pods and both engine `BUILD_ID` and
  host `SERVER_BUILD_ID` fence admission/recovery. Names never grant mechanics.
- Retain durable acceptance before acknowledgment, immutable command dispositions,
  match-scoped IDs, bounded queues/pages and one executor per match. Test
  crash/retry/reconnect/seat isolation for changes to those paths.
- Never expose seeds, snapshots, receipts, raw IDs, internal traces, query sidecars,
  other seats' history or global private rule counters. Bearers stay in headers,
  never URLs/logs. Resource/clock faults are not game loss/draw.
- Core quantities are exact decimal strings; transport counters are canonical
  bounded u64 strings. Public numeric fields must fit JavaScript's exact range.
- Keep registry/worker directories 0700 and ledgers/credentials 0600 regardless
  of umask. Verification children use process-local 0022 for permission fixtures;
  driver stays 0077. Do not change global umask.
- Preserve original executables/data/evidence for exact recovery. No deployment,
  lobby/account service or new profile admission follows from protocol scaffolding.

Use pinned tools and shared Cargo lock from [development](../../docs/game-engine-development.md).
Default engine check compiles server targets but omits server runtime tests.
Choose affected `cargo test --locked -p mtg-server FILTER` / production Clippy,
then relevant isolated SDK checks. Confirm selectors execute tests. SDK generation
never starts Cargo; [SDK manual](sdk/AGENTS.md) governs its local lock/install.
Commit/push coherent focused checkpoints with omissions; no automatic full matrix.
