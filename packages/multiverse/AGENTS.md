# Multiverse product application

Read the root manual, [architecture](web/ARCHITECTURE.md) and
[design](web/DESIGN.md) before changing browser code. This package owns the product,
frontend build and process composition. [Application boundaries](web/ARCHITECTURE.md#application-boundaries)
define the public backend/game dependencies and [game launch](web/ARCHITECTURE.md#game-launch)
its gates. Game implementation belongs to `../game`; this package composes its
public exports in one deployment.

- Reuse the existing Actor/account system, storage paths, DECKBUILDER settings,
  credentials, MCP and public URLs. Browser requests go only through `web/src/api`.
- No private cross-package imports, copied display, inferred legality or card facts.
- Keep lazy loading, native modal containment, IME safety, text scaling, route
  focus and responsive access; preserve drafts/selection across remote updates,
  inspection and resizing.
- Product sign-in grants no Rust seat; never use the solo playtester as game-client
  design guidance.

[README](README.md) owns setup and checks. No product command starts Cargo or
regenerates native schemas/SDK. Browser checks use
[disposable actor-scoped fixtures](../deck-builder/browser-checks/README.md), never
hosted/owner data.
