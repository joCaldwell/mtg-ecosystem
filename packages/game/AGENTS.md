# Game application

Read root guidance, [client contracts](../../docs/game-client.md) and [setup](README.md).
This application owns `web/` (decision/session UI and preview), `server/`
(launch/recovery, native/bot lifetime and human-seat proxy) and `shared/`
contracts; game-ui owns display, Rust owns gameplay.

Server code consumes public deck-builder `./application` ports (existing Actor,
SQL gate/transaction, account checks, owned deck capture); never import private
deck/auth SQL. Multiverse composes this application in the existing process: no
second login, database or deployment. Migrations and deletion stop handoffs stay
in deck-builder. Browser code cannot import server modules.

## Boundaries

- `hosted` consumes injected product HTTP/navigation/UI ports; launch, saved records
  and live session entrypoints stay separate so setup never loads the SDK.
- `app` composes routes/lifetime; components never fetch or access storage.
- `session` owns SDK effects, authorization, exclusive ownership and recovery.
  Pure modules inject effects without browser/React/SDK runtime dependencies.
- `decisions` owns local drafts over supplied candidates/constraints and original
  context. Never infer legality, costs, payment, combat or source meaning from text.
- `presentation` transforms permitted projections/producer-owned public metadata;
  no executable IR, snapshots, source fetching or private fixtures.
- `features` consumes public game-ui exports; never duplicate table components or
  import deck-builder internals, SDK example policies or compiler internals.
- Router-free `/preview` imports no app/session/auth/SDK runtime; Node-only
  `/verify-preview` is never imported by browser code.
- Fixtures keep exact producer facts/provenance. Never invent names, printed
  characteristics, images, Oracle text, source labels or missing decision context.

## Constraints

Use the README's pinned isolated installs; no package command may build Cargo,
native schemas or SDK artifacts, and there is no default full native matrix. Use
one explicitly qualified SDK throughout checks. Session work keeps the
[authorization](../../docs/game-client.md#authorization-reads-and-ownership) and
[command journal](../../docs/game-client.md#durable-command-journal) contracts
intact. Preserve keyboard focus, reduced motion, text fallbacks and remote-update
focus ownership.
