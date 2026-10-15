import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef, Trigger } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseSentences } from "../src/parser/effects.ts";
import { parseDelayedPrefix, parseDelayedSpellPrefix, parseDelayedSuffix } from "../src/parser/delayed.ts";
import { parseTrigger, parseTriggerLimit } from "../src/parser/triggers.ts";

// Oracle clauses checked against corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 601.2a/i, 603.1, 603.2e/h, 603.4, 603.7a–c, 701.37, 702.112:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const you = { player: "you" } as const;
const source: ObjectRef = { ref: "this", noun: "creature" };
const creature: ObjectRef = { ref: "filter", filter: { types: ["creature"] }, count: one };
const instantOrSorcery = { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } as const;
const draw: Effect = { effect: "draw", who: you, amount: one };

function full(text: string): Ability[] {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  // Assert the full serialized representation, the contract stored by the
  // archive. Optional undefined properties have no serialized meaning.
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function trigger(text: string): Trigger {
  const c = new Cursor(lex(text));
  const result = parseTrigger(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return JSON.parse(JSON.stringify(result)) as Trigger;
}
function delayed(text: string): Effect[] {
  const c = new Cursor(lex(text));
  const result = parseDelayedPrefix(c, { modalOptions: [] }, parseSentences);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return JSON.parse(JSON.stringify(result)) as Effect[];
}

describe("trigger event meaning and qualifiers", () => {
  it("preserves monstrous and renowned transitions as distinct events", () => {
    // Alpha Deathclaw 0a7d0d1f-a5ac-4db9-860c-9fcbe3fd9e71.
    assert.deepEqual(full("When this creature enters or becomes monstrous, destroy target permanent."), [{
      kind: "triggered", trigger: { trigger: "any-of", events: [
        { trigger: "enters", what: source },
        { trigger: "becomes-status", what: source, status: "monstrous" },
      ] }, effects: [{ steps: [{ effect: "destroy", what: { ref: "target", filter: { cls: "permanent" }, count: one } }] }],
    }]);
    // Valeron Wardens: becoming renowned is not merely having counters.
    assert.deepEqual(full("Whenever a creature you control becomes renowned, draw a card."), [{
      kind: "triggered", trigger: { trigger: "becomes-status", status: "renowned",
        what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: one } },
      effects: [{ steps: [draw] }],
    }]);
    for (const header of ["When this creature is monstrous", "When this creature becomes", "When this creature becomes legendary"])
      assert.equal(parseOracleText(`${header}, draw a card.`).ok, false, header);
  });

  it("preserves all three Waste Not filters and their different effects", () => {
    // Waste Not 00fdcc19-88ed-46c3-91f0-095806228105.
    assert.deepEqual(full([
      "Whenever an opponent discards a creature card, create a 2/2 black Zombie creature token.",
      "Whenever an opponent discards a land card, add {B}{B}.",
      "Whenever an opponent discards a noncreature, nonland card, draw a card.",
    ].join("\n")), [
      { kind: "triggered", trigger: { trigger: "discards", who: { player: "an-opponent" },
        what: { ref: "filter", filter: { types: ["creature"], cls: "card" }, count: one } },
      effects: [{ steps: [{ effect: "create-token", count: one, token: {
        colors: ["black"], supertypes: [], types: ["creature"], subtypes: ["Zombie"], keywords: [],
        power: { amount: "fixed", value: 2 }, toughness: { amount: "fixed", value: 2 },
      } }] }] },
      { kind: "triggered", trigger: { trigger: "discards", who: { player: "an-opponent" },
        what: { ref: "filter", filter: { types: ["land"], cls: "card" }, count: one } },
      effects: [{ steps: [{ effect: "add-mana", who: you, mana: { mana: "fixed", symbols: ["B", "B"] } }] }] },
      { kind: "triggered", trigger: { trigger: "discards", who: { player: "an-opponent" },
        what: { ref: "filter", filter: { nonTypes: ["creature", "land"], cls: "card" }, count: one } },
      effects: [{ steps: [draw] }] },
    ]);
    assert.deepEqual(trigger("Whenever you discard one or more cards"), {
      trigger: "discards", who: you, what: { ref: "filter", filter: { cls: "card" }, count: one }, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever a player discards a creature card"), {
      trigger: "discards", who: { player: "a-player" }, what: { ref: "filter", filter: { types: ["creature"], cls: "card" }, count: one },
    });
  });

  it("preserves zone-change source, group, and turn qualifiers", () => {
    // Thran Vigil 01c179aa-533e-47e1-b6e5-c35587a39680.
    assert.deepEqual(full("Whenever one or more artifact and/or creature cards leave your graveyard during your turn, put a +1/+1 counter on target creature you control."), [{
      kind: "triggered", trigger: { trigger: "leaves-zone", from: { zone: "graveyard", owner: "your" }, grouping: "one-or-more", during: "your",
        what: { ref: "filter", count: one, filter: { allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }], cls: "card" } } },
      effects: [{ steps: [{ effect: "put-counters", counter: { type: "+1/+1" }, count: one,
        on: { ref: "target", filter: { types: ["creature"], control: "you" }, count: one } }] }],
    }]);
    assert.deepEqual(trigger("When this creature enters from your graveyard under your control"), {
      trigger: "enters", what: source, from: { zone: "graveyard", owner: "your" }, controller: you,
    });
    assert.deepEqual(trigger("When this creature enters under your control from your graveyard"), {
      trigger: "enters", what: source, from: { zone: "graveyard", owner: "your" }, controller: you,
    });
    assert.deepEqual(trigger("When this creature leaves the battlefield"), { trigger: "leaves", what: source });
    assert.deepEqual(trigger("Whenever a creature card leaves an opponent's graveyard"), {
      trigger: "leaves-zone", what: { ref: "filter", filter: { types: ["creature"], cls: "card" }, count: one },
      from: { zone: "graveyard", owner: "an-opponent" },
    });
    for (const header of ["When this creature enters from", "When this creature leaves your", "When this creature leaves your battlefield"])
      assert.equal(parseOracleText(`${header}, draw a card.`).ok, false, header);
  });

  it("keeps cast origins out of the current spell filter and rejects ambiguous source scopes", () => {
    assert.deepEqual(trigger("Whenever you cast a spell from your graveyard"), {
      trigger: "cast", who: you, what: { cls: "spell" }, from: { zone: "graveyard", owner: "your" },
    });
    assert.deepEqual(trigger("Whenever you cast an instant or sorcery spell from your hand"), {
      trigger: "cast", who: you, what: instantOrSorcery, from: { zone: "hand", owner: "your" },
    });
    for (const text of [
      "Whenever you cast a spell in your graveyard", "Whenever you copy a spell from your graveyard",
      "Whenever you cast or copy a spell from your graveyard", "Whenever you cast a spell from your hand from exile",
      "Whenever you cast an instant spell from your hand or a sorcery spell from your graveyard",
    ]) assert.equal(parseOracleText(`${text}, draw a card.`).ok, false, text);
  });

  it("keeps ordinal and turn restrictions local to independently introduced clauses", () => {
    assert.deepEqual(trigger("When this creature enters and whenever you cast a spell for the first time each turn during your turn"), {
      trigger: "any-of", events: [
        { trigger: "enters", what: source },
        { trigger: "cast", who: you, what: { cls: "spell" }, occurrence: { ordinal: 1, period: "turn" }, during: "your" },
      ],
    });
    assert.deepEqual(trigger("Whenever this creature attacks or dies during your turn"), {
      trigger: "any-of", events: [{ trigger: "attacks", what: source }, { trigger: "dies", what: source }], during: "your",
    });
  });
});

describe("ability trigger limits are not event ordinals", () => {
  it("stores Academy Wall's limit outside its optional and conditional resolution", () => {
    assert.deepEqual(full("Whenever you cast an instant or sorcery spell, you may draw a card. If you do, discard a card. This ability triggers only once each turn."), [{
      kind: "triggered", trigger: { trigger: "cast", who: you, what: instantOrSorcery },
      effects: [
        { steps: [{ ...draw, optional: true }] },
        { steps: [{ effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: one } }], condition: { condition: "you-do" }, conditionKind: "if" },
      ], limit: { maximum: 1, period: "turn" },
    }]);
    assert.deepEqual(full("Whenever you draw your second card each turn, draw a card. This ability triggers only twice each turn."), [{
      kind: "triggered", trigger: { trigger: "draws", who: you, occurrence: { ordinal: 2, period: "turn" } },
      effects: [{ steps: [draw] }], limit: { maximum: 2, period: "turn" },
    }]);
  });

  it("represents Acrobatic Cheerleader's once limit per ability instance", () => {
    assert.deepEqual(full("Survival — At the beginning of your second main phase, if this creature is tapped, put a flying counter on it. This ability triggers only once."), [{
      kind: "triggered", abilityWord: "Survival", trigger: { trigger: "phase", phase: "second-main", whose: "your" },
      condition: { condition: "status", what: source, status: "tapped" },
      effects: [{ steps: [{ effect: "put-counters", count: one, counter: { type: "flying" }, on: { ref: "it" } }] }],
      limit: { maximum: 1, period: "ability-instance" },
    }]);
  });

  it("distinguishes the second main phase from every postcombat main phase", () => {
    // CR505.1a/b: additional main phases make these observably different.
    // First/precombat remain equivalent, including turns with extra phases.
    for (const [text, phase] of [
      ["first main phase", "precombat-main"],
      ["precombat main phase", "precombat-main"],
      ["second main phase", "second-main"],
      ["postcombat main phase", "postcombat-main"],
    ]) assert.deepEqual(full(`At the beginning of your ${text}, draw a card.`), [{
      kind: "triggered", trigger: { trigger: "phase", phase, whose: "your" }, effects: [{ steps: [draw] }],
    }]);
  });

  it("does not reinterpret action limits or attach trigger limits to activated or spell instructions", () => {
    for (const text of [
      "Whenever this creature dies, draw a card. Do this only once each turn.",
      "Draw a card. This ability triggers only once each turn.",
      "{1}: Draw a card. This ability triggers only once each turn.",
      "Whenever this creature dies, draw a card. This ability triggers only once each combat.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    for (const text of ["This ability triggers only once each", "This ability triggers only once. Draw a card.", "Do this only once each turn."]) {
      const c = new Cursor(lex(text));
      assert.equal(parseTriggerLimit(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });
});

describe("delayed event lifetime and occurrence", () => {
  it("classifies Ondu Rising's bounded instruction as a spell creating a repeating trigger", () => {
    assert.deepEqual(full("Whenever a creature attacks this turn, it gains lifelink until end of turn."), [{
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "attacks", what: creature },
        occurrence: "each", duration: { duration: "this-turn" }, targets: "delayed-ability",
        effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "lifelink" } }], duration: { duration: "end-of-turn" } }] }],
      }] }],
    }]);
    assert.deepEqual(delayed("When that creature dies this turn, draw a card."), [{
      effect: "create-delayed-trigger", trigger: { trigger: "dies", what: { ref: "that", filter: { types: ["creature"] } } },
      occurrence: "each", duration: { duration: "this-turn" }, targets: "delayed-ability", effects: [{ steps: [draw] }],
    }]);
    assert.deepEqual(delayed("Whenever a creature attacks this combat, draw a card."), [{
      effect: "create-delayed-trigger", trigger: { trigger: "attacks", what: creature }, occurrence: "each",
      duration: { duration: "this-combat" }, targets: "delayed-ability", effects: [{ steps: [draw] }],
    }]);
  });

  it("keeps explicit next-cast limits even when a duration applies", () => {
    // Teach by Example d85ba214-fa05-44ca-a7f9-ab2c5dc07962.
    assert.deepEqual(full("When you next cast an instant or sorcery spell this turn, copy that spell. You may choose new targets for the copy."), [{
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "cast", who: you, what: instantOrSorcery },
        occurrence: "next", nextOnly: true, duration: { duration: "this-turn" }, targets: "delayed-ability", effects: [
          { steps: [{ effect: "copy-stack-object", what: { ref: "that", filter: { cls: "spell" } }, who: you, count: one, retarget: { who: you, optional: true } }] },
        ],
      }] }],
    }]);
    // Narset Transcendent's -2 clause: the source zone is event history.
    assert.deepEqual(full("-2: When you next cast an instant or sorcery spell from your hand this turn, it gains rebound."), [{
      kind: "loyalty", cost: { sign: -1, amount: 2 }, effects: [{ steps: [{ effect: "create-delayed-trigger",
        trigger: { trigger: "cast", who: you, what: instantOrSorcery, from: { zone: "hand", owner: "your" } },
        occurrence: "next", nextOnly: true, duration: { duration: "this-turn" }, targets: "delayed-ability",
        effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "rebound" } }] }] }],
      }] }],
    }]);
    for (const [header, occurrence, nextOnly] of [
      ["whenever a creature dies", "each", false], ["when a creature next dies", "next", true],
    ] as const) assert.deepEqual(full(`Until end of turn, ${header}, draw a card.`), [{
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "dies", what: creature },
        occurrence, ...(nextOnly ? { nextOnly: true } : {}), duration: { duration: "end-of-turn" }, targets: "delayed-ability", effects: [{ steps: [draw] }],
      }] }],
    }]);
  });

  it("keeps intervening conditions on the delayed ability, not only its resolution body", () => {
    assert.deepEqual(delayed("At the beginning of your next end step, if you control a creature, draw a card."), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "your" },
      occurrence: "next", nextOnly: true, targets: "delayed-ability", condition: { condition: "control", who: you, what: creature },
      effects: [{ steps: [draw] }],
    }]);
    const c = new Cursor(lex("when this creature dies this turn"));
    assert.deepEqual(parseDelayedSuffix(c, [draw]), [{ effect: "create-delayed-trigger", trigger: { trigger: "dies", what: source },
      occurrence: "each", duration: { duration: "this-turn" }, effects: [{ steps: [draw] }], targets: "enclosing-ability" }]);
    assert.equal(c.done(), true);
  });

  it("rejects targets declared inside unmodeled delayed-condition scopes", () => {
    for (const condition of ["target creature is tapped", "target player has ten or more life"]) {
      const text = `At the beginning of your next end step, if ${condition}, draw a card.`;
      const c = new Cursor(lex(text));
      assert.equal(parseDelayedPrefix(c, { modalOptions: [] }, parseSentences), null);
      assert.equal(c.pos, 0);
      assert.equal(parseOracleText(`Draw a card. ${text}`).ok, false);
    }
  });

  it("separates targets chosen for the event header from targets in the eventual body", () => {
    // Graceful Reprieve 76cef1af-ff7e-4eab-9be6-81ba6049d222.
    const target: ObjectRef = { ref: "target", filter: { types: ["creature"] }, count: one };
    assert.deepEqual(full("When target creature dies this turn, return that card to the battlefield under its owner's control."), [{
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "dies", what: target },
        occurrence: "each", duration: { duration: "this-turn" }, triggerTargets: "enclosing-ability", targets: "delayed-ability",
        effects: [{ steps: [{ effect: "move-zone", what: { ref: "that", filter: { cls: "card" } }, to: { zone: "battlefield" }, controller: "owner" }] }],
      }] }],
    }]);
    assert.deepEqual(delayed("When target creature dies this turn, destroy target creature."), [{
      effect: "create-delayed-trigger", trigger: { trigger: "dies", what: target }, occurrence: "each", duration: { duration: "this-turn" },
      triggerTargets: "enclosing-ability", targets: "delayed-ability", effects: [{ steps: [{ effect: "destroy", what: target }] }],
    }]);
    // A target first declared in the delayed body belongs to that ability.
    assert.deepEqual(delayed("At the beginning of the next end step, choose target creature. If you do, destroy it."), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      occurrence: "next", nextOnly: true, targets: "delayed-ability", effects: [
        { steps: [{ effect: "declare-target", what: target }] },
        { steps: [{ effect: "destroy", what: { ref: "it" } }], condition: { condition: "you-do" }, conditionKind: "if" },
      ],
    }]);
  });

  it("retains proven dependent continuation sentences in the delayed body", () => {
    // The Last Ronin chapter III keeps the ability grant with the attack.
    assert.deepEqual(full("Whenever a creature you control attacks alone this turn, put three +1/+1 counters on it. It gains trample, lifelink, and indestructible until end of turn."), [{
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger",
        trigger: { trigger: "attacks", what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: one }, alone: true },
        occurrence: "each", duration: { duration: "this-turn" }, targets: "delayed-ability", effects: [
          { steps: [{ effect: "put-counters", counter: { type: "+1/+1" }, count: { amount: "fixed", value: 3 }, on: { ref: "it" } }] },
          { steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [
            { gained: "keyword", keyword: { keyword: "trample" } }, { gained: "keyword", keyword: { keyword: "lifelink" } },
            { gained: "keyword", keyword: { keyword: "indestructible" } },
          ], duration: { duration: "end-of-turn" } }] },
        ],
      }] }],
    }]);
    // Consuming Rage creates the end-of-combat destruction only on attack.
    assert.deepEqual(full("Whenever a Minotaur attacks this turn, it gets +2/+0 until end of turn. Destroy that creature at end of combat."), [{
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger",
        trigger: { trigger: "attacks", what: { ref: "filter", filter: { subtypes: ["Minotaur"] }, count: one } },
        occurrence: "each", duration: { duration: "this-turn" }, targets: "delayed-ability", effects: [
          { steps: [{ effect: "pump", what: { ref: "it" }, power: { sign: 1, amount: { amount: "fixed", value: 2 } },
            toughness: { sign: 1, amount: { amount: "fixed", value: 0 } }, duration: { duration: "end-of-turn" } }] },
          { steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-of-combat", whose: "each-player" },
            occurrence: "next", targets: "enclosing-ability", effects: [{ steps: [{ effect: "destroy", what: { ref: "that", filter: { types: ["creature"] } } }] }],
          }] },
        ],
      }] }],
    }]);
  });

  it("rejects unproven sentence scope instead of silently postponing immediate actions", () => {
    // Time to Feed 59642d6f-8a25-4f79-9e81-643eac775658 has an immediate
    // fight after its delayed life-gain instruction. Until that boundary is
    // represented by a supported production, the entire ability must fail.
    assert.equal(parseOracleText("Choose target creature an opponent controls. When that creature dies this turn, you gain 3 life. Target creature you control fights that creature.").ok, false);
    for (const text of [
      "When that creature dies this turn, draw a card. You gain 1 life.",
      "At the beginning of your next end step, draw a card. Target creature gets +1/+1 until end of turn.",
      "When you do, draw a card. You gain 1 life.",
      "When you next cast a spell this turn, draw a card. You may choose new targets for the copy.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseDelayedPrefix(c, { modalOptions: [] }, parseSentences), null, text);
      assert.equal(c.pos, 0, text);
    }
  });

  it("keeps the printed trigger limit outside the delayed body it creates", () => {
    assert.deepEqual(full("Whenever this creature attacks, draw a card. At the beginning of the next end step, sacrifice it. This ability triggers only once each turn."), [{
      kind: "triggered", trigger: { trigger: "attacks", what: source }, limit: { maximum: 1, period: "turn" }, effects: [
        { steps: [draw] },
        { steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
          occurrence: "next", nextOnly: true, targets: "delayed-ability", effects: [{ steps: [{ effect: "sacrifice", who: you, what: { ref: "it" } }] }],
        }] },
      ],
    }]);
  });

  it("rejects incomplete and ambiguous lifetime syntax transactionally", () => {
    for (const text of [
      "When that creature dies this game, draw a card.", "When that creature dies this turn next turn, draw a card.",
      "When you next cast next a spell this turn, draw a card.", "When you cast your next spell this turn, draw a card.",
      "At the beginning of your next end step, if you control, draw a card.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseDelayedPrefix(c, { modalOptions: [] }, parseSentences), null, text);
      assert.equal(c.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
    for (const text of ["Whenever a creature dies, draw a card."]) {
      const c = new Cursor(lex(text));
      assert.equal(parseDelayedSpellPrefix(c, { modalOptions: [] }, parseSentences), null, text);
      assert.equal(c.pos, 0, text);
    }
    // An explicit "next" is a complete delayed-trigger lifetime (one
    // trigger at the next such step, CR 603.7b), so this spell line is now
    // represented instead of rejected (Glorious End, the Pact cycle).
    const next = new Cursor(lex("At the beginning of your next end step, draw a card."));
    assert.deepEqual(parseDelayedSpellPrefix(next, { modalOptions: [] }, parseSentences), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "your" },
      occurrence: "next", nextOnly: true, targets: "delayed-ability", effects: [{ steps: [draw] }],
    }]);
    assert.equal(next.done(), true);
  });
});
