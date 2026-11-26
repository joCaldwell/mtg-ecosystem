# Local Rust host and generated clients

Read [the current engine map](../../docs/game-engine-current.md),
[hosting contract](../../docs/game-engine-server.md), [client delivery](../../docs/game-client.md)
and [focused verification](../../docs/game-engine-development.md#choose-focused-checks).
Rust DTOs/public core types own the protocol; generated schemas/SDK follow them,
with no handwritten response/observation replacements. Core rules stay deterministic.

- Host owns auth-to-seat, HTTP/OS/SQLite, queues, clocks and resource groups.
  Derive the viewer from verified authorization; product sign-in grants no seat.
- Capabilities, exact catalogs/pods and both engine `ENGINE_BUILD` (installed into
  core by `main`) and host `SERVER_BUILD_ID` fence admission/recovery. Names never
  grant mechanics.
- Retain durable acceptance before acknowledgment, immutable command dispositions,
  match-scoped IDs, bounded queues/pages and one executor per match. Test
  crash/retry/reconnect/seat isolation when changing those paths.
- Never expose seeds, snapshots, receipts, raw IDs, internal traces, query sidecars,
  other seats' history or global private rule counters. Bearers stay in headers,
  never URLs/logs. Resource/clock faults are not game loss/draw.
- Keep registry/worker directories 0700 and ledgers/credentials 0600 regardless of
  umask. Verification children use process-local 0022 for permission fixtures;
  the driver stays 0077. Never change the global umask.
- Preserve original executables/data/evidence for exact recovery. Protocol
  scaffolding implies no deployment, lobby/account service or new profile admission.

Use pinned tools and the worktree Cargo lock from [development](../../docs/game-engine-development.md).
The default engine check covers the server library; for executable changes run
`cargo check --locked -p mtg-server --bin mtg-server`. Choose affected
`cargo test --locked -p mtg-server FILTER` / production Clippy, then relevant
isolated SDK checks, and confirm selectors execute tests. SDK generation never
starts Cargo; the [SDK manual](sdk/AGENTS.md) governs its lock/install.
