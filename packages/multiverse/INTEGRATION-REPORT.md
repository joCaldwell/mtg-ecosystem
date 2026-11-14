# Unified product ownership checkpoint

Source prerequisite: workspace UX `a80c2f5251c07de92245d085b63fd7c5f4937ccb`,
followed by the current-main integration on `t3/game-client-workspace-ux`.
The isolated worktree is
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-7ab024bd-7766f83a106e0c59`.

## Delivered ownership and behavior

- `packages/multiverse` owns the complete browser application, account/session
  UX, product navigation, Home, deck editor, Play setup, compact preview, build
  and process entrypoint. The browser move preserves one implementation rather
  than copying the editor or introducing shell/feature dependency cycles.
- The existing backend account/auth, Actor/SQLite, deck capabilities and MCP stay
  under `deck-builder`, behind public host and type-only contract exports.
  Account backend extraction is not claimed. No account/data migration occurred;
  default storage, DECKBUILDER settings, dotenv lookup, manager PID/log paths,
  relative storage semantics, tokens and public URLs remain preserved.
- Home uses actual account-scoped deck information. Setup selects an owned deck
  and displays its real revision/counts. Unreadable decks never substitute another
  selection; a late prior read cannot replace the newly selected deck.
- Three automated seats define the initial game direction. Actual Start game is
  explicitly unavailable until admission/provisioning and complete browser play
  exist. The separate preview uses a qualified captured sample, never the chosen
  deck, and creates no match, game request or persistent game state.
- `game-ui` provides the shared permitted table/inspection display with React
  peers and scoped `--game-*` styling. Both product and standalone client consume
  it. The router-free client preview has a component-only public declaration;
  generated protocol types and session effects stay private to their lane.
- Product builds verify tracked fixture hashes and catalog/viewer consistency.
  The original SDK/native schema qualification remains separate and unchanged.

## Executed verification

All package commands used pinned Node through `scripts/engine-env.sh`, isolated
dependencies and package locks. No Cargo or native schema generation ran.

| Check | Result |
| --- | --- |
| Backend `check` | Strict types and 103 tests passed |
| Product `check` | Server/browser types and 31 tests passed |
| Shared game UI `check` | Types, boundary guard and 3 tests passed |
| Client `check` | Types, boundaries, qualified fixtures, 116 tests and build passed |
| Standalone production browser checks | 28 Chromium/Firefox checks passed |
| Integrated product browser checks | Seven groups passed: account/deck setup, preview/draft/focus/return, deep links, delayed deck reads, cross-account denial, signed-out gating, desktop/tablet/phone access |
| Product production build/public budget | Build passed; public closure 104,509 gzip bytes, below existing 110,000-byte gate |
| Clean product consumer | Types and production build passed with SDK/client generated directories absent; all held bytes restored identically |
| Runtime import audit | Backend 389 imports and product 3 imports passed |
| Moby Docker context | 604 included files and 11 sensitive/generated exclusion checks passed |

The final browser fixture uses a disposable source-backed seed copy and mocked
test actors, rather than production auth or owner records. The optional review
server's default actor is a fixture-only option; production has no such setting.
Screenshots were inspected at desktop and phone sizes. Shared controls were also
inspected against the host reset: visible button borders/padding and original
card padding remain intact. The game chunk stays lazy.

Independent reviews found and closed a missing second-consumer provenance gate,
a host CSS reset conflict and a pending-start/close lifecycle race. The lifecycle
regression proves both callers settle, the listener remains closed and the data
lease can be reacquired. The earlier gallery focus regression remains covered.

## Evidence and omissions

Final command logs are `generated/product-{backend,app,ui,game-client}-final-check.log`
and `generated/product-game-browser-final.log`. The clean-consumer proof is
`generated/multiverse-clean-consumer-59576cab.json`; earlier clean-consumer logs
remain preserved. Integrated browser report/screenshots are under
`packages/deck-builder/.dev/product-shell/final-top-level/`. Runtime/context logs,
initial typecheck failures and the lifecycle probe remain in their generated or
package `.dev` evidence directories. Initial failures were preserved and fixed.

The actual Docker image build was not run successfully: this environment has no
Docker daemon/socket (`generated/product-docker-build.log`). No deployment,
native host game, automated match launch, full-game browser play, dense-board
acceptance or strategic-opponent quality is claimed. Those require the explicit
coordinator and admission work in `docs/multiverse-app.md`.
