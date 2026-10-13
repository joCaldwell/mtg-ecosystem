# Desktop Commander playtester

Open a deck and choose **Playtest**, or navigate to `#/deck/<id>/playtest`.
The first version implements the [approved requirements](playtester-design-brief.md)
inside the standalone deck-builder. Target: 2560×1440 and larger monitors, at
100% scaling and zoom. The checked usable viewport reserves room for browser
chrome. Mobile layout, rules automation, multiplayer and completed-game history
remain outside this version.

## Using the table

Start a Commander or practice opening sequence. Commander redraws draw seven,
with the first mulligan free; later hands require ordered bottom selections.
Practice redraws have no penalty. Keep advances to turn one without drawing.
Changing mode starts a fresh opening sequence. A new opening test asks before
replacing the current game and retains an unfinished observation with its
original tested revision.

Click or Enter/Space on a card selects and pins it. Shift/Ctrl/Cmd-click toggles
individual instances into a selection. Hover/focus displays rules immediately.
A separate scrollable preview inspects another object while the selected card
stays pinned. Tap is an explicit action, never a side effect of selection.

The battlefield has three rows with simple dividers and no visible headings or
row counts. Drag cards between zones and rows, or drop onto a card to insert
before it. The inspector provides row selection and Move earlier/later as
keyboard-accessible alternatives. The selection bar moves several objects and
sets their tapped state explicitly.

Library tools reveal ordered top cards or search the whole current library by
name. Search never sorts or changes its order. Select cards to move to hand,
battlefield, graveyard, exile, command, companion, stack or library top/bottom.
Arrow controls reorder individual library cards. Graveyard/exile and command
viewers support the same immediate inspection and manual movement.

Tokens are manually created copies of verified database cards or existing table
objects, each with independent identity/state. Counters, printed-face selection,
manual power/toughness, attachments, life and commander cast counts are editable.
These controls do not resolve spells, determine legal targets, evaluate continuous
effects or infer available mana. Moving a host out of the battlefield detaches
its attachments; game undo restores those relationships.

| Shortcut | Action |
| --- | --- |
| D / Shift-D | Draw one / the visible draw quantity |
| S | Shuffle the library |
| U | Untap the battlefield |
| N | Untap and advance the turn; draw separately |
| T | Toggle selected battlefield objects' tapped state |
| Z / Shift-Z | Game undo / redo |
| Ctrl/Cmd-Z / Ctrl/Cmd-Shift-Z | Game undo / redo |
| Escape | Close a viewer or clear selection |

Shortcuts stand down in editable fields and modal dialogs. Routine controls
remain visible. Game undo retains up to 100 action frames, including exact
pre-shuffle library order; it never reverses saved memories or tags.

## Persistence and deck context

Schema 7 adds `playtest_sessions` and an observation retry ledger. A deck has
one current table, deleted by the deck's existing cascade when that deck is
removed. The table snapshots membership, commander/companion roles, quantities,
card facts and faces, and the tested revision. Maybe-list entries are excluded.
Changing the deck elsewhere leaves the table intact and surfaces a revision
mismatch. Current deck checks remain available as context, not as proof of game
legality. Starting requires at least seven ordinary deck cards.

The service in `packages/deck-builder/src/playtest` is owner-only. It validates
intent, applies each action transactionally, and returns the authoritative
session plus computed counts/mulligan state. Mutations check session identity
and version to reject stale-window writes. JSON snapshots preserve objects,
zones, order, rows, tapped states, counters, faces, manual values, attachments,
life, turn, opening stage, game undo/redo and UI selection/draft. A malformed
saved table produces an error and remains on disk until explicitly replaced.

The browser serializes writes against the latest returned version. Saving,
Saved and Save failed are distinct states. A small browser scratch copy recovers
an unfinished observation when it is at least as recent as the saved session;
the database remains the session store. The scratch copy never replaces game
state. Panel width is a browser preference. Failed writes retain editor text;
reload recovers the authoritative table while keeping an unfinished draft.

Explicit observation saves use the existing owner memory capability, link the
chosen card when applicable, and carry the tested revision even after restart
or deck edits. A request ID makes retrying a lost observation response
idempotent. Hover and ordinary card selection do not retarget an unfinished
observation; the inspector exposes an explicit retarget action.

The inspector reuses the existing searchable checkbox tag picker and vocabulary
validation. Tags apply immediately to the current saved deck-card entry. Tokens
and removed deck cards show why tag assignment is unavailable. Saved memories
and tags survive game undo and restart.

## Reusable presentation

`web/src/mtg/table` contains `TextCard`, `TableZone`, `PlayerSummary` and
`CardInspection`. They accept observations and callbacks, with contextual tools
supplied by their host. They depend on no routing, deck notes, API transport or
rules engine. `web/src/domain/table.ts` holds pure display/selection helpers;
wire types enter through `api/client.ts`. `CounterControl` is a generic UI
primitive. The original visual system is extended with 15px/1.6 rules typography
and card/table dimensions, rather than changing the deck-list typography.

The `#/design` page demonstrates full, compact, selected, tapped, targeted,
disabled and anonymous cards, zones and inspection. Its four-player study is
presentation only. A hidden observation contains no card identity; a future
Rust game service must supply permitted observations and authoritative actions.

## Verification

Run from `packages/deck-builder`:

```sh
npm run check
npx vite build
```

The package gate passes 381 tests, including new service, HTTP, domain and React
component integration checks. They cover mulligan limits and ordering, snapshot
isolation, multiple commanders/companion/maybe cards, faces, exact shuffle
undo/redo, independent token state, attachments/cycles, batch library order,
revision-bound memories and retained drafts, idempotent retries, owner permissions,
stale versions, invalid actions, migration from schema 6, disk resume, malformed
payload preservation and anonymous hidden-card markup. Component checks load
real JSX through the existing Vite pipeline without new runtime dependencies.

Live browser checks used an isolated SQLite backup and seeded demo data, never
the owner's working database. Native pointer/keyboard checks covered selection
versus tapping, pinned hover inspection, typing without triggering draw, Commander
mulligans/bottoming, Next turn without draw, library ordering, drag/drop,
observation failure/one-time retry, editing a draft after a lost committed
response, tag creation/assignment, first-session refresh and exact resume.
At 2560×1320, seven opening cards fit across with 15px rules. A 20-card hand and
32-object battlefield retained independent scrolling and zero page overflow.
At 3840×2040 the hand still scrolls, the board fits, and text remains 15px.
The collaborative browser's captured screenshot stayed cropped to its physical
1280×800 surface; desktop layout verification used measured DOM geometry.
