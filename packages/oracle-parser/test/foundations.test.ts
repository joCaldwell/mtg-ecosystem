import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/index.ts";
import type { Ability, Effect, ObjectFilter, ParsedLine } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAmount, parseComparison, parseEqualTo, parseFilter, parseObjectRef, parsePlayerRef, parseZoneRef } from "../src/parser/refs.ts";
import { parseTrigger } from "../src/parser/triggers.ts";
import { parseSentences, type EffectContext } from "../src/parser/effects.ts";
import { parseCostList } from "../src/parser/costs.ts";

function one(text: string): Ability {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return result.abilities[0];
}
function steps(text: string): Effect[] {
  const ability = one(text);
  assert.equal(ability.kind, "spell");
  return ability.effects.flatMap(sentence => sentence.steps);
}
function filter(text: string): ObjectFilter {
  const step = steps(`Destroy target ${text}.`)[0];
  assert.equal(step.effect, "destroy");
  assert.equal(step.what.ref, "target");
  return step.what.filter;
}
function failure(text: string): string {
  const result = parseOracleText(text);
  assert.equal(result.ok, false, text);
  assert.equal("abilities" in result, false);
  const line = result.lines.find(line => !line.ok);
  assert.ok(line && !line.ok);
  return line.error;
}

describe("semantic distinctions", () => {
  it("distinguishes type intersection from union", () => {
    assert.deepEqual(filter("artifact creature"), { types: ["artifact", "creature"] });
    assert.deepEqual(filter("artifact or creature"), {
      allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }],
    });
    assert.deepEqual(filter("artifact or enchantment you control"), {
      allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["enchantment"] }] }], control: "you",
    });
  });
  it("distinguishes color conjunction from disjunction", () => {
    assert.deepEqual(filter("red and green creature"), { colors: ["red", "green"], types: ["creature"] });
    assert.deepEqual(filter("red or green creature"), {
      allOf: [{ anyOf: [{ colors: ["red"] }, { colors: ["green"] }] }], types: ["creature"],
    });
    assert.deepEqual(filter("blue, black, or red spell"), {
      allOf: [{ anyOf: [{ colors: ["blue"] }, { colors: ["black"] }, { colors: ["red"] }] }], cls: "spell",
    });
    failure("Destroy target red and green or blue creature.");
  });
  it("retains event grouping (CR 603.2c and 700.1)", () => {
    const single = one("Whenever a creature dies, draw a card.");
    const group = one("Whenever one or more creatures die, draw a card.");
    assert.equal(single.kind, "triggered"); assert.equal(group.kind, "triggered");
    const trigger = { trigger: "dies", what: { ref: "filter", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } } };
    assert.deepEqual(single.trigger, trigger);
    assert.deepEqual(group.trigger, { ...trigger, grouping: "one-or-more" });
  });
  it("retains both endpoints of graveyard events", () => {
    // Skola Grovedancer's text checked in the cached 2026-08-26 Scryfall corpus.
    const a = one("Whenever a land card is put into your graveyard from anywhere, you gain 1 life.");
    assert.equal(a.kind, "triggered");
    assert.deepEqual(a.trigger, {
      trigger: "put-into-graveyard",
      what: { ref: "filter", filter: { types: ["land"], cls: "card" }, count: { amount: "fixed", value: 1 } },
      from: "anywhere", to: { zone: "graveyard", owner: "your" },
    });
    const b = one("Whenever a land card is put into a graveyard from the battlefield, you gain 1 life.");
    assert.equal(b.kind, "triggered");
    assert.deepEqual(b.trigger, { ...a.trigger, from: "battlefield", to: { zone: "graveyard", owner: "any" } });
  });
  it("preserves the object being blocked", () => {
    const a = one("Whenever this creature blocks a red creature, draw a card.");
    assert.equal(a.kind, "triggered");
    assert.deepEqual(a.trigger, {
      trigger: "blocks", what: { ref: "this", noun: "creature" },
      blocking: { ref: "filter", filter: { colors: ["red"], types: ["creature"] }, count: { amount: "fixed", value: 1 } },
    });
  });
  it("preserves battlefield controllers and hand recipients", () => {
    const what = { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } };
    for (const [text, controller] of [["your", "you"], ["its owner's", "owner"]]) {
      assert.deepEqual(steps(`Return target creature to the battlefield under ${text} control.`), [
        { effect: "move-zone", what, to: { zone: "battlefield" }, controller },
      ]);
      assert.deepEqual(steps(`Return target creature to ${text} hand.`), [
        { effect: "return-to-hand", what, to: controller === "you" ? "your" : "owner" },
      ]);
    }
    failure("Return target creature to the battlefield under your.");
  });
  it("preserves search zone ownership", () => {
    for (const [text, owner] of [["your", "your"], ["an opponent's", "an-opponent"], ["each opponent's", "each-opponent"]]) {
      assert.deepEqual(steps(`Search ${text} library for a land card.`), [{
        effect: "search", who: { player: "you" }, zone: { zone: "library", owner },
        for: { ref: "filter", filter: { types: ["land"], cls: "card" }, count: { amount: "fixed", value: 1 } },
      }]);
    }
  });
  it("keeps shared durations within their coordinated instruction", () => {
    const effects = steps("Target creature gets +1/+1, then target creature gets +2/+2 and gains flying until end of turn.");
    assert.equal(effects[0].duration, undefined);
    assert.deepEqual(effects.slice(1).map(e => e.duration), [
      { duration: "end-of-turn" }, { duration: "end-of-turn" },
    ]);
    const separate = steps("Target creature gets +1/+1. Target creature gains flying until end of turn.");
    assert.equal(separate[0].duration, undefined);
  });
});

describe("complete supported ability blocks", () => {
  it("types restrictions on activated and loyalty abilities", () => {
    for (const prefix of ["{T}", "+1"]) {
      const a = one(`${prefix}: Draw a card. Activate only once each turn.`);
      assert.ok(a.kind === "activated" || a.kind === "loyalty");
      assert.deepEqual(a.restriction, { restriction: "once-each-turn" });
      const b = one(`${prefix}: Draw a card. Activate only as a sorcery.`);
      assert.ok(b.kind === "activated" || b.kind === "loyalty");
      assert.deepEqual(b.restriction, { restriction: "sorcery" });
      failure(`${prefix}: Draw a card. Activate nonsense.`);
      failure(`${prefix}: Draw a card. Activate only as a sorcery. Destroy target creature.`);
    }
  });
  it("rejects invalid mana in production, costs, and payment conditions", () => {
    for (const symbol of ["NOT_MANA", "", "T", "Q", "E", "2/P", "W/W", "W/C", "02"]) {
      failure(`Add {${symbol}}.`);
      failure(`Counter target spell unless its controller pays {${symbol}}.`);
    }
    failure("{NOT_MANA}: Draw a card.");
    assert.deepEqual(steps("Add {G}{G}."), [{ effect: "add-mana", mana: { mana: "fixed", symbols: ["G", "G"] } }]);
  });
  it("never drops orphaned bullets, invalid options, or extra clauses", () => {
    for (const text of [
      "Flying\n• Draw a card.", "• Draw a card.", "Choose one —\n• Frobnicate a card.",
      "Choose one —\n• Draw a card.\n• %", "Choose three —\n• Draw a card.",
      "{T},: Draw a card.", "{T}: Draw a card. Frobnicate.",
    ]) failure(text);
    const bad = parseOracleText("Choose one —\n• Draw a card.\n• %");
    assert.equal(bad.lines.length, 1);
    assert.equal(bad.lines[0].text, "Choose one —\n• Draw a card.\n• %");
  });
  it("reports failures in activated, loyalty, prefixed, and modal bodies", () => {
    assert.match(failure("{T}: draw a banana."), /"banana" \(token 4\)/);
    assert.match(failure("+1: draw a banana."), /"banana" \(token 5\)/);
    assert.match(failure("Landfall — Whenever a land enters, draw a banana."), /"banana" \(token 9\)/);
    assert.match(failure("Choose one —\n• draw a banana."), /modal option 1:.*"banana"/);
  });
  it("has discriminated success and failure results", () => {
    const result = parseOracleText("Flying\nFrobnicate.");
    assert.equal(result.ok, false);
    assert.equal("abilities" in result, false);
    assert.equal(result.lines[0].ok, true);
    // @ts-expect-error Success requires an ability.
    const invalid: ParsedLine = { text: "Flying", ok: true };
    void invalid;
  });
});

describe("transactional parser cursors", () => {
  it("restores cursors after incomplete reference, amount, trigger, and cost parses", () => {
    const cases: [string, (cursor: Cursor) => unknown][] = [
      ["twice", parseAmount], ["1 plus", parseAmount], ["less than", parseComparison],
      ["equal to", parseEqualTo], ["Whenever", parseTrigger], ["At the beginning of", parseTrigger],
      ["red", parseFilter], ["target", parseObjectRef], ["its", parsePlayerRef],
      ["your", parseZoneRef], ["Pay", parseCostList],
    ];
    for (const [text, parser] of cases) {
      const cursor = new Cursor(lex(`prefix ${text}`)); cursor.pos = 1;
      assert.equal(parser(cursor), null, text);
      assert.equal(cursor.pos, 1, text);
      assert.ok(cursor.farthest >= 1);
    }
  });
  it("restores elided-subject context when a sentence fails", () => {
    const cursor = new Cursor(lex("Target creature gets +1/+1 unsupported."));
    const ctx: EffectContext = { modalOptions: [], lastSubject: { ref: "self" } };
    assert.equal(parseSentences(cursor, ctx), null);
    assert.equal(cursor.pos, 0);
    assert.deepEqual(ctx.lastSubject, { ref: "self" });
  });
  it("does not advance past end of input", () => {
    const cursor = new Cursor([]);
    assert.equal(cursor.next(), undefined);
    assert.equal(cursor.pos, 0);
  });
});
