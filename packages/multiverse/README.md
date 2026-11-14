# Multiverse application

The product application owns account UX, navigation, Home, the deck editor,
game setup and the compact game frame. Read [AGENTS.md](AGENTS.md),
[frontend architecture](web/ARCHITECTURE.md) and the
[application boundaries](../../docs/multiverse-app.md) before changing it.

The existing account/auth, Actor/SQLite, deck and MCP backend is consumed through
`@mtg-ecosystem/deck-builder/host` and type-only `/contracts`. Data paths, existing
DECKBUILDER environment settings and credentials retain their meaning. There is
no second login or database migration.

## Local setup

From the repository root, use the pinned Node launcher and isolated locks:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-ui ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/multiverse ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/multiverse run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/multiverse run build:web --workspaces=false
```

`npm --prefix packages/multiverse run dev --workspaces=false` runs the normal
development application. Existing backend-package launch commands delegate to
the product manager. Use a disposable explicit DB/DATA path for testing; never
point experiments at hosted or owner data. Relative storage settings and the
existing backend `.dev` PID/log locations remain preserved by the manager.

The product build checks exact preview provenance without requiring generated
native/SDK artifacts. The standalone game-client harness retains its separate
qualified SDK setup and checks; no application build starts Cargo.

## Current flow

Home at `/` links to `/decks` and `/play`; selecting a real owned deck carries its
saved revision into setup. `/play/preview` embeds the same shared table as the
standalone client, using a qualified captured sample. It does not use the selected
deck or start a game. Real automated-game launch needs the account-to-seat,
admission and supervision services described in the application boundaries.
