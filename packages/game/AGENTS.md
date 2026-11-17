# Game application

Read root guidance, [client contracts](../../docs/game-client.md) and [setup](README.md).
This application owns `web/` (existing decision/session UI and preview), `server/`
(launch/recovery, native/bot lifetime and human-seat proxy), and `shared/` contracts;
game-ui owns display, Rust owns gameplay. Opening choices are qualified; later
requests/full games retain explicit gates. [Evidence](../../docs/game-client-evidence.md)
binds sources and artifacts separately from current behavior.

Server code consumes public deck-builder `./application` ports: the existing Actor,
SQL gate/transaction, human/account checks and owned deck capture. Never import
private deck/auth SQL. Multiverse composes this application in the existing process;
no second login, database or deployment. Historical migrations and deletion stop
handoffs remain in deck-builder. Browser code cannot import server modules.

## Boundaries

- `hosted` consumes injected product HTTP/navigation/UI ports; keep launch, saved
  records and live session entrypoints separate so setup never loads the SDK.
- `app` composes routes/lifetime; components never fetch or access storage.
- `session` owns SDK effects, authorization, exclusive ownership and recovery.
  Pure modules inject effects without browser/React/SDK runtime dependencies.
- `decisions` owns local drafts over supplied candidates/constraints and original
  context. Never infer legality, costs, payment, combat or source meaning from text.
- `presentation` transforms permitted projections/producer-owned public metadata;
  no executable IR, snapshots, source fetching or private fixtures.
- `features` consumes public game-ui exports; never duplicate table components or
  import deck-builder internals/SDK example policies/compiler internals.
- Router-free `/preview` imports no app/session/auth/SDK runtime. Its public types
  hide generated inputs while package checks prove implementation parity. Node-only
  `/verify-preview` verifies historical provenance; never import it in browser code.
- Fixtures retain exact producer facts/provenance. Never invent names, printed
  characteristics, images, Oracle text, source labels or missing decision context.

## Toolchain and checks

Use pinned Node and isolated package lock/install from README, always
`--workspaces=false`; never replace root dependencies. No package command may
implicitly build Cargo, native schemas or SDK artifacts. `check` owns types,
fixture/qualification, pure tests, transitive public import boundaries and build.
Browser fixture/live suites have distinct source/host requirements in README.
Authenticated traces/HAR/video/screenshots stay off; retain failures and scrubbed
browser/version/source receipts. No default full native matrix.

## Live integration constraints

Use one explicitly qualified SDK throughout checks; historical fixture bytes do
not change current native pins. Match configured upstream/Host/Origin and verified
viewer/build/catalog before any operation. Import one seat via picker, keep bearer
only in memory, and reimport after refresh. Acquire Web Locks before session I/O;
no forced takeover, secret broadcasts or ownership fallback. Fence all effects by
authorization/generation. Retain original draft CommandContext and journal exact
bytes durably before send; storage failure blocks sending. Unresolved records bind
origin/match/seat before build matching. Local stop, missing status or timeout never
settles ambiguity. [The session contract](../../docs/game-client.md#durable-command-journal)
owns exact limits/retry/disposition rules and must remain intact.

Preserve keyboard focus, reduced motion, text fallbacks and remote-update focus
ownership. Publish focused checkpoints/draft PRs with actual checks and omissions;
no incidental deployment or merge.
