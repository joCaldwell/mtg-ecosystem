# Frontend architecture

The UI is a React single-page app with no state library, no router library
and no CSS framework. What keeps it changeable is a small number of layers
with one-way dependencies, and one rule about where each kind of code goes.

## Layers

```
web/src/
  design/    tokens.css, base.css          the only place raw colours and sizes exist
  ui/        Button, Chip, Tabs, Dialog…   generic primitives; know nothing about Magic or this app
  mtg/       ManaCost, CardRef, RichText…  how a Magic card is shown; no API, no app state
  domain/    grouping, review, marks…      pure functions over wire types; no React, no DOM
  api/       client.ts, live.tsx, useQuery the typed HTTP client and the change stream
  app/       App, routes, shell, providers wiring: routing, sidebar, app-wide providers
  features/  deck/, inbox/, memories/…     one folder per screen or shared workflow
```

Imports go **down this list only**:

| Layer | May import from |
| --- | --- |
| `ui` | `design` (CSS variables only) |
| `mtg` | `ui`, `domain` |
| `domain` | wire types from `api/client.ts`, **type-only** |
| `api` | server types, type-only |
| `features/x` | `ui`, `mtg`, `domain`, `api`, `app` hooks, and another feature **only through its `index.ts`** |
| `app` | everything |

`test/web-architecture.test.ts` enforces the table. If a change needs an
import that the table forbids, the code is in the wrong layer.

## Where does it go?

- **It is a number or a colour.** `design/tokens.css`. Never a literal in a component.
- **It could be used by an app that has nothing to do with Magic.** `ui/`.
- **It draws part of a Magic card.** `mtg/`.
- **It turns server data into something to display, and has no JSX.** `domain/`, with a test in `test/web/`.
- **It talks to the server.** `api/client.ts`. Components never call `fetch`.
- **It is one screen, or a workflow shared by two screens.** `features/<name>/`.

## Data flow

The server is the source of truth and the client never merges.

1. **Reads.** `useQuery(() => api.something(), [deps])` fetches, and refetches
   whenever the server's event stream says anything changed (`useLive`). That
   is what makes an agent's proposal appear without a reload.
2. **The open deck.** `features/deck/DeckStore.tsx` holds the `DeckState`
   composite. Every mutation endpoint returns a fresh composite under
   `state`; `run(() => api.x(...))` applies it, so no component ever patches
   state by hand.
3. **Screen state** that the server does not know about (which tab is open,
   which card is selected, whether a review is in progress) lives in a
   reducer next to the screen: `features/deck/DeckUi.tsx`.
4. **Computed numbers come from the server.** Counts, slot deltas, the curve,
   pips and the projected count after pending proposals are all in
   `state.computed`. The client arranges them; it does not recount them.

## Conventions

- **One component per file**, named for the component. Its styles sit beside
  it as `Name.module.css`. No global class names outside `design/`.
- **CSS modules use tokens only.** No hex colours, no pixel font sizes. If a
  value is missing, add a token.
- **`style={{…}}` is for values computed at runtime** (a meter's width, a
  popover's position). Everything else is a class.
- **Props over context.** Context is for things many distant components need:
  the deck store, the screen state, toasts, the card lookup.
- **A feature exports through `index.ts`.** Other features import from there
  and nowhere else inside it.
- **Keyboard shortcuts** are declared with `useHotkeys` at the screen that
  owns them, never on `document` directly.
- **Card facts are never typed in.** Names, costs and text come from the API.
- Comments say why. The spec section (`§8.2`) is the usual why.

## Adding things

**A new page** (say, a playtester):

1. `features/playtest/PlaytestPage.tsx`, built from `ui/` and `mtg/` pieces.
2. Add the route to `app/routes.ts` and the page to the switch in `app/App.tsx`.
3. Add a sidebar entry in `app/Sidebar.tsx` if it needs one.
4. Any data shaping goes in `domain/` with a test.

**A new primitive:** add it to `ui/`, export it from `ui/index.ts`, and add
an example to `features/styleguide/StyleguidePage.tsx` (`#/design`). If it
needs a new colour or size, that is a token first.

**A new server field:** change the server type; `api/client.ts` imports it
type-only, so the typecheck (`npm run build`) points at every place the UI
has to follow.

## Checking a change

`npm run build` typechecks the server and the UI. `npm test` runs the domain
tests and the layering test. `npm run dev` serves the UI with hot reload;
seed the demo deck first (`npm run seed:demo`) so every panel has content.
`#/design` shows every primitive in every state.
