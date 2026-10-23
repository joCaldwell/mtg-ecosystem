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
