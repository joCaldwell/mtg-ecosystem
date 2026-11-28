# Multiverse browser regressions

Disposable source-backed browser fixtures verify focus, layout, stored data and
accessibility. This nested dev package is outside the root workspace, package
`check` gates and Docker context. It builds `packages/multiverse`, uses backend
capabilities and never tests against production users or a live owner database.

## Setup and fixtures

From repository root, with the pinned Node launcher:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder/browser-checks ci --workspaces=false --ignore-scripts
```

The lock pins `playwright-core` and `axe-core` and downloads no browsers; use
matching installed Playwright browsers (`~/.cache/ms-playwright`). Automation
injects Axe, which is never a reason to relax CSP. Run the rest from this directory
under pinned Node; `CHECKS_ROOT` overrides the fixture root
`~/.cache/deck-builder-browser-checks`.

```sh
node seed.ts --source /absolute/disposable/card-source.db --images /absolute/frozen/oracle-cards.json
node serve.ts --port 8830
```

`seed.ts` takes an online read-only backup of the explicit `--source`, migrates only
its copy and keeps global card/name/price facts, never source users; the optional
Oracle corpus fills missing image URLs/artists. Output is `CHECKS_ROOT/seed/seed.db`
plus `seed.json` with actual fixture IDs. Personal records come from
capability/UI/MCP adapters: Rowan (user 1, admin) owns Paper, public, Theorycrafted
and Archived decks with proposals, memories and tokens; Sam (user 2) is empty;
Jordan (user 3) has the sample. Clock, randomness, locale, timezone and colour
scheme are fixed (time ends `2026-10-20T17:00:00Z`). `seed-long-notes.ts` builds
the long maybe-note fixture.

`serve.ts` copies the seed per start, disables background jobs and serves hosted
composition with fixture providers (raised rate limits; not live OAuth proof). The
`checks_user=<id>` cookie selects an actor; no cookie is signed out.
`--default-user 1` is for owner UX review only, never authentication or isolation
checks. Builds go to a temporary directory, never `web/dist`; `--dist DIR` reuses an
immutable build and `--build-only --build-dir DIR` prints a new build path. Use a
fresh fixture for every script that mutates data.

## Focused regressions

Each `check-*.mjs` header states its contract. Common flags: `--origin
http://127.0.0.1:8830` (a disposable loopback fixture), `--baseline ORIGIN` (a
separate immutable baseline fixture), `--browser chromium|firefox` and
`--out DIR`/`--report FILE` for ignored reports; read the source for others.

```sh
node check-product-shell.mjs --origin http://127.0.0.1:8830 --out ../.dev/product-shell-check
```

Hosted native game sessions use `../../game/browser-checks/check-product-session.ts --help`.
The backend's `scripts/check-deck-editor*.mjs` also consume this fixture. Accepted
frontend evidence lives in [client evidence](../../../docs/game-client.md#reproducibility-and-current-acceptance).

## Header scroll timing

Use immutable baseline/candidate builds and distinct ports:

```sh
node serve.ts --build-only --build-dir /tmp/NEW-deck-scroll-build
node check-header-scroll.mjs --port 8850 \
  --seed ~/.cache/deck-builder-browser-checks/seed-tokens/seed.db \
  --dist /tmp/NEW-deck-scroll-build/web/dist --out /tmp/NEW-deck-scroll-frames.json
```

The probe runs its own fixture and records rAF geometry, drift, stalls and
reversals across wheel, keyboard, focus reveal, resize and both motion preferences.
`--regressions-only` runs panel/inspection/reveal/scrollbar checks; `--only REGEX`
selects timing cases. Playwright wheel events are precise pixels; CDP wheel
emulation adds native notch animation ([input handler](https://github.com/chromium/chromium/blob/main/content/browser/devtools/protocol/input_handler.cc)).
Neither emulates physical devices. External images are blocked during timing.
Retain failed attempts and raw timing scope.
