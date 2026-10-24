# Hosting `commander-attachments-v1` — P8.2

The local server (docs/game-engine-server.md) hosts only
`commander-foundations-v1`, a 34-definition pool. Since then the engine has
gained seven legal profiles; the richest, `commander-attachments-v1`
(docs/game-engine-attachments.md), has 118 definitions and four decks in four
colors. This slice lets the same server host that profile, so remote agents
using the TypeScript SDK can play the full current card pool. Foundations
hosting stays exactly as verified. No hosting limit, deadline or guard is
relaxed.

## Scope

- **Registry.** It admits exactly two hosted profiles: foundations and
  attachments. Every other legal profile stays rejected with the existing
  typed error. Registration, identity binding, credentials, recovery, clocks
  and resource groups are unchanged; profile and catalog are already pinned
  per match.
- **Wire schema.** The server protocol already embeds the core
  `Observation`, `PlayerRequest` and `PlayerResponse` types. The SDK's
  generated declarations must cover every request and response variant this
  profile can produce. Its exhaustive response gate (sdk/tests/types.ts,
  scripts/verify.mjs) must list them.
- **SDK consumer policy.** A new SDK example policy plays a complete
  attachments-profile game using only its own seat's observations and the
  public manifest/catalog. It must answer every request variant the profile
  can issue: mulligans, priority casts and activations (including equip and
  mana abilities), targets, mana payment and color choices, spending,
  attackers, blockers (using the legality hints), damage and trample
  assignment, searches, optional recursion, commander returns and discards. It
  is completion machinery, not a strategic agent. It may follow the simulator's
  develop policy, but must not import Rust or read private data.
- **Privacy.** Search requests, hands and library contents appear only in the
  owning seat's stream. Attachments, granted keywords, counters and tokens
  appear in public projections.

## Acceptance

Use the procedure that verified foundations hosting, unchanged:

1. **Actual hosted game.** Four independent Node SDK client processes play
   one admitted default pod (white / black-green / blue / red) against the
   real supervisor, under the existing 25-minute watchdog.
   - The game must end naturally: a typed win or draw at all four clients,
     zero retries and no CoreConcede.
   - Record turns, accepted inputs, the outcome, the final state hash and the
     worker's peak memory under the 4 GiB / swap-zero / tasks-32 limits.
2. **Completed-game recovery.** Normal startup against a fresh private copy
   of the closed game must reach Ready within the unchanged 120-second
   deadline. All durable tables and the head must match; only generation
   advances.
3. **Server tests.**
   - Registry admits the attachments profile and still rejects the others.
   - Worker create/recover round-trips an attachments-profile match.
   - Strict decoding covers the new request and response variants.
   - Search privacy holds across seat streams.
4. **Checks.** SDK verification passes: tests, strict TypeScript and the
   exhaustive gate. `npm run check:engine` passes. Foundations server tests and
   the public foundations schema behaviour are unchanged.

If any of these fails, record the failure with its evidence; failed attempts
stay preserved, as in the original P8 record. A failed time or memory limit is
a performance result, not a reason to change the limit.

- **Engine cost:** if the cause is engine cost (for example, recovery
  re-execution or state hashing over a larger state), report the measured
  phase and stop, rather than optimizing speculatively.
- **Host or harness cost:** if the cause is the host or harness, fix it only
  if it is a defect, and say which it was.

Out of scope: hosting the other profiles, server-side search, deployment,
lobbies, matchmaking, spectators, and changing hosting limits.
