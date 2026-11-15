# Multiverse browser regressions

Disposable source-backed browser fixtures verify focus, layout, stored data and
accessibility. This nested dev package is outside the root workspace, package
`check` gates and Docker context. It builds `packages/multiverse` and uses backend
capabilities; it never tests against production users or a live owner database.

## Setup and fixtures

From repository root, use the pinned Node launcher:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder/browser-checks ci --workspaces=false --ignore-scripts
```

The local lock pins `playwright-core` and `axe-core`; it downloads no browsers.
Use matching installed Playwright browsers (`~/.cache/ms-playwright`), changing
the package pin and browser installation together. Axe is injected by automation,
not a reason to relax the application's CSP. Vite and the public-bundle checker
come from the product package.

Run the remaining examples from `packages/deck-builder/browser-checks`, under
pinned Node. Set `CHECKS_ROOT` to change the default persistent fixture root
`~/.cache/deck-builder-browser-checks`.

```sh
node seed.ts --source /absolute/disposable/card-source.db --images /absolute/frozen/oracle-cards.json
node serve.ts --port 8830
```

`seed.ts` takes an online read-only backup of the specified source, migrates only
its cached copy and copies global card/name/price facts, never source users. The
optional Oracle corpus fills absent stored image URLs/artists through `toRow`;
without it images retain placeholders. Default historical source paths are in
the script; supply `--source` explicitly for a new checkout. Output is
`CHECKS_ROOT/seed/seed.db` plus `seed.json` with actual fixture IDs.

All personal records are created through capability/UI/MCP adapters. Rowan
(user 1, admin) has six source-export decks: Paper decks with slots/tags/proposals,
a public deck with published decisions, Theorycrafted and Archived examples,
memories/profile/tokens. Sam (user 2) is empty; Jordan (user 3) has the labelled
sample. Time ends at `2026-10-20T17:00:00Z`; clock, SQLite time, randomness and
browser locale/timezone/colour scheme are fixed. Seed rejects real timestamps.
`seed-long-notes.ts` supplies the separate 20-maybe-card note/blank/import fixture.

`serve.ts` copies the seed for each start, disables background jobs and serves
hosted composition with fixture providers. The `checks_user=<id>` cookie selects
an actor; no cookie is signed out. `--default-user 1` is for owner UX review only:
never use it for authentication/isolation checks. Production exposes no such flag.
Rate limits are raised only inside the fixture. This is not live OAuth proof.

The server builds into a temporary directory, never product `web/dist`.
`--dist DIR` reuses an immutable build; `--build-only --build-dir DIR` prints a
new build's dist path without serving. Keep builds unchanged while checks use
them. Start a fresh fixture for every script that mutates its data.

## Focused regressions

Most scripts take `--origin http://127.0.0.1:8830`; read the source for additional
flags/defaults. Use only a disposable loopback fixture and keep reports ignored.
Scripts with `--baseline` require a separate immutable baseline fixture.

| Script | Contract |
| --- | --- |
| `check-product-shell.mjs` | Home, selected-deck setup, qualified preview/drafts/focus/return, deep links, delayed reads, account isolation and desktop/tablet/phone navigation; no game commands or saved-deck change |
| `check-product-session.ts --help` | Hosted native sessions: exact journals, Chromium/Firefox recovery; opt-in natural play bounded at 90 minutes/2,000 human commands |
| `check-modern-primitives.mjs` | Native and missing-feature overlay/menu/checkbox paths, safe autofocus, focus return, labelled triggers, text scaling and toast replacement; run with `--browser chromium` and `--browser firefox` |
| `check-modern-navigation.mjs` | Chromium/Firefox route titles/focus, native document links, pointer/keyboard hover previews, IME Escape, missing public routes and consent; optional `--out DIR` |
| `check-feature-modernization.mjs` | Native submit validation, IME handlers, keyboard AddBar actions and reversed async deck command completions |
| `check-panel-inspection.mjs` | Original panel/draft/reason DOM, focus/scroll, first inspection return, distinct sheet Escape/Close, hidden accessibility, four desktop widths; `--report FILE`, optional `--deck ID` |
| `check-selects.mjs`, `check-copies.mjs`, `check-card-controls.mjs` | Dropdown keyboard/toggle contracts, Copies and server-supplied Board roles |
| `check-export.mjs` | Whole/filtered/maybe exports, every option and held-response scope safety |
| `check-search-scope.mjs` | Delayed query/colour-scope changes expose only current keyboard/pointer options |
| `check-long-maybe.mjs`, `check-filtered-maybe.mjs` | Long-note fixture at four desktop widths and actual 340/392/640 px panels; complete notes, original column width, drafts/links/Back/focus/scroll and stored data |
| `check-maybe-rows.mjs` | Whole-row pointer/keyboard activation, nested controls, main modifier selection, promotion/undo and stored assignments; `--baseline ORIGIN` |
| `check-sidebar-search.mjs` | Full deck/commander names, clear/empty state, lifecycle labels, collapsed access and unsaved editor identity |
| `check-review-clarity.mjs` | Selected proposal/decision reasons, native disclosures, collection stats, slot help, import-note controls, forecasts and empty secondary pages |
| `check-visual-stats.mjs` | Independent colour denominators/legends, empty/mono/flexible/overlapping/alternate-face source states, actual panel widths, accessibility and unchanged inspection data; `--out DIR` |
| `check-settings-economy.mjs` | Price/currency and budget saves/restoration, invalid inputs, setup disclosures, token-label drafts and regular-account access |
| `check-hand-empty.mjs` | Native hand keyboard reveal and selected-card bounds, stable frame, menu access and empty-page recovery |
| `check-playtest-geometry.mjs` | Hand/rail/inspector/hover geometry, keyboard access, axe and unchanged saved table; `--baseline ORIGIN` |
| `check-full-lanes.mjs` | Native mouse drags through all battlefield lanes, child transitions/cancellation/outside drops, exactly-once movement, undo/resume |
| `check-agent-contact.mjs`, `check-agent-workflows.mjs` | Read-only contact, tenant separation, credential aging/revocation, scoped updates, tab ownership and exact retry after dropped response |

For example:

```sh
node check-product-shell.mjs --origin http://127.0.0.1:8830 --out ../.dev/product-shell-check
node check-modern-primitives.mjs --origin http://127.0.0.1:8830 --browser chromium
```

The backend's `scripts/check-deck-editor.mjs` and `scripts/check-deck-editor-ux.mjs`
also consume this fixture. Game-client production browser checks stay in that
package; authenticated evidence has separate privacy and source-qualification
requirements. Historical frontend acceptance is retained in
[client evidence](../../../docs/game-client-evidence.md), not inferred from these
command examples.

## Header scroll timing

Use immutable baseline/candidate builds and distinct fixture ports. From this
directory:

```sh
node serve.ts --build-only --build-dir /tmp/NEW-deck-scroll-build
node check-header-scroll.mjs --port 8850 \
  --seed ~/.cache/deck-builder-browser-checks/seed-tokens/seed.db \
  --dist /tmp/NEW-deck-scroll-build/web/dist --out /tmp/NEW-deck-scroll-frames.json
```

The probe starts/stops its own fixture, enables smooth scrolling/native scrollbars
and records rAF geometry, offsets, anchor rows, range drift, stalls and reversals.
It covers one/accumulating notches, precise 4 px frames, reverse/mid-list, keyboard,
focus reveal, resize and both motion preferences. `--regressions-only` selects
panel-only scrolling, inspection, filter/review reveal and native scrollbar checks;
`--only REGEX` selects timing cases.

Playwright wheel events are precise pixels even at 120 px; CDP's wheel emulation
also exercises native notch animation with opposite sign, per Chromium's
[input handler](https://github.com/chromium/chromium/blob/main/content/browser/devtools/protocol/input_handler.cc).
Neither emulates physical mouse/trackpad feel. External images are blocked during
timing to isolate input/layout. Retain failed attempts and raw timing scope.
