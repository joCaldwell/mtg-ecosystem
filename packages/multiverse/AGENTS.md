# Multiverse product application

Read the root manual and this package's `web/ARCHITECTURE.md` and `web/DESIGN.md`.
This project owns the product account UI/session, navigation, Home, game entry,
shared browser infrastructure and the named deck-building features it hosts.

- One account boundary: reuse the existing backend's authenticated Actor and data.
  Browser calls go through `web/src/api`; no component fetches.
- Backend types come only through the type-only public
  `@mtg-ecosystem/deck-builder/contracts` export. Product runtime composes the
  existing backend through its public host export. No cross-package private imports.
- Game presentation uses public `@multiverse/game-client/preview` or
  `@multiverse/game-ui` exports. Engine semantics, admission and session recovery
  retain their existing authorities; do not infer supported cards or actions.
- Preserve existing account/database paths, DECKBUILDER environment settings,
  personal tokens, MCP endpoints and public URL formats. No storage migration is
  implied by moving browser ownership.
- The table preview uses a qualified captured sample, not a selected library deck.
  Real game creation stays unavailable until an authenticated coordinator exists.
- Do not use the solo playtester guidance or implementation as design references.
- Follow the frontend layers, public feature barrels, tokens and approved
  breakpoints. Keep game loading lazy and styles scoped. Preserve keyboard focus,
  reduced motion, responsive access and the phone read-and-decide boundary.

Use pinned Node with `sh scripts/engine-env.sh` from the repository root. Local
package installs use their own lockfile and `--workspaces=false`; never replace
the root shared node_modules. `check` runs server/browser types and product tests;
`build:web` produces the browser bundle. No product build invokes Cargo or
regenerates native schemas/SDK artifacts. Browser integration checks use a
disposable actor-scoped fixture, never hosted user data.
