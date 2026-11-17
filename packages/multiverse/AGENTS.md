# Multiverse product application

Read the root manual, [architecture](web/ARCHITECTURE.md) and
[design](web/DESIGN.md) before changing browser code. This package owns the product,
frontend build and process composition; architecture's
[application boundaries](web/ARCHITECTURE.md#application-boundaries) define the
public backend/game dependencies and [game launch](web/ARCHITECTURE.md#game-launch)
retains qualification gates. Game implementation belongs to `../game` (`web/`,
`server/`, `shared/`); this package composes its public exports in one deployment.

- Reuse the existing Actor/account system, storage paths, DECKBUILDER settings,
  credentials, MCP and public URLs. Browser requests go only through `web/src/api`.
- Backend runtime uses public `./host`; browser types use type-only `./contracts`.
  Game consumers use public preview/game-ui exports, scoped CSS and host React.
  No private cross-package imports, copied display, inferred legality or card facts.
- Keep lazy loading, native modal containment, feature-detected overlay fallbacks,
  IME safety, quiet validation, text scaling, route focus and responsive access.
  Preserve drafts/selection across remote updates, inspection and resizing.
- The captured preview is separate from selected library decks and actual games.
  Product sign-in grants no Rust seat; never use the solo playtester as game-client
  design guidance.

[README](README.md) owns pinned setup and package checks. No product command starts
Cargo or regenerates native schemas/SDK. Integration browser checks use
[disposable actor-scoped fixtures](../deck-builder/browser-checks/README.md), never
hosted/owner data. Keep current contracts in the owning sections; historical
acceptance belongs in [client evidence](../../docs/game-client-evidence.md).
