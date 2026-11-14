# Game client UX direction

Josh activated this work on 2026-11-14. This document records the direction for
the first UX review; it is not acceptance of the design or complete gameplay.

## References and scope

Use the [client plan](game-client-plan.md), [responsibility boundaries](game-client-boundaries.md)
and the deck-builder's current [design system](../packages/deck-builder/web/DESIGN.md)
and [tokens](../packages/deck-builder/web/src/design/tokens.css). Do not use the
solo playtester brief, layouts or table components as design guidance. The owner
has not been satisfied with that experience.

The current client supports permitted observations, card inspection, local
decision exercises and live opening-hand choices with saved-choice recovery.
This increment improves those surfaces. It does not establish full-game play.

## Design thesis

A focused desktop game workspace: neutral dark surfaces, readable source-backed
text, aligned player information, quiet metadata and blue emphasis for selection
and deliberate actions. Match the deck builder's visual conventions while
designing interactions around the authoritative game's decisions.

The screen should answer, in order: what is happening, what needs my choice,
what information helps me choose, and what happened to my last submission.
Avoid a large empty battlefield pushing the hand or choice below the viewport.
Reference information should remain useful without competing with the decision.

## First review criteria

- At 1280 by 720, a compact player summary and current decision are easy to find.
  Larger desktops give the board and readable cards more room. Smaller windows
  and browser zoom retain access through scrolling or panels.
- Hand and decision controls stay close enough to compare without navigating
  away. Inspecting another card preserves the current decision draft and order.
- Source-backed card text, missing metadata and current characteristics remain
  distinguishable. Printed text is never interpreted as eligibility or legality.
- Selection uses exact object handles. Identically named cards remain separate.
  Enter opens inspection; Close or Escape restores focus to its originating card.
- Jump to decision focuses the actual current region. Remote updates announce
  changes without taking keyboard focus. Reduced motion remains supported.
- Keep, mulligan, bottom order and confirmation remain explicit. A pending or
  uncertain saved choice is visibly different from a consumed choice; recovery
  preserves the original command. Releasing a seat never implies undo.
- Fixture and synthetic controls remain clearly distinguished from live actions.
  Technical source/build details are available without dominating the workspace.
- Join, missing metadata, local decision exercises and live status share the
  table's typography, surface, control and focus conventions.

## Evidence limits

The current display model has no active-player identity or available card-action
list. Do not invent active-seat cues, Cast/Pass controls or automatic moves.
Card names, printed facts and rules text come only from qualified producer data.
The opening fixture does not establish dense-board or complete-game usability.
Record rendered layout review and automated interaction checks separately from
actual human play and fresh native-host integration acceptance.
