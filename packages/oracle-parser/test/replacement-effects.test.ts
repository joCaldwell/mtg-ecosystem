import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { ReplacementInstruction, Sentence, StaticEffect } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseSentences } from "../src/parser/effects.ts";
import { looksLikeStaticReplacement, parseReplacementSentence, parseStaticReplacement } from "../src/parser/replacement.ts";

// Named wording is verified against cached Scryfall oracle-cards.json.
// CR 614.1, 614.5–6, 614.11, 614.16, 616.1:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const you = { player: "you" } as const;
const creature = { ref: "filter", filter: { types: ["creature"], control: "you" }, count: one } as const;
const affected = { ref: "replacement-event-object", role: "affected" } as const;
const twice = { replacement: "modify-quantity", change: { operation: "multiply", factor: 2 } } as const;
const plusOne = { replacement: "modify-quantity", change: { operation: "add", amount: one } } as const;

function replacement(text: string): ReplacementInstruction {
  const c = new Cursor(lex(text));
  const result = parseStaticReplacement(c, parseSentences);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true);
  assert.equal(result.static, "replacement");
  return (result as Extract<StaticEffect, { static: "replacement" }>).replacement;
}

function dynamic(text: string): Sentence {
  const c = new Cursor(lex(text));
  const result = parseReplacementSentence(c, { modalOptions: [] }, parseSentences);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true);
  return result;
}

describe("replacement event identity and lifetime", () => {
  it("matches dying objects individually and binds the replaced object (Vren)", () => {
    assert.deepEqual(replacement("If a creature an opponent controls would die, exile it instead."), {
      event: { event: "move-zone", what: { ref: "filter", filter: { types: ["creature"], control: "opponent" }, count: one },
        from: { zone: "battlefield" }, to: { zone: "graveyard" } },
      occurrence: "each", replace: { replacement: "effects", effects: [{ steps: [{ effect: "exile", what: affected }] }] },
    });
  });

  it("preserves graveyard ownership and any origin (Leyline of the Void)", () => {
    assert.deepEqual(replacement("If a card would be put into an opponent's graveyard from anywhere, exile it instead."), {
      event: { event: "move-zone", what: { ref: "filter", filter: { cls: "card" }, count: one },
        to: { zone: "graveyard", owner: "an-opponent" }, from: "anywhere" },
      occurrence: "each", replace: { replacement: "effects", effects: [{ steps: [{ effect: "exile", what: affected }] }] },
    });
  });

  it("does not narrow a card-or-token union to cards (Rest in Peace)", () => {
    const result = replacement("If a card or token would be put into a graveyard from anywhere, exile it instead.");
    assert.deepEqual(result.event, { event: "move-zone", what: { ref: "filter",
      filter: { allOf: [{ anyOf: [{ cls: "card" }, { cls: "token" }] }] }, count: one },
      to: { zone: "graveyard", owner: "any" }, from: "anywhere" });
  });

  it("creates a temporary replacement instead of executing exile now (Puncturing Blow)", () => {
    const text = "If that creature would die this turn, exile it instead.";
    assert.deepEqual(dynamic(text), { steps: [{ effect: "create-replacement", duration: { duration: "this-turn" }, replacement: {
      event: { event: "move-zone", what: { ref: "that", filter: { types: ["creature"] } }, from: { zone: "battlefield" }, to: { zone: "graveyard" } },
      occurrence: "each", replace: { replacement: "effects", effects: [{ steps: [{ effect: "exile", what: affected }] }] },
    } }] });
    const c = new Cursor(lex(text));
    assert.equal(parseStaticReplacement(c, parseSentences), null);
    assert.equal(c.pos, 0);
    assert.equal(looksLikeStaticReplacement(c), false);
    const full = parseOracleText("Puncturing Blow deals 5 damage to target creature. " + text, "Puncturing Blow");
    assert.equal(full.ok, true);
  });

  it("keeps lifetime and next occurrence distinct from a permanent static ability", () => {
    const sentence = dynamic("The next time you would draw a card this turn, draw two cards instead.");
    assert.deepEqual(sentence, { steps: [{ effect: "create-replacement", duration: { duration: "this-turn" }, replacement: {
      event: { event: "draw", who: you, quantity: { op: "eq", value: one } }, occurrence: "next",
      replace: { replacement: "modify-quantity", change: { operation: "set", amount: { amount: "fixed", value: 2 } } },
    } }] });
    const c = new Cursor(lex("The next time you would draw a card, draw two cards instead."));
    assert.equal(parseStaticReplacement(c, parseSentences), null);
    assert.equal(c.pos, 0);
    assert.equal(looksLikeStaticReplacement(c), false);
  });

  it("keeps optionality on the replacement choice (Pursuit of Knowledge)", () => {
    assert.deepEqual(replacement("If you would draw a card, you may put a study counter on this enchantment instead."), {
      event: { event: "draw", who: you, quantity: { op: "eq", value: one } }, occurrence: "each", optional: { who: you },
      replace: { replacement: "effects", effects: [{ steps: [{ effect: "put-counters", counter: { type: "study" }, count: one, on: { ref: "this", noun: "enchantment" } }] }] },
    });
  });

  it("binds an explicit counter-placement actor to the watched event", () => {
    assert.deepEqual(replacement("If an opponent would draw a card, that player puts a +1/+1 counter on this creature instead."), {
      event: { event: "draw", who: { player: "an-opponent" }, quantity: { op: "eq", value: one } }, occurrence: "each",
      replace: { replacement: "effects", effects: [{ steps: [{ effect: "put-counters", counter: { type: "+1/+1" }, count: one,
        on: { ref: "this", noun: "creature" }, who: { player: "replacement-event-player", role: "actor" } }] }] },
    });
  });
});

describe("replacement quantities preserve the complete original event", () => {
  it("keeps an effect cause distinct from all causes (Doubling Season / Primal Vigor)", () => {
    assert.deepEqual(replacement("If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead."), {
      event: { event: "create-token", tokens: { cls: "token" }, cause: "effect", quantity: { op: "ge", value: one }, under: you, match: "creation-event" },
      occurrence: "each", replace: twice,
    });
    assert.deepEqual(replacement("If one or more tokens would be created, twice that many of those tokens are created instead."), {
      event: { event: "create-token", tokens: { cls: "token" }, cause: "any", quantity: { op: "ge", value: one }, match: "creation-event" },
      occurrence: "each", replace: twice,
    });
  });

  it("preserves counter kinds, eligible recipients and per-kind arithmetic", () => {
    assert.deepEqual(replacement("If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."), {
      event: { event: "put-counters", on: [creature], counter: { type: "+1/+1" }, cause: "any", quantity: { op: "ge", value: one } },
      occurrence: "each", replace: plusOne,
    });
    assert.deepEqual(replacement("If an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead."), {
      event: { event: "put-counters", on: [{ ref: "filter", filter: { cls: "permanent", control: "you" }, count: one }], counter: "any", cause: "effect", quantity: { op: "ge", value: one } },
      occurrence: "each", replace: twice,
    });
  });

  it("preserves the player putting counters and recipient alternatives (Vorinclex)", () => {
    assert.deepEqual(replacement("If you would put one or more counters on a permanent or player, put twice that many of each of those kinds of counters on that permanent or player instead."), {
      event: { event: "put-counters", by: you, on: [{ ref: "filter", filter: { cls: "permanent" }, count: one }, { player: "a-player" }], counter: "any", cause: "any", quantity: { op: "ge", value: one } },
      occurrence: "each", replace: twice,
    });
  });

  it("halves with explicit rounding and subtracts counters without changing their kinds", () => {
    const half = replacement("If an opponent would put one or more counters on a permanent or player, they put half that many of each of those kinds of counters on that permanent or player instead, rounded down.");
    assert.deepEqual(half.replace, { replacement: "modify-quantity", change: { operation: "half", round: "down" } });
    assert.ok(half.event.event === "put-counters");
    assert.deepEqual(half.event.by, { player: "an-opponent" });
    assert.equal(half.event.counter, "any");
    const subtract = replacement("If one or more -1/-1 counters would be put on a creature you control, that many -1/-1 counters minus one are put on it instead.");
    assert.deepEqual(subtract.replace, { replacement: "modify-quantity", change: { operation: "subtract", amount: one } });
    assert.ok(subtract.event.event === "put-counters");
    assert.deepEqual(subtract.event.counter, { type: "-1/-1" });
    assert.deepEqual(replacement("If you would get one or more counters, you get that many plus one of each of those kinds of counters instead."), {
      event: { event: "put-counters", on: [you], counter: "any", cause: "any", quantity: { op: "ge", value: one } }, occurrence: "each", replace: plusOne,
    });
  });

  it("preserves original token kinds when adding a different token (Peregrin Took)", () => {
    assert.deepEqual(replacement("If one or more tokens would be created under your control, those tokens plus an additional Food token are created instead."), {
      event: { event: "create-token", tokens: { cls: "token" }, cause: "any", quantity: { op: "ge", value: one }, under: you, match: "creation-event" },
      occurrence: "each", replace: { replacement: "augment-tokens", count: one, token: { predefined: "Food" } },
    });
    assert.deepEqual(replacement("If you would create one or more Treasure tokens, instead create those tokens plus an additional Treasure token."), {
      event: { event: "create-token", tokens: { subtypes: ["Treasure"], cls: "token" }, cause: "any", quantity: { op: "ge", value: one }, under: you, match: "creation-event" },
      occurrence: "each", replace: { replacement: "augment-tokens", count: one, token: { predefined: "Treasure" } },
    });
  });

  it("counts total original tokens for Chatterfang's extra Squirrels", () => {
    const result = replacement("If one or more tokens would be created under your control, those tokens plus that many 1/1 green Squirrel creature tokens are created instead.");
    assert.deepEqual(result.replace, { replacement: "augment-tokens", count: { amount: "replacement-event-quantity" }, token: {
      power: one, toughness: one, colors: ["green"], subtypes: ["Squirrel"], types: ["creature"], supertypes: [], keywords: [],
    } });
  });

  it("replaces the token specifications rather than adding Angels (Divine Visitation)", () => {
    const result = replacement("If one or more creature tokens would be created under your control, that many 4/4 white Angel creature tokens with flying and vigilance are created instead.");
    assert.deepEqual(result.replace, { replacement: "replace-token-spec", count: { amount: "replacement-event-quantity" }, token: {
      power: { amount: "fixed", value: 4 }, toughness: { amount: "fixed", value: 4 }, colors: ["white"], subtypes: ["Angel"], types: ["creature"],
      supertypes: [], keywords: [{ keyword: "flying" }, { keyword: "vigilance" }],
    } });
  });

  it("replaces EACH matching Fish and preserves original creation modifiers (Fisher's Talent)", () => {
    // WotC explicitly keeps tapped/attacking/haste/delayed-exile instructions:
    // https://magic.wizards.com/en/news/feature/bloomburrow-release-notes
    const result = replacement("If you would create a Fish token, create a 3/3 blue Shark creature token instead.");
    assert.deepEqual(result, {
      event: { event: "create-token", under: you, tokens: { subtypes: ["Fish"], cls: "token" }, cause: "any", quantity: { op: "eq", value: one }, match: "each-token" },
      occurrence: "each", replace: { replacement: "replace-token-spec", count: one, token: {
        power: { amount: "fixed", value: 3 }, toughness: { amount: "fixed", value: 3 }, colors: ["blue"], subtypes: ["Shark"], types: ["creature"], supertypes: [], keywords: [],
      } },
    });
    assert.notEqual(result.replace.replacement, "effects");
  });

  it("doubles or adds to life gain without changing its recipient", () => {
    assert.deepEqual(replacement("If you would gain life, you gain twice that much life instead."), {
      event: { event: "gain-life", who: you }, occurrence: "each", replace: twice,
    });
    assert.deepEqual(replacement("If you would gain life, you gain that much life plus 1 instead."), {
      event: { event: "gain-life", who: you }, occurrence: "each", replace: plusOne,
    });
  });

  it("binds cross-event quantities to the watched gain, not an earlier instruction", () => {
    assert.deepEqual(replacement("If an opponent would gain life, that player loses that much life instead."), {
      event: { event: "gain-life", who: { player: "an-opponent" } }, occurrence: "each",
      replace: { replacement: "effects", effects: [{ steps: [{ effect: "lose-life", who: { player: "replacement-event-player", role: "actor" }, amount: { amount: "replacement-event-quantity" } }] }] },
    });
    assert.deepEqual(replacement("If you would gain life, draw that many cards instead."), {
      event: { event: "gain-life", who: you }, occurrence: "each",
      replace: { replacement: "effects", effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "replacement-event-quantity" } }] }] },
    });
  });

  it("retains the first draw in each draw step exception (Teferi's Ageless Insight)", () => {
    assert.deepEqual(replacement("If you would draw a card except the first one you draw in each of your draw steps, draw two cards instead."), {
      event: { event: "draw", who: you, quantity: { op: "eq", value: one }, except: "first-in-each-draw-step" }, occurrence: "each",
      replace: { replacement: "modify-quantity", change: { operation: "set", amount: { amount: "fixed", value: 2 } } },
    });
  });

  it("preserves damage source, recipient union and multipliers (Furnace of Rath)", () => {
    assert.deepEqual(replacement("If a source would deal damage to a permanent or player, it deals double that damage to that permanent or player instead."), {
      event: { event: "damage", kind: "all", source: { ref: "filter", filter: {}, count: one }, to: [{ ref: "filter", filter: { cls: "permanent" }, count: one }, { player: "a-player" }] },
      occurrence: "each", replace: twice,
    });
  });

  it("keeps Torbran's opponent scope and red controlled source", () => {
    assert.deepEqual(replacement("If a red source you control would deal damage to an opponent or a permanent an opponent controls, it deals that much damage plus 2 instead."), {
      event: { event: "damage", kind: "all", source: { ref: "filter", filter: { colors: ["red"], control: "you" }, count: one }, to: [{ player: "an-opponent" }, { ref: "filter", filter: { cls: "permanent", control: "opponent" }, count: one }] },
      occurrence: "each", replace: { replacement: "modify-quantity", change: { operation: "add", amount: { amount: "fixed", value: 2 } } },
    });
  });

  it("keeps combat damage separate and triples rather than doubles", () => {
    assert.deepEqual(replacement("If this creature would deal combat damage to a player, it deals triple that damage to that player instead."), {
      event: { event: "damage", kind: "combat", source: { ref: "this", noun: "creature" }, to: [{ player: "a-player" }] },
      occurrence: "each", replace: { replacement: "modify-quantity", change: { operation: "multiply", factor: 3 } },
    });
  });

  it("binds damage-to-counter replacements to the watched recipient (Soul-Scar Mage)", () => {
    const result = replacement("If a source you control would deal noncombat damage to a creature an opponent controls, put that many -1/-1 counters on that creature instead.");
    assert.deepEqual(result.replace, { replacement: "effects", effects: [{ steps: [{ effect: "put-counters", counter: { type: "-1/-1" }, count: { amount: "replacement-event-quantity" }, on: { ref: "replacement-event-object", role: "recipient" } }] }] });
    assert.ok(result.event.event === "damage");
    assert.equal(result.event.kind, "noncombat");
    assert.deepEqual(result.event.to, [{ ref: "filter", filter: { types: ["creature"], control: "opponent" }, count: one }]);
  });

  it("binds watched-object attributes without capturing a later quantity (Ravenous Slime)", () => {
    const result = replacement("If a creature an opponent controls would die, instead exile it and put a number of +1/+1 counters equal to that creature's power on this creature.");
    assert.deepEqual(result.replace, { replacement: "effects", effects: [{ steps: [
      { effect: "exile", what: affected },
      { effect: "put-counters", counter: { type: "+1/+1" }, count: { amount: "attribute", of: affected, attribute: "power" }, on: { ref: "this", noun: "creature" } },
    ] }] });
  });

  it("excludes the equipped recipient for Inquisitor's Flail and source for Benevolent Hydra", () => {
    const flail = replacement("If another creature would deal combat damage to equipped creature, it deals double that damage to equipped creature instead.");
    assert.ok(flail.event.event === "damage");
    assert.deepEqual(flail.event.source, { ref: "filter", filter: { types: ["creature"], otherThan: { ref: "replacement-event-object", role: "recipient" } }, count: one });
    const hydra = replacement("If one or more +1/+1 counters would be put on another creature you control, that many plus one +1/+1 counters are put on it instead.");
    assert.ok(hydra.event.event === "put-counters");
    assert.deepEqual(hydra.event.on, [{ ref: "filter", filter: { types: ["creature"], control: "you", otherThan: { ref: "self" } }, count: one }]);
  });
});

describe("replacement scope boundaries reject unsupported meaning", () => {
  it("rejects incomplete riders and mismatched counter/recipient/source bindings with full rewind", () => {
    for (const text of [
      "If a creature would die, exile that land instead.",
      "If one or more +1/+1 counters would be put on a creature, twice that many -1/-1 counters are put on it instead.",
      "If one or more +1/+1 counters would be put on a creature, twice that many +1/+1 counters are put on that land instead.",
      "If a source would deal damage to a player, this creature deals twice that much damage instead.",
      "If a source would deal damage to a player, it deals twice that much damage to you instead.",
      "If a card would be put into a graveyard from anywhere except from a library, exile it instead.",
      "If an effect would create one or more tokens, it creates twice that many of those tokens and an extra token instead.",
      "If you would gain life while the moon is full, you gain twice that much life instead.",
      "If you would gain life while, you gain twice that much life instead.",
      "If you would draw a card except the first one you draw each turn, draw two cards instead.",
      "If you would gain life, you gain twice that much life instead, rounded up.",
      "If you would gain life, you gain half that much life instead.",
      "If you would gain life, gain 2 life and draw that many cards instead.",
      "If a creature would die, create that many Treasure tokens instead.",
      "If another creature would deal damage to a creature, it deals double that damage instead.",
      "If you would draw a card, reveal the top card of your library instead. If it's a creature card, put it into your graveyard.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseStaticReplacement(c, parseSentences), null, text);
      assert.equal(c.pos, 0, text);
    }
  });

  it("does not fall through an unsupported permanent static into a spell", () => {
    const text = "If you would draw a card, draw two cards instead. Put a card from your hand on top of your library.";
    const c = new Cursor(lex(text));
    assert.equal(looksLikeStaticReplacement(c), true);
    assert.equal(c.pos, 0);
    assert.equal(parseOracleText(text).ok, false);
  });

  it("full parser stores a static replacement rather than an ordinary if or a trigger", () => {
    const text = "If you would gain life, you gain twice that much life instead.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true);
    if (result.ok && result.lines[0].ok) assert.deepEqual(result.lines[0].ability, { kind: "static", abilityWord: undefined, effect: { static: "replacement", replacement: replacement(text) } });
  });
});
