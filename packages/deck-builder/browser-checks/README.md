# browser-checks: regression scripts for the deck-builder web app

Chromium scripts that drive a disposable, seeded copy of the app and assert on
real behaviour: focus, layout, stored data and accessibility. They never touch
live data. This is dev tooling with its own `package.json`. It sits outside the
npm workspace (`packages/*` does not match it), outside `npm run check`, and
outside the Docker context's allow-list.

## Dependencies

| Package | Why |
| --- | --- |
| `playwright-core@1.62.1` | Drives Chromium. Pinned to the release whose browsers are already in `~/.cache/ms-playwright` (`chromium-1234`, `chromium_headless_shell-1234`, `webkit-2336`), so installing downloads nothing large. `playwright-core` never downloads browsers itself. Change the pin only together with the cached browsers. |
| `axe-core@4.14.0` | Accessibility violations per screen, the version phase 4b's QA used. Injected with `page.evaluate`, which the page's CSP does not apply to, so the hosted CSP stays on. |

Vite comes from the deck-builder's own dev dependencies.

```
cd packages/deck-builder/browser-checks && npm ci
```

Node 26 (type stripping runs `seed.ts` and `serve.ts`, which import `../src`).

Set `CHECKS_ROOT` to override the default root, `~/.cache/deck-builder-browser-checks`.
Seed data and server working files live under this root and persist across
reboots.

## The pieces

### `node seed.ts` → `~/.cache/deck-builder-browser-checks/seed/seed.db`

A fresh database at the current schema. Card facts (`cards`, `card_names`,
`card_prices`, and the `last_ingest`/`prices_as_of` meta keys) are copied
from a `node:sqlite` `backup()` of
`t3-aa7eb73f/packages/deck-builder/.dev/owner-copy.db`, migrated by `openDb`.
No user row is copied. That snapshot has no image URLs or artists, so those
two facts are filled from the Scryfall oracle bulk file
`/home/josh/Code/mtg-ecosystem/.scryfall-cache/oracle-cards.json` through
the app's own `toRow()` (pass `--images <file>` to change it; without one,
card images show their placeholder). The migrated, enriched copy is cached in
`~/.cache/deck-builder-browser-checks/cache/`.

Everything else goes through the capability layer, owner actions as a UI
actor and agent writes through the MCP tool adapters (`callTool`) as agent
actors, so activity, attribution and lint apply:

- **Rowan** (user 1, `@rowan`, admin): six decks imported from
  `test/fixtures/moxfield/*.copy-plain-text.txt` with `createDeck` +
  `applyImport`. Najeela and Kinnan (Paper) have slots, tags, a brief, an
  engine, maybe lists, an accepted and a rejected decision, and open
  proposals from `claude-code` and `codex` (five card proposals and one brief
  edit). Hail Caesar is public with decisions published. Bombadil and
  Stickers are Theorycrafted (an agent edited Bombadil directly). Rograkh is
  Archived. Global and deck memories from agents and the owner, a profile,
  two agent tokens.
- **Sam** (user 2, `@sam`): an empty account for the first-run views.
- **Jordan** (user 3, `@jordan`): the sample deck from `src/demo.ts`.

Time is fictional and fixed: the timeline ends just before
`2026-10-20T17:00:00Z`. `seed.json` beside the database lists user and deck
ids. The seed fails if any timestamp carries the real date.


### `node serve.ts [--port 8830]`

`createApp` with no background jobs, in hosted mode, against a fresh copy of
the seed (so every start is the same state). The web app is built with
`vite build --outDir` into a temporary folder, never `web/dist`; pass
`--dist <dir>` to reuse a build, or `--build-only` to just build. Sessions
are mocked through `createApp`'s `resolveUiActor` seam, as
`test/sharing-http.test.ts` does: the cookie `checks_user=<id>` is that user,
no cookie is signed out. Real hosted auth is configured (Discord and Google
fixtures, a gated sign-up), so the sign-in page, legal footer and signed-out
public pages render as in production. Rate limits are raised in-process,

**Determinism.** In Node (`lib/determinism.ts`): `Date` and SQLite's
`datetime('now')` read a fixed clock; `Math.random` and `node:crypto`'s random
functions come from a seeded PRNG, so share slugs and tokens repeat. In the
browser (`lib/browser.mjs`): `Date` is frozen, `Math.random` seeded, and locale,
time zone and colour scheme fixed.

## The checks

Start a fresh `serve.ts` for each script that changes its fixture.

Desktop export states also cover the whole saved deck and a filtered maybe card
at each desktop width. `check-export.mjs` additionally exercises every export
option and a held response to verify stale scope text cannot be copied.
`check-search-scope.mjs` checks delayed query and colour-scope changes at all
four desktop widths, with only current options exposed to pointer and keyboard.

`seed-long-notes.ts` creates a separate capability-authored fixture with 20
maybe cards: 15 multi-paragraph linked notes, three blank notes, import origins
and costless cards. `check-long-maybe.mjs` compares row geometry at all four
desktop widths and actual 340/392/640 px panels, reads the full notes in details,
and checks links, Back, unsaved drafts, focus, order and exact stored data.
`check-full-lanes.mjs` drives native mouse drags across all three battlefield
lanes, including empty/populated centers and edges, child transitions,
cancellation, outside drops, exactly-once movement, undo and resume.

### `node check-panel-inspection.mjs [--origin http://127.0.0.1:8853] [--report <file>]`

Run against an existing `serve.ts` disposable fixture server. Exercises visible
panel controls, first-card return origin, draft/reason/engine DOM preservation,
scroll and focus, four desktop widths and the shared popup across resizing.
Escape leaves inspection before closing the sheet; Close closes it. Checks
that hidden content is inaccessible and deck data is unchanged. Refuses hosted
origins. The default seeded deck is 2; `--deck` can select an equivalent fixture.

### `node check-visual-stats.mjs [--origin http://127.0.0.1:8858] [--out <dir>]`

Uses an existing disposable `serve.ts` copy. Resolves chart fixtures against the
seed card database and creates fixture decks through the API. Checks independent
colour denominators, visible legends, mono/zero/any-colour/colourless/overlapping
source/alternative-face states, adaptive actual panels at four desktop widths,
accessibility and unchanged inspection data. Also checks Share controls after
prose removal. Refuses non-loopback origins. Start a fresh fixture server first.

`node check-maybe-rows.mjs --origin http://127.0.0.1:8853 --baseline http://127.0.0.1:8857`
checks whole-row pointer/keyboard activation, focus, nested note controls, main
modifier selection, persistent reason labels, and stored assignments/membership
through promotion and undo against two disposable fixture servers.

`node check-sidebar-search.mjs` checks the expanded disposable collection: full
name visibility, deck/commander queries, unsaved draft identity, Escape/empty
clear, visible lifecycle labels and collapsed accessible names, compact-rail
destinations and keyboard navigation at all four desktop widths.

`node check-filtered-maybe.mjs --origin http://127.0.0.1:8885` uses the long-note
fixture to check that one search result keeps its original column width. It
types real search keystrokes and checks hidden nonmatches, full-row pointer and
keyboard activation, complete notes, inspection, Back, drafts, selection, scroll
restoration and unchanged stored assignments/order at all four desktop widths.

`node check-playtest-geometry.mjs --origin http://127.0.0.1:8853 --baseline http://127.0.0.1:8855`
compares adaptive hand widths/row counts, commander rail height, inspector and
hover-preview width to an established build at all four desktop widths. It
checks keyboard selection/menu access, candidate axe and unchanged deck data.

`node check-review-clarity.mjs --origin http://127.0.0.1:8854`
checks selected proposal scope, native disclosure activation without leaving
Inbox, persistent reasons and a real selected-only decision. It compares
collection colour labels/counts to server statistics, checks slot help spacing,
and exercises mixed import provenance plus custom-note controls without changing

statistics, expanded pending forecasts and empty Inbox/Memories/Activity states.

`node check-hand-empty.mjs --origin http://127.0.0.1:8853`
exercises opening and kept-hand scrolling with native Enter/Space, verifies all
seven cards can be reached and the saved table frame stays unchanged, then checks
selection/menu access and empty Inbox/Activity/history recovery at four desktop
widths.
The hand QA additionally checks that a selected card's name, mana cost and type
stay within the hand viewport after native focus scrolling. Review-clarity QA
checks the actual stored agent reasons are visible next to selected changes.

`node check-settings-economy.mjs --origin http://127.0.0.1:8853`
checks price/currency and memory-budget saves with restoration, invalid-budget
feedback, setup-disclosure keyboard behavior and unsaved token labels at every
desktop width. A regular account verifies open initial instructions and disabled

fixture; its secret is never logged or captured.

### Header scroll timing

Evaluates the header/list hand-off with native input, against immutable
baseline and candidate builds:

```sh
npx vite build --outDir /tmp/deck-scroll-dist --emptyOutDir
node browser-checks/check-header-scroll.mjs --port 8850 \
  --seed ~/.cache/deck-builder-browser-checks/seed-tokens/seed.db \
  --dist /tmp/deck-scroll-dist --out /tmp/deck-scroll-frames.json
```

Run it against immutable baseline and candidate builds. It starts `serve.ts`
against a copy of the seed, enables Chromium smooth scrolling and shows native
scrollbars. Each rAF records header position, compact opacity/visibility,
toolbar position, list offset, an anchor row and the first visible row. The
summaries include largest frame movements, reversals, interior stalls,
hand-off neighbourhoods, row/column changes, range drift and browser timing.
Scenarios cover one notch, accumulating notches, 4 px every frame, reverse,
mid-list, keyboard, native focus reveal and resize, with both motion preferences.
Extra assertions exercise panel-only scrolling, card inspection, find/clear
restoration, review keys and dragging the native scrollbar. Use
`--regressions-only` for those assertions alone, or `--only <regex>` to select
`input-mode/motion-preference/scenario` timing cases.

Playwright's `page.mouse.wheel` always sends Chromium **precise pixel** input:
even a 120 px event bypasses native notch animation. The probe keeps those
real wheel events for direct-input measurements, and also uses CDP's pixel-wheel
path (`Input.emulateTouchFromMouseEvent`, wheel events only) to exercise the
browser's native animator. Its sign is opposite Playwright's. The distinction
comes from Chromium's [input handler](https://github.com/chromium/chromium/blob/main/content/browser/devtools/protocol/input_handler.cc).
Neither path is a physical mouse or trackpad; validate hardware feel separately.
External images are blocked in timing runs to isolate layout and input.

`check-copies.mjs`, `check-card-controls.mjs` and `check-selects.mjs` cover the
card panel's Copies stepper, eligible Board roles and dropdown keyboard use.
`check-agent-contact.mjs` and `check-agent-workflows.mjs` are described in
`docs/deck-builder-mcp-workflows.md`. `scripts/check-deck-editor.mjs` and
`scripts/check-deck-editor-ux.mjs` run against this server too.
