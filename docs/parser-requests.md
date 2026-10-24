# Parser requests from engine slices

Engine slices consume the frozen oracle AST archive (parser revision
`41863dc8a39248e37048eae7326e02b34bdb0877`, manifest pinned in
`packages/game-engine/tests/fixtures/archive-manifest.json`). When a card a
slice needs is unparsed or has an AST shape the engine can't lower faithfully,
record it here. Do not edit the parser from engine work. Each entry gives the
card, Oracle ID, AST path, the shape needed, the rules citation, and whether a
slice is blocked.

## Open

### Split basic-land search destination (Cultivate, Kodama's Reach)

- **Cards:** Cultivate `8b755881-a72d-4e21-a369-d2924eb4585a`; Kodama's Reach
  `1593ea18-2f2f-4ab4-83fb-6ccc0bec8a90`. Both are high-use Commander ramp
  (EDHREC ranks 20 and 37).
- **Text:** "Search your library for up to two basic land cards, reveal those
  cards, put one onto the battlefield tapped and the other into your hand, then
  shuffle."
- **Archive result:** `faces/0/result/lines/0` fails with
  `expected counter type at "tapped" (token 20)`, so there is no AST.
- **Needed shape:** a `search` step for up to two basic land cards, with
  `reveal`, followed by a split `move-zone` that sends exactly one found card to
  the battlefield tapped and the remainder to hand, then `shuffle`. The split
  must say who chooses which card goes where: the searching player, as part of
  resolution.
- **Rules:** CR 701.23a, 701.23b (a player may fail to find cards with a
  stated quality), 701.23e (reveal), 701.24 (shuffle).
- **Status:** not blocking. The next ramp slice uses Rampant Growth and
  Explosive Vegetation, which parse. Check whether the current main parser
  already handles this text before treating it as new parser work.

### Each-other-player sacrifice and conditional discard (P6.9 exclusions)

- **Cards:** Grave Pact `6f4ac4a4-53ec-4bc9-8f5c-d4b801d867b2`;
  Liliana's Triumph `c2e68ac5-cfff-4b76-8062-25a82fdf9a5c`.
- **Frozen archive:** 1-based lines **16697** and **29312** of
  `generated/oracle-ast.jsonl`; both have status `unsupported` and no abilities.
- **Paths/errors:** `faces/0/result/lines/0`. Grave Pact fails at `"player"`
  (token 9) in “each other player sacrifices a creature of their choice”.
  Liliana's Triumph fails at `"also"` (token 18) in its conditional discard.
- **Needed shapes:** a dies trigger filtered by `control: you`, followed by
  an `each-other-player` sacrifice set whose members choose their own creatures;
  and an `each-opponent` sacrifice followed by a separate resolution-time
  conditional discard if the controller controls a Liliana planeswalker.
  The condition must retain the planeswalker subtype and its controller.
- **Rules:** pinned CR 101.4, 102.2–.3, 603.6c/603.10a, 608.2c/h,
  700.4 and 701.21a. Player-set semantics must remain distinct from targeting.
- **Status:** not blocking. The frozen P6.9 contract explicitly excludes these
  cards and uses parsed Butcher of Malakir for APNAP forced sacrifice. No parser
  changes were made. Recheck the current parser before starting new parser work.
