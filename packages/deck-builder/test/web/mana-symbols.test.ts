import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { symbolSvgUrl } from "../../web/src/mtg/manaSymbols.ts";

// Real entries from Scryfall's GET /symbology, reduced to symbol and svg_uri.
const fixture: { data: { symbol: string; svg_uri: string }[] } = JSON.parse(
  readFileSync(new URL("../fixtures/scryfall-symbology.json", import.meta.url), "utf8"),
);
const real = new Map(fixture.data.map((e) => [e.symbol.slice(1, -1), e.svg_uri]));

test("every mapped symbol names exactly the svg_uri Scryfall publishes for it", () => {
  assert.ok(real.size > 70);
  for (const [symbol, uri] of real) assert.equal(symbolSvgUrl(symbol), uri, symbol);
  assert.equal(symbolSvgUrl("w/u"), real.get("W/U"), "case follows parseSymbol");
});

test("nothing outside Scryfall's list is mapped, so no symbol requests a missing file", () => {
  const tokens = ["W", "U", "B", "R", "G", "C", "P", "2", "S", "X", "H"];
  const candidates = new Set<string>(["21", "99", "101", "1000", "01", "", "PW", "CHAOS", "TK", "A", "L", "D", "HW", "½/W", "W/U/P/P"]);
  for (let n = 0; n <= 120; n++) candidates.add(String(n));
  for (const a of tokens) {
    candidates.add(a);
    for (const b of tokens) {
      candidates.add(`${a}/${b}`);
      for (const c of tokens) candidates.add(`${a}/${b}/${c}`);
    }
  }
  for (const symbol of candidates) {
    assert.equal(symbolSvgUrl(symbol) !== null, real.has(symbol), symbol);
  }
  assert.equal(symbolSvgUrl("U/W"), null, "hybrids exist only in Scryfall's order");
  assert.equal(symbolSvgUrl("U/W/P"), null);
  assert.equal(symbolSvgUrl("H"), null, "Scryfall's {H} is a bare black glyph that vanishes on the canvas");
});
