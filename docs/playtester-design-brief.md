# Commander playtester requirements and OpenDesign brief

Design a desktop solo playtester within the deck-builder, focused on opening
hands and early turns. Make rules text easy to read and common actions fast.
Develop reusable table components that can later serve the full Commander game
interface. Create a new OpenDesign project using the existing deck-builder
design system, extending that system where the table needs new components.

Status: implemented first version, 2026-10-13. The decisions below are agreed with the owner;
the functional details and design defaults form the proposed first-version
scope. Implementation and verification details are recorded in [the playtester guide](playtester.md).

## Agreed product direction

- Integrate the playtester into the deck-builder with a dedicated table screen.
- Support desktop at 2560×1440 and larger, readable at 100% display scaling and
  100% browser zoom. Mobile and tablet layouts are outside this version.
- Start with solo goldfishing and manual spell resolution. Supply convenient
  actions rather than automatic rules enforcement or an automated opponent.
- Focus on evaluating opening hands, mana development, and early turns.
- Use simple dividers between the three battlefield rows, without visible row
  headings, counts, or a main Battlefield heading (owner refinement, 2026-10-13).
- Arrange the battlefield in organized rows, with freedom to move and reorder
  cards. Battlefield rows are presentation, independent of deck slots and tags.
- Use full rules text in hand and compact battlefield cards. Prioritize text
  and game state over artwork.
- Show full text immediately on hover or keyboard focus. Clicking selects and
  pins a card in the right inspection panel; tapping is a separate action.
- Let the owner record deck observations, card-linked memories, and card tags
  during playtesting.
- Support normal multiplayer Commander mulligans and unlimited practice redraws.
- Next turn untaps the battlefield and increments the turn number. Drawing is
  always a separate action.
- Preserve the current session so it can be resumed. A history of completed
  games and a statistical dashboard are outside this version.
- Design the table pieces for eventual reuse in a full game interface.

## Primary workflows

### Test an opening hand

Open Playtest from a deck, start a fresh shuffled session or resume the existing
one, and inspect seven cards with their rules text visible. Choose a mulligan
mode, redraw as needed, select cards to bottom when required, then keep. Enter
turn one without drawing an extra card automatically.

### Play the early turns

Draw explicitly, select a card, and use a visible action or drag it to the
appropriate zone. Arrange permanents, tap for resources manually, and resolve
effects by moving cards, adjusting counters, or using library tools. Next turn
untaps and advances without drawing. Restart quickly to try another opening.

### Capture an observation

Select a card to pin its details. Move into the panel, write a memory linked to
that card in this deck, or assign tags. Use a deck observation action for a
memory about the whole deck. Saving either observation keeps the table and its
selection intact. The saved observation records the revision being tested.

### Resume a session

After refreshing or reopening the app, resume the same table with the same hand,
library order, card positions, tapped states, counters, and turn number. If the
saved deck has changed, identify the revision in the session and offer either
continuing that session or starting from the current list.

## First version functional requirements

### Session setup and deck identity

- Start from a snapshot of the selected deck's membership and commander roles.
  The snapshot is private session state, not a new general deck checkpoint
  feature.
- Display the deck name and tested revision. Changes made elsewhere to deck
  membership must not silently alter an in-progress session.
- Keep commander cards separate from the shuffled library. Support multiple
  commanders and represent a companion outside the main deck when configured.
- Exclude the maybe list from the library. Surface existing deck-check findings
  without presenting manual playtesting as proof of legality.
- Use the existing card database for names, mana costs, types, Oracle text,
  faces, and printed characteristics. Resolve all real cards used in designs
  against that data; do not invent example Oracle text.
- Start the ordinary Commander session at 40 life. See rules 903.6–903.7 in the
  [Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).

### Opening hands and mulligans

- Make all seven prospective opening cards readable without opening individual
  dialogs. Long text may scroll naturally; text must not shrink to fit.
- In normal multiplayer Commander mode, shuffle the rejected hand into the
  library and draw seven; the first mulligan is free. Later mulligans require
  bottoming the appropriate number of cards in the owner's chosen order.
  Enforce the ordinary stopping limit. Follow rules 103.5 and 103.5c in the
  [Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
- Show the mulligan count and number of cards to bottom. Mark Keep unavailable
  while required choices are incomplete, with a readable reason.
- In practice mode, allow unlimited fresh seven-card hands without a bottoming
  penalty. Identify practice mode visibly so redraws cannot be confused with
  normal mulligans.
- Select the mode before beginning the hand sequence. Changing modes starts a
  fresh opening sequence rather than retaining an ambiguous count.
- Make a new opening test easy to start while protecting an existing session
  from an accidental destructive restart.

### Card readability and inspection

- Hand cards show name, mana cost, type line, complete Oracle text, and relevant
  printed characteristics as text and mana symbols. Artwork is optional and
  never needed to understand or operate a card.
- Battlefield cards show name, cost, relevant current characteristics, tapped
  state, counters, and attachments. Preserve legibility when tapped; use a
  state marker rather than rotating readable text sideways.
- Hover or keyboard focus previews full card text in the right panel with no
  click. A click pins the selected object and exposes its actions and editing
  tools.
- While a card is pinned, hovering another card must not replace the selection
  or the memory/tag target. Other cards remain inspectable using a temporary
  preview; specify its placement in the design.
- Moving into the right panel must preserve its card. Clearing selection
  explicitly returns the panel to deck context.
- Distinguish printed card facts from manually adjusted game state. Do not
  claim that an editable power or toughness value includes evaluated continuous
  effects.
- Provide a visible face selector for cards with multiple faces. Long text,
  long names, and multiple abilities must remain reachable.
- Never require a context menu and another dialog just to read rules text.

### Battlefield and zones

- Provide default rows for creatures, other permanents, and lands. Allow manual
  placement, reordering, and moving between rows without changing deck tags.
- Keep hand, library, command zone, graveyard, and exile clearly labeled and
  show their counts. Reserve a reusable zone presentation for future stack
  objects; automatic stack handling is outside this version.
- Make commanders discoverable and provide the same selection and move
  affordances used for other cards.
- Expand graveyard and exile into readable, selectable lists. Their viewers
  should allow inspection and movement without hiding the whole battlefield.
- Allow moves between supported zones, including library top and bottom.
  Preserve order where it matters; visual sorting must not change library order.
- Support multi-selection and batch tap, untap, and zone movement. Make the
  target of every action clear when several cards are selected.
- Give each card or token instance its own selection and state. Two copies of
  one card must be independently selectable and able to hold different counters.

### Manual actions and state

- Provide separate visible controls and shortcuts for Draw, Draw several,
  Shuffle, Untap all, Next turn, Undo, and Redo.
- Next turn increments the turn and untaps the battlefield. It does not draw,
  resolve upkeep effects, pay costs, or enforce a phase sequence.
- Selecting a card changes selection only. Tapping uses an explicit card
  control, panel action, or shortcut.
- Provide search of the current library and an ordered top-card viewer. Allow
  manual selection, reordering, and distribution among supported zones for
  effects such as scry, surveil, and mill.
- Allow token creation, token copies, counters with visible increment/decrement
  controls, manual power/toughness adjustment, attachment/detachment, and manual
  life adjustment. Support a manual commander cast-count field if needed.
- Undo game actions, including draws, moves, shuffles, counter changes, and
  Next turn. Undoing a shuffle restores its previous order.
- Keep game undo separate from saved memories and tags. Rewinding a turn must
  not remove a deck observation or undo a tag edit.
- Do not infer mana availability, resolve triggers, judge legal targets, or
  report an automated win condition. Manual controls should remain available
  for unusual interactions and corrections.

### Memories and tags

- Reuse the existing memory model rather than introducing separate persistent
  card notes. Deck observations are deck-scoped; card observations additionally
  link the selected card. Default both to the deck being tested.
- Attribute owner-written memories to the owner and retain the tested revision
  even when the current deck list has subsequently changed. Existing memories
  should be available for context in the panel.
- Save observations to the deck-builder immediately on explicit Save. Preserve
  an unfinished editor draft while the user continues inspecting the table.
- Assign and remove existing per-deck tags using the existing vocabulary and
  naming rules. Permit creating a valid tag from the same workflow.
- Tag edits apply to the saved deck and are not temporary token/card-instance
  annotations. Disable assignment for an object that has no current deck-card
  entry, explaining why; a card-linked memory can still capture the observation.
- Game restart clears the session's game state while saved memories and tags
  remain. Show save progress, success, and failures without clearing unsaved
  text or covering the table.
- Follow the existing [memory and tag requirements](deck-builder-spec.md),
  especially sections 5.4 and 7.

### Resume and interaction quality

- Save enough session state to restore the exact library order, zones, instances,
  positions, counters, attachments, faces, life, turn, and opening-hand stage.
  Restore selection and an unfinished observation draft where practical.
- Distinguish Saving, Saved, and Save failed. Recovery must not masquerade as a
  successful save or replace a failed resume with an unexplained fresh shuffle.
- Keep routine interactions free of blocking dialogs. Use a confirmation for
  discarding the current session during restart; preserve saved observations.
- Provide keyboard focus, readable action labels, and visible shortcut hints.
  Shortcuts do not fire while typing a memory, tag name, or search query.
- Distinguish selected, hovered, tapped, and disabled states with text or shape
  as well as color. Avoid hand fanning, overlap that hides names, and automatic
  movement that makes the pointer chase a card.
- Scroll zones and long content without losing the action bar or inspection
  panel. Larger boards and hands use more space or scrolling, not progressively
  smaller fonts. Respect reduced-motion preferences.

## Reuse in the future Commander interface

The shared presentation should accept the state it is allowed to display and
the actions available in that context. It reports user intent; the session or
future rules engine supplies resulting state. A component must not depend on
deck-builder routing, memories, tags, or a particular game transport.

Design reusable pieces for:

- Text card displays in full, compact, selected, tapped, face-down, and targeted
  states, with separate printed facts and game-state indicators.
- Zone lists and ordered viewers, including hidden-count representations.
- Battlefield rows, player areas, selection, counters, and attachments.
- Card inspection with optional contextual tools supplied by its host screen.
- Player summaries, turn indicators, action bars, and decision prompts.

The future design study should exercise four players, a shared stack, active
player and priority indicators, target selection, disabled actions with reasons,
distinct owner/controller labels, hidden hands, and revealed cards. Never send
hidden card identities to a display and rely on CSS to conceal them. Render the
permitted observation supplied by the game system.

These states are component and layout studies, not first-version multiplayer
features. The engine's authoritative object identities and legal-action
protocol remain separate from manual session data. The established boundary is
a TypeScript browser interface consuming the future Rust engine/server; see
[architecture](architecture.md) and the
[language decision](runtime-language-decision.md). Start within the existing
frontend layers and extract a shared package when another client exists.

## Existing design system

Use the deck-builder's existing OpenDesign library when available, cross-checking
it against [web/DESIGN.md](../packages/deck-builder/web/DESIGN.md),
[web/ARCHITECTURE.md](../packages/deck-builder/web/ARCHITECTURE.md),
[tokens.css](../packages/deck-builder/web/src/design/tokens.css), and the live
style guide at `#/design`.

Preserve the dark surfaces, typography families, semantic colors, sentence-case
labels, focus treatment, buttons, inputs, tags, menus, and resizable right
panel. Color meanings remain consistent: blue selection, violet agent
attribution, green additions, red removals, and amber warnings. Use grayscale
for ordinary structure.

Add reusable components for text cards, zone headers/viewers, battlefield rows,
tap state, counter editing, player summaries, and the inspection panel's table
context. Add readable rules-text typography and necessary dimensions as design
tokens. Larger rules text is a deliberate extension of the dense deck-list
system; do not scale up or redesign the entire application.

Show new components and states in a design-system page, with existing and new
assets identifiable. Preserve the existing deck-builder project while creating
the separate playtester project.

## OpenDesign screens and review criteria

Create the main designs at 2560×1440. Verify that browser chrome can reduce the
usable height without hiding primary actions. Also review at 3840×2160 at 100%
scaling, using additional space rather than stretching text across the screen.
Ultrawide is an exploratory check, not a separate agreed minimum requirement.

Design these states using real database-backed card examples:

1. New session with a seven-card opening hand and mode selection.
2. A free first mulligan and a later mulligan requiring bottom-card selection.
3. Practice redraw mode and a clear Keep action.
4. A typical turn-three table with organized rows, hand text, zone counts, and
   separate Draw and Next turn controls.
5. A selected battlefield card pinned in the right panel, with tap controls,
   counters, card memory, and tag editing.
6. A deck observation editor, including unsaved text and a failed save.
7. Expanded graveyard/exile and library search/top-card ordering.
8. A large hand of at least 20 cards and a battlefield with at least 30 objects,
   including repeated tokens and attachments.
9. Long Oracle text, long card names, multiple faces, and two commanders.
10. Resume after the saved deck's membership changes, with explicit tested
    revision and choices to continue or start fresh.
11. Component variants and keyboard focus states in the design-system page.
12. A future four-player Commander layout using the same core components.

A design passes review when:

- A seven-card opening hand can be read at normal zoom without menu navigation.
- Any battlefield card's full rules text can be inspected with zero clicks.
- Selecting, hovering, and tapping are visibly distinct; click-to-select never
  changes tapped state.
- Moving into the panel preserves the intended memory/tag target and unsaved
  editor text.
- Next turn leaves the hand and library unchanged while advancing and untapping.
- Normal mulligans expose required bottoming choices; practice redraws do not
  accumulate a normal mulligan penalty.
- Large hands and busy battlefields remain navigable without shrinking text.
- Expanded zones and editors preserve access to the table and main controls.
- A session can resume its exact state, and saved observations outlive restart.
- The future study can reuse cards, zones, and inspection without requiring
  deck notes or exposing hidden card identities.

## Research informing the design

Player discussions give qualitative examples of preferences and friction; they
are not a representative ranking of products. These lessons inform the design,
while the owner's stated readability problem determines its priorities.

- Players disagree about Moxfield versus Archidekt. A recent discussion calls
  out awkward animated hands at 20+ cards and praises simpler shortcuts. Favor
  stable text cards and visible actions.
  [Player discussion](https://www.reddit.com/r/EDH/comments/1w2z0tv/what_are_people_using_to_goldfishtest_their_decks/)
- Archidekt's Playtester 2.0 documentation covers card sizing, multi-selection,
  counters, library tools, and mulligans. Feedback on that older redesign asks
  for persistent previews, more visible controls, and easier command-zone use.
  These are interaction lessons, not claims about missing current features.
  [Archidekt update and feedback](https://archidekt.com/news/3417345)
- A Moxfield counter-editing request complains about repeated menu use; a reply
  explains an existing right-click decrement action. Make common actions
  discoverable as well as available.
  [Moxfield counter feedback](https://moxfield.nolt.io/1516)
- Playgroup documents draw, mulligan, zone movement, undo, and quick restart for
  solo tests. These form a useful baseline for repeated early-turn practice.
  [Playgroup playtester](https://playgroup.gg/playtest)
- EDH Lib documents organized battlefield types, zone viewers, shortcuts, and
  a combined untap/draw/next-turn action. Reuse the organizational ideas while
  keeping drawing separate as the owner requested.
  [EDH Lib playtest documentation](https://www.edhlib.com/docs/playtest)
- Archidekt's interaction simulator provides configurable hypothetical
  opposition with manual resolution. Feedback praises the feature but also
  describes implausible plays. Simulated opposition remains a later option.
  [Interaction simulator and feedback](https://archidekt.com/news/26104256)

## OpenDesign project prompt

Create a new Commander playtester project using the existing deck-builder
design system. Design for 2560×1440 and larger desktop screens at 100% scaling
and browser zoom. The first version is a manual solo goldfish table for testing
opening hands and early turns. Prioritize readable rules text and game state
over artwork. Show full text in hand, compact battlefield cards in rearrangeable
rows, immediate hover/focus inspection, and click-to-select with a pinned right
panel. Tapping is a separate visible action. Put card memories, deck observations,
and deck-tag editing in contextual panel tools. Next turn untaps and increments
the turn; Draw remains separate. Support normal multiplayer Commander mulligans,
unlimited practice redraws, undo, zone movement, library tools, and exact current
session resume. Extend the shared design system with reusable table components
and demonstrate their future use in a four-player Commander layout. Follow the
screen checklist and review criteria above; keep rules enforcement and multiplayer
behavior outside the first-version design flow.
