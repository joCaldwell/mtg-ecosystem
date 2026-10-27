import { test } from "node:test";
import assert from "node:assert/strict";
import { colourDistribution, manaDistributions, shareLabel, STAT_COLOURS } from "../../web/src/domain/deckStats.ts";
import { analyzeMana } from "../../src/deck/mana.ts";
import { MANA_FIXTURES } from "../fixtures/mana-analysis.ts";

test("each colour distribution has its own denominator and exact clockwise shares", () => {
  const costs = colourDistribution({ W: 3, U: 2, B: 1, R: 0, G: 4 });
  assert.equal(costs.total, 10);
  assert.deepEqual(costs.rows.map(row => [row.id, row.count, row.percentage, row.offset]), [
    ["W", 3, "30%", 0], ["U", 2, "20%", 0.3], ["B", 1, "10%", 0.5], ["R", 0, "0%", 0.6], ["G", 4, "40%", 0.6],
  ]);
  assert.equal(costs.rows.reduce((sum, row) => sum + row.share, 0), 1);
});

test("mono-colour is a true full ring and empty distributions never manufacture shares", () => {
  for (const colour of STAT_COLOURS) {
    const distribution = colourDistribution({ W: 0, U: 0, B: 0, R: 0, G: 0, [colour.id]: 76 });
    assert.equal(distribution.rows.find(row => row.id === colour.id)?.share, 1);
    assert.equal(distribution.rows.find(row => row.id === colour.id)?.percentage, "100%");
    assert.equal(distribution.rows.filter(row => row.share > 0).length, 1);
  }
  const empty = colourDistribution({ W: 0, U: 0, B: 0, R: 0, G: 0 });
  assert.equal(empty.total, 0);
  assert(empty.rows.every(row => row.share === 0 && row.offset === 0 && row.percentage === "—"));
  assert.equal(shareLabel(1, 100000), "<0.1%");
  assert.equal(shareLabel(99999, 100000), ">99.9%");
});

test("source-colour shares count overlaps without inventing cards, any-colour splits or other faces", () => {
  const names = ["Glacial Fortress", "Savai Triome", "Birds of Paradise", "Sol Ring", "Toluz, Clever Conductor", "Witch Enchanter"];
  const rows = names.map(name => {
    const fixture = MANA_FIXTURES.find(card => card.name === name || card.name.split(" // ")[0] === name);
    assert(fixture, `DB-verified fixture ${name}`);
    return { ...fixture, quantity: 1, role: "card", maybe: 0 };
  });
  const mana = analyzeMana(rows), { costs, sources } = manaDistributions(mana);
  assert.equal(sources.total, 5, "two land copies yield five named-colour counts, not five unique sources");
  assert.deepEqual(sources.rows.map(row => row.count), [2, 1, 1, 1, 0]);
  assert.deepEqual(sources.rows.map(row => row.percentage), ["40%", "20%", "20%", "20%", "0%"]);
  assert.equal(mana.first_face.sources.nonland.any_color_copies, 1);
  assert.equal(mana.first_face.sources.nonland.by_type.C, 1);
  assert.equal(mana.other_faces.sources.land.by_type.W, 1);
  assert.equal(costs.total, 3, "fixed symbols only; flexible costs and other faces are not split into the pie");
});
