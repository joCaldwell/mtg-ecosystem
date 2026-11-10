import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/parser/index.ts";

// Cleave, CR 702.148a-b: paying the cleave cost removes all text in square
// brackets from the spell's rules text, so a bracketed paragraph has two
// readings. Oracle text of Fierce Retribution, Path of Peril, Dig Up,
// Inspired Idea (shape) and Graveyard Dig (no cleave keyword).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const REMINDER = "(You may cast this spell for its cleave cost. If you do, remove the words in square brackets.)";
const parse = (text: string, typeLine = "Instant") =>
  JSON.parse(JSON.stringify(parseOracleText(text, "Test Spell", { typeLine }))) as
    { ok: boolean; lines: { text: string; ok: boolean; ability?: unknown }[] };
const n = (value: number) => ({ amount: "fixed", value });
const cleave = (...symbols: string[]) => ({ kind: "keywords", keywords: [{ keyword: "cleave", cost: [{ cost: "mana", symbols }] }] });
const destroy = (filter: object) => ({ steps: [{ effect: "destroy", what: { ref: "target", filter, count: n(1) } }] });

describe("cleave paragraphs (CR 702.148)", () => {
  it("keeps the printed reading and the reading without the bracketed words", () => {
    const result = parse(`Cleave {5}{W} ${REMINDER}\nDestroy target [attacking] creature.`);
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines.map(line => line.ability), [
      cleave("5", "W"),
      { kind: "spell", effects: [destroy({ types: ["creature"], status: ["attacking"] })], cleaved: [destroy({ types: ["creature"] })] },
    ]);
    assert.equal(result.lines[1].text, "Destroy target [attacking] creature.");
  });

  it("removes every bracketed span, including a whole instruction", () => {
    const result = parse(`Cleave {1}{B}{B}{G} ${REMINDER}\nSearch your library for a [basic land] card, [reveal it,] put it into your hand, then shuffle.`, "Sorcery");
    assert.equal(result.ok, true);
    const search = (filter: object) => ({ effect: "search", who: { player: "you" },
      zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter, count: n(1) } });
    const toHand = { effect: "move-zone", what: { ref: "it" }, to: { zone: "hand", owner: "your" } };
    const shuffle = { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } };
    assert.deepEqual(result.lines[1].ability, {
      kind: "spell",
      effects: [{ steps: [search({ supertypes: ["basic"], types: ["land"], cls: "card" }),
        { effect: "reveal", who: { player: "you" }, what: { ref: "it" } }, toHand, shuffle] }],
      cleaved: [{ steps: [search({ cls: "card" }), toHand, shuffle] }],
    });
  });

  it("drops a bracketed trailing sentence only from the cleaved reading", () => {
    const result = parse(`Cleave {3}{U}{U} ${REMINDER}\nDraw three cards. [You lose 3 life.]`, "Sorcery");
    assert.equal(result.ok, true);
    const draw = { steps: [{ effect: "draw", who: { player: "you" }, amount: n(3) }] };
    assert.deepEqual(result.lines[1].ability, {
      kind: "spell",
      effects: [draw, { steps: [{ effect: "lose-life", who: { player: "you" }, amount: n(3) }] }],
      cleaved: [draw],
    });
  });

  it("leaves an unbracketed paragraph of a cleave spell unchanged", () => {
    const result = parse(`Cleave {3}{U} ${REMINDER}\nDestroy target [attacking] creature.\nDraw a card.`);
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines[2].ability, { kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }] });
  });

  it("fails when either reading is unsupported or the brackets are malformed", () => {
    // Without the cleave keyword, brackets have no rules meaning (Graveyard Dig).
    assert.equal(parse("Destroy target [attacking] creature.").ok, false);
    assert.equal(parse(`Cleave {5}{W} ${REMINDER}\nDestroy target [frobnicating] creature.`).ok, false);
    assert.equal(parse(`Cleave {5}{W} ${REMINDER}\nDestroy target [attacking creature.`).ok, false);
    assert.equal(parse(`Cleave {5}{W} ${REMINDER}\nDestroy target [] creature.`).ok, false);
    assert.equal(parse(`Cleave {5}{W} ${REMINDER}\nDestroy target [attacking [tapped]] creature.`).ok, false);
    // Removing the words must leave a complete instruction.
    assert.equal(parse(`Cleave {5}{W} ${REMINDER}\nDestroy [target attacking creature].`).ok, false);
    assert.equal(parse(`Cleave {5}{W} ${REMINDER}\n[Destroy target attacking creature.]`).ok, false);
  });
});
