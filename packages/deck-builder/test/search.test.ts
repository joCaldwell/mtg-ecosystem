import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { search, resolveExactName, suggestNames, SearchError } from "../src/search/index.ts";
import { parse } from "../src/search/parse.ts";


const db = openDb(":memory:");
ingestCards(db, FIXTURES);

function names(query: string, opts = {}): string[] {
  return search(db, query, opts).map((r) => r.name);
}

describe("ingest", () => {
  test("skips token layouts", () => {
    assert.equal(names("soldier").length, 0);
  });
  test("MDFC gets combined text, front-face P/T, joined mana cost", () => {
    const [witch] = search(db, '!"Witch Enchanter"');
    assert.ok(witch, "MDFC resolves by face name");
    assert.match(witch.oracle_text, /destroy up to one target artifact/);
    assert.match(witch.oracle_text, /enters tapped/);
    assert.equal(witch.power, "3");
    assert.equal(witch.mana_cost, "{3}{W} // ");
  });
});

describe("name terms", () => {
  test("bare word is a substring match", () => {
    assert.deepEqual(names("seedborn"), ["Seedborn Muse"]);
  });
  test("quoted phrase matches names with spaces", () => {
    assert.deepEqual(names('"sol ring"'), ["Sol Ring"]);
  });
  test("! is exact, case-insensitive, and matches face names", () => {
    assert.deepEqual(names("!counterspell"), ["Counterspell"]);
    assert.deepEqual(names('!"witch-blessed meadow"'), ["Witch Enchanter // Witch-Blessed Meadow"]);
    assert.equal(names("!counter").length, 0);
  });
});

describe("type and text", () => {
  test("t: is a partial match", () => {
    assert.ok(names("t:legend").includes("Atraxa, Praetors' Voice"));
    assert.ok(names("t:creature").includes("Tarmogoyf"));
  });
  test("o: searches oracle text including MDFC back faces", () => {
    assert.ok(names("o:untap").includes("Seedborn Muse"));
    assert.ok(names('o:"add {w}"').includes("Witch Enchanter // Witch-Blessed Meadow"));
  });
  test("o: excludes reminder text; fo: includes it", () => {
    assert.ok(!names("o:destroy").includes("Typhoid Rats"));
    assert.ok(names("fo:destroy").includes("Typhoid Rats"));
  });
  test("~ substitutes the card name", () => {
    assert.ok(names('o:"~ enters tapped"').includes("Bojuka Bog"));
  });
});

describe("numbers", () => {
  test("cmc comparators and mv alias", () => {
    assert.ok(names("cmc<=1").includes("Sol Ring"));
    assert.ok(names("mv>=6").includes("Teferi, Temporal Archmage"));
    assert.ok(names("mv:even t:instant").includes("Counterspell"));
  });
  test("pow/tou comparisons skip non-numeric values", () => {
    assert.ok(names("pow>=2").includes("Seedborn Muse"));
    assert.ok(!names("pow>=0").includes("Tarmogoyf"));
    assert.deepEqual(names("pow>tou"), ["Ball Lightning", "Witch Enchanter // Witch-Blessed Meadow"]);
  });
  test("loyalty", () => {
    assert.deepEqual(names("loy=5"), ["Teferi, Temporal Archmage"]);
  });
});

describe("colors and identity", () => {
  test("c: means at-least; id: means within", () => {
    // Kenrith is a white card with a five-color identity
    assert.ok(names("c:w t:legend").includes("Kenrith, the Returned King"));
    assert.ok(!names("id:w").includes("Kenrith, the Returned King"));
    assert.ok(names("id:wubrg t:legend").includes("Kenrith, the Returned King"));
  });
  test("id<= nickname sets", () => {
    const esperLegal = names("id<=esper");
    assert.ok(esperLegal.includes("Counterspell"));
    assert.ok(esperLegal.includes("Sol Ring"));
    assert.ok(!esperLegal.includes("Llanowar Elves"));
  });
  test("colorless and multicolor", () => {
    assert.ok(names("c:c").includes("Sol Ring"));
    assert.ok(names("id:c t:artifact").includes("Sol Ring"));
    assert.ok(names("c:m").includes("Atraxa, Praetors' Voice"));
    assert.ok(!names("c:m").includes("Kenrith, the Returned King"));
  });
  test("color counts", () => {
    assert.ok(names("c=4").includes("Atraxa, Praetors' Voice"));
    assert.ok(names("id=5").includes("Golos, Tireless Pilgrim"));
  });
});

describe("commander filters", () => {
  test("is:commander covers legendary creatures and can-be-your-commander text", () => {
    const commanders = names("is:commander");
    assert.ok(commanders.includes("Atraxa, Praetors' Voice"));
    assert.ok(commanders.includes("Teferi, Temporal Archmage"));
    assert.ok(!commanders.includes("Llanowar Elves"));
  });
  test("f:commander excludes banned cards; banned:commander finds them", () => {
    assert.ok(!names("f:commander is:commander").includes("Golos, Tireless Pilgrim"));
    assert.deepEqual(names("banned:commander"), ["Golos, Tireless Pilgrim"]);
  });
});

describe("boolean logic", () => {
  test("negation", () => {
    const nonGreen = names("t:creature -c:g");
    assert.ok(!nonGreen.includes("Llanowar Elves"));
    assert.ok(nonGreen.includes("Ball Lightning"));
  });
  test("or with parentheses", () => {
    const r = names("(t:instant or t:artifact) cmc<=2");
    assert.deepEqual(r, ["Counterspell", "Sol Ring"]);
  });
});

describe("color identity pre-filter (spec §6.3)", () => {
  test("out-of-identity cards never come back", () => {
    // Simulate a mono-green deck: G bit = 16
    const r = names("t:instant or t:creature", { colorIdentityMask: 16 });
    assert.ok(!r.includes("Counterspell"));
    assert.ok(r.includes("Llanowar Elves"));
  });
});

describe("exact resolution and suggestions (spec §6.4)", () => {
  test("resolveExactName is exact only", () => {
    assert.equal(resolveExactName(db, "Seedborn Muse").length, 1);
    assert.equal(resolveExactName(db, "Seedborn Sage").length, 0);
  });
  test("suggestNames offers did-you-mean candidates", () => {
    assert.ok(suggestNames(db, "Seedborn Sage").includes("Seedborn Muse"));
  });
});

describe("errors", () => {
  test("unknown key, unbalanced parens, reserved keys", () => {
    assert.throws(() => parse("(t:creature"), SearchError);
    assert.throws(() => names("xyz:foo"), SearchError);
    assert.throws(() => names("slot:ramp"), /deck context/);
    assert.throws(() => names("is:funny"), /is:commander/);
  });
});
