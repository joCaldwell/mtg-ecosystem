# Game Server (Layer2)

The original verified P8 release hosts the finite `commander-foundations-v1` profile over
loopback HTTP, with strict Rust schemas, a generated TypeScript SDK, a fenced
SQLite ledger and an exact-operation durable worker. An actual four-client game,
crash/reconnect, backpressure and completed-game startup recovery passed.
That P8 build
`7e77708cad1f80dfa7ad3be82d198f3ad6eae7f62b9c127879a335206bbc8da5`
also passed all sixty quiet primary search jobs and independent semantic replays:
p95 2,662,119µs, maximum 2,690,433µs, kernel whole-process peak 394,698,752 bytes,
4,188 useful trials, and 3,012 unstarted trials excluded. This is the bounded
search workload, not a full-pod throughput or scaling measurement.

Final check-engine verification passed 526 Rust debug tests and 40 Node checks;
release verification passed 526 Rust tests; the SDK passed 40 tests and strict
TypeScript. Engine offline retention passed, followed by server/SDK offline
reconstruction and independent native/server audits. Two host-only fixes preserve
npm 12's command-specific tarball opt-in and the SDK tests' exact native catalog
fixture. Fourteen focused host guards passed. All 192 native / 23 SDK sources
remain exact; reused phase proofs retain their original freeze/log identities.

Final gate status SHA256:
`da1c4f116af4c7a6c2fd505f8dcbd3f5a16bebefa0391098df875b6aa3aebbc6`.
All proofs are retained privately outside the worktree under
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-11fdee9d9b50bb8d-2Y0CLl/server-proof-archive/`.
Archive manifest SHA256:
`a11f203b1cff1334c57e13655a7f93549c138e77dd00c69596238085d600952a`.

Read [AGENTS.md](AGENTS.md) for commands and invariants and the
[implementation contract](../../docs/game-engine-server.md) for authority,
recovery, clocks, selected limits and verification status. The server links
Rust core directly; it does not expose search or consume parser source at runtime.

The SDK has an isolated locked Node tool package. Its
[manual](sdk/AGENTS.md) documents generation, strict type checking and tests.

After the full engine retention gate, `npm run retain:server` independently
rebuilds and tests the server offline with a fresh Cargo home/target, checks its
compiled engine identity and linked SQLite options, then reconstructs the SDK
from its own pinned sources/dependency cache and runs generation, strict type
checks and runtime tests offline. This separate client closure is not part of
the Rust native-source hash. Retained reports are generated artifacts.

P8.2 additionally hosts `commander-attachments-v1`, with an SDK completion
consumer for the default white / black-green / blue / red pod. All four frozen
acceptance items passed: the actual four-process game, normal completed-game
recovery, 95 server tests, and SDK/engine checks. Foundations schema bytes and
all hosting limits remain unchanged. See the
[frozen P8.2 contract](../../docs/game-engine-server-attachments.md) and
[measured results and retained proofs](../../docs/game-engine-progress.md#p82--attachments-hosting-results-2026-10-24).
The earlier figures above remain qualified to their original P8 executable.
