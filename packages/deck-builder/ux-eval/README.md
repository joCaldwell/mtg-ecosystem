# ux-eval: the measurement harness for UX hill-climbing

Deterministic captures of the deck-builder web app, hard metrics, and a kit
for blind judging. It measures; it changes no app code. The orchestration of
judges and the fixer comes later and builds on these commands.

This is dev tooling with its own `package.json`. It sits outside the npm
workspace (`packages/*` does not match it), outside `npm run check`, and
outside the Docker context's allow-list.

## Dependencies

| Package | Why |
| --- | --- |
| `playwright-core@1.62.1` | Drives Chromium. Pinned to the release whose browsers are already in `~/.cache/ms-playwright` (`chromium-1234`, `chromium_headless_shell-1234`, `webkit-2336`), so installing downloads nothing large. `playwright-core` never downloads browsers itself. Change the pin only together with the cached browsers. |
| `axe-core@4.14.0` | Accessibility violations per screen, the version phase 4b's QA used. Injected with `page.evaluate`, which the page's CSP does not apply to, so the hosted CSP stays on. |

Vite comes from the deck-builder's own dev dependencies.

```
cd packages/deck-builder/ux-eval && npm ci
```

Node 26 (type stripping runs `seed.ts` and `serve.ts`, which import `../src`).

Set `UX_EVAL_ROOT` to override the default root, `~/.cache/deck-builder-ux-eval`.
All runs, packets, seed data, caches, builds and server working files live under
this root and persist across reboots.

## The pieces

### `node seed.ts` → `~/.cache/deck-builder-ux-eval/seed/seed.db`

A fresh database at the current schema. Card facts (`cards`, `card_names`,
`card_prices`, and the `last_ingest`/`prices_as_of` meta keys) are copied
from a `node:sqlite` `backup()` of
`t3-aa7eb73f/packages/deck-builder/.dev/owner-copy.db`, migrated by `openDb`.
No user row is copied. That snapshot has no image URLs or artists, so those
two facts are filled from the Scryfall oracle bulk file
`/home/josh/Code/mtg-ecosystem/.scryfall-cache/oracle-cards.json` through
the app's own `toRow()` (pass `--images <file>` to change it; without one,
card images show their placeholder). The migrated, enriched copy is cached in
`~/.cache/deck-builder-ux-eval/cache/`.

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
`test/sharing-http.test.ts` does: the cookie `uxeval_user=<id>` is that user,
no cookie is signed out. Real hosted auth is configured (Discord and Google
fixtures, a gated sign-up), so the sign-in page, legal footer and signed-out
public pages render as in production. Rate limits are raised in-process,
because a capture makes far more requests per minute than a person.

### `node capture.mjs` → `~/.cache/deck-builder-ux-eval/runs/<random-id>/`

Builds the web app (or `--dist`), then starts and restarts `serve.ts`
itself: once for the read-only screens, and fresh for every state sequence
that writes (the import dialog) and for every flow. Options: `--port`,
`--seed`, `--dist`, `--out`, `--only <regex over keys>` (flows are
`flow:<name>`).

For large-screen layout checks, `--wide` also enables 1920 × 1080 and
2560 × 1440 app captures. Combine it with `--only`, for example
`--wide --only '^(decks|settings|profile|activity|memories)--.*--(1920|2560)$'`.
The default capture suite and its comparable metrics stay unchanged.

```
<run>/screens/<key>.png         first viewport
<run>/screens/<key>.full.png    full page, when the window scrolls past one viewport
<run>/flows/<flow>/<NN>.png     each step of a task
<run>/manifest.json             key, screen, state, caption, viewport, files; flows with step captions
<run>/metrics.json              per-screen and per-flow metrics, and a summary
<run>/run.json                  private: commit, time, arguments. Never given to judges.
```

Keys are `<screen>--<state>--<width>` (`deck--card-selected--1440`) and
`<flow>/<NN>`. They never change between runs, so runs pair screen by screen.

**Screens** at 375, 768 and 1280 px, plus 1440 for the signed-in app:
sign-in; first run (empty account); all decks; a Paper deck by default, with a
card selected, reviewing proposals, scrolled to the maybe list, filtered by a
search, and with the slots dialog open; the import dialog empty, with a pasted
preview, and after applying; inbox, memories, activity, settings, profile;
the sample deck; the public deck page and a public profile, signed out. Desktop
playtest captures cover New game, the opening hand, the kept table, selected
card inspection and the ordered library viewer, each from a fresh fixture. Phone
views are the 375 px captures.

**Flows**, with action captions and step and click counts:

| Flow | Width | Task |
| --- | --- | --- |
| `playtest-draw-undo-resume` | 1440 | Keep a hand, keyboard-select a card and open its action menu, draw, undo exact zone order and reload the saved table without changing the deck |
| `first-run-sample` | 1280 | From an empty account to the sample deck |
| `import-moxfield` | 1280 | Import a Moxfield list into a new deck |
| `review-accept-with-reason` | 1440 | Review one agent proposal and accept it with a reason |
| `phone-accept` | 375 | Accept an agent proposal on a phone |
| `share-public` | 1280 | Share a deck and open its public page |
| `maybe-card-details` | 1440 | Find a card, add it to the maybe list, open its details |

Flows find controls by role and accessible name and take a shorter path when
one appears (an Import entry on All decks, a reason field for accepting), so
they keep working while the UI changes. Each flow ends by reading the result
back through the API: `completed` is true only if the outcome is really in the
database (`facts` says what was checked).

**Hard metrics** per screen: axe-core violations; horizontal overflow; text
nodes under 11 px (icon glyphs excluded); interactive targets under 44 px at
widths under 1100 px with a coarse pointer (375 and 768 are touch contexts;
inline text links are exempt, as in WCAG 2.5.5); console errors; cumulative
layout shift from load to capture. Per run: the public page's gzip JS from
the repository's own `scripts/check-public-bundle.mjs`, run beside the
temporary build.

**Determinism.** In Node (`lib/determinism.ts`): `Date` and SQLite's
`datetime('now')` read a fixed clock (the built-in is overloaded on the
connection, which DEFAULTs also use); `Math.random` and `node:crypto`'s
random functions come from a seeded PRNG, so share slugs and tokens repeat.
In the browser (`lib/browser.mjs`): `Date` frozen and `Math.random` seeded
through `addInitScript`; timers of 3 s or more (toasts) are held and released
just before the next screenshot, so a toast is in exactly the screenshot after
the action that raised it; fixed locale, time zone, colour scheme and reduced
motion; software raster on one thread; external requests refused except card
images, which are cached in `~/.cache/deck-builder-ux-eval/cache/images` after the first fetch.
Scrolling is only by mouse wheel (a CDP touch drag scrolls too, but lands a
few pixels differently each run; synthesized gesture scrolling does nothing in
headless Chromium); the pointer is parked in a corner after each action so no
hover state is captured. Never `scrollTo`.

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

### `node compare.mjs <runA> <runB>`

Pairs two runs and lists images and metrics that differ. Run it whenever the
harness changes: two runs of the same app must be identical.

### The judge kit (`judging/`)

| File | For |
| --- | --- |
| `brief.md` | The product brief judges read: users, tasks, design principles |
| `rubric.md` | Nine categories plus overall, each with 3/6/9 anchors and the owner's concerns to check |
| `judge-absolute.md` | Prompt and JSON schema: 0–10 per category, up to 10 findings with screen key, severity, issue, fix, scope |
| `judge-pairwise.md` | Prompt and JSON schema: Version 1 vs Version 2 per category, with margin and reasons |
| `aggregate.md` | Prompt for the cheap model: merge, de-duplicate and rank findings into a backlog, small fixes apart from owner decisions |
| `packet.mjs` | Builds a judge packet folder from one run or two |
| `score.mjs` | Scores judge outputs; de-blinds pairwise results and gives the keep/reject verdict |

```
node judging/packet.mjs absolute <run> [--only <regex>] [--no-flows] [--full]
node judging/packet.mjs pairwise <baseline-run> <candidate-run> [same options]
```

A packet (`~/.cache/deck-builder-ux-eval/packets/<id>/`) contains `brief.md`, `rubric.md`,
`prompt.md`, `index.md` and the images under their capture keys. Pairwise
packets put them in `v1/` and `v2/`; which run is Version 1 is random per
packet, recorded in `<id>.key.json` *beside* the folder. Give judges the
folder only. The packet builder refuses to finish if its text contains a run
id, a commit SHA or an ISO timestamp. A full packet is large (about 100
images, twice that pairwise); `--only` cuts it down to a subset of screens.

```
node judging/score.mjs --absolute results/*.json
node judging/score.mjs --key ~/.cache/deck-builder-ux-eval/packets/<id>.key.json results/*.json
```

Name each judge reply `<judge>--<sample>.json` (`opus--1.json`,
`sol--2.json`), or wrap it as `{ "judge", "sample", "output" }`. The verdict
is **keep** only when (1) a majority of all judge × sample votes prefer the
candidate overall, (2) the mean signed overall preference (+margin for, −margin
against, 0 tie) is above the noise, the mean of each judge's own standard
deviation across its samples (so every judge needs two or more samples), and
(3) no hard metric got worse: total axe nodes, small-text nodes, small tap
targets, console errors, layout shift (+0.05 tolerance, since load timing moves it slightly), incomplete captures
and completed flows; per screen, axe nodes, overflow and console errors; per
completed flow, clicks and steps; and the public bundle (under 110,000 bytes,
growth at most 500).

## The loop, as a checklist

Once per baseline:

1. `node seed.ts` (only when the seed or card data changes).
2. `node capture.mjs` on the accepted version → baseline run.
3. `node compare.mjs` against a second capture of the same version, once,
   to confirm the harness is still deterministic on this machine.

Each pass:

1. Absolute review of the baseline: `packet.mjs absolute <baseline>`; each
   judge (Claude Opus, GPT Sol) scores it, several samples each, replies saved
   as `<judge>--<n>.json`.
2. `score.mjs --absolute` for the scores; give all replies and
   `aggregate.md` to the cheap model (GPT Terra) for the ranked backlog.
3. The fixer takes one small fix from the backlog (owner decisions go to the
   owner instead) and changes the app on a branch.
4. `node capture.mjs` on the candidate → candidate run. Check its
   `metrics.json` summary.
5. `packet.mjs pairwise <baseline> <candidate>`; each judge compares,
   several samples each.
6. `score.mjs --key <id>.key.json <replies>`. **Keep** → the candidate
   becomes the baseline. **Reject** → discard the change.
7. Never show a judge the backlog, earlier replies, the key file, `run.json`
   or anything that names a pass, a date or a commit.

`node check-maybe-rows.mjs --origin http://127.0.0.1:8853 --baseline http://127.0.0.1:8857`
checks whole-row pointer/keyboard activation, focus, nested note controls, main
modifier selection, persistent reason labels, and stored assignments/membership
through promotion and undo against two disposable fixture servers.

`node check-sidebar-search.mjs` checks the expanded disposable collection: full
name visibility, deck/commander queries, unsaved draft identity, Escape/empty
clear, compact-rail destinations and keyboard navigation at all four desktop widths.

`node check-playtest-geometry.mjs --origin http://127.0.0.1:8853 --baseline http://127.0.0.1:8855`
compares adaptive hand widths/row counts, commander rail height, inspector and
hover-preview width to an established build at all four desktop widths. It
checks keyboard selection/menu access, candidate axe and unchanged deck data.

`node check-review-clarity.mjs --origin http://127.0.0.1:8854`
checks selected proposal scope, native disclosure activation without leaving
Inbox, persistent reasons and a real selected-only decision. It compares
collection colour labels/counts to server statistics, checks slot help spacing,
and exercises mixed import provenance plus custom-note controls without changing
stored assignments or order. Use a separate disposable fixture server for each
QA script that changes its setup data.

With `--wide`, capture coverage also includes wide public readers, collection
statistics, expanded pending forecasts and empty Inbox/Memories/Activity states.

`node check-hand-empty.mjs --origin http://127.0.0.1:8853`
exercises opening and kept-hand scrolling with native Enter/Space, verifies all
seven cards can be reached and the saved table frame stays unchanged, then checks
selection/menu access and empty Inbox/Activity/history recovery at four desktop
widths. Hand-card capture selectors use their object identity, since the hand can
also contain a labelled group of scrolling controls.
