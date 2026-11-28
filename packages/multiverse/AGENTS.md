# Multiverse product application

This package owns the product, frontend build and process composition; read the
root manual, [architecture](web/ARCHITECTURE.md) and [design](web/DESIGN.md) before
changing browser code. [Application boundaries](web/ARCHITECTURE.md#application-boundaries)
define the public backend/game dependencies and [game launch](web/ARCHITECTURE.md#game-launch)
its gates; `../game` owns game implementation, composed here through public exports.

- Reuse the existing Actor/account system, storage paths, DECKBUILDER settings,
  credentials, MCP and public URLs. Browser requests go only through `web/src/api`.
- No private cross-package imports, copied display, inferred legality or card facts.
- Keep lazy loading, native modal containment, IME safety, text scaling, route
  focus and responsive access; preserve drafts/selection across remote updates,
  inspection and resizing.

The [README](README.md) owns setup and checks; no product command starts Cargo or
regenerates native schemas/SDK. Browser checks use
[disposable actor-scoped fixtures](../deck-builder/browser-checks/README.md), never
hosted/owner data.
